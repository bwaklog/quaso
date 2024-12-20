use core::time;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use rand::Rng;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::server::rpc::{
    AppendEntriesRequest, AppendEntriesResponse, ElectionVoteRequest, ElectionVoteResponse,
    PingRequest, PingResponse, RequestPattern,
};
use crate::utils::Config;

use super::log::*;
use super::rpc::{Client, Server};

pub type NodeTerm = u32;
pub type NodeId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NodeRole {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersistentState<T>
where
    T: Entry + Debug + Display,
{
    pub node_term: NodeTerm,
    pub voted_for: Option<NodeId>,
    pub log: Vec<LogEntry<T>>,
}

impl<T: Entry + Debug + Display + Serialize + DeserializeOwned> PersistentState<T> {
    async fn init_state(path: PathBuf) -> PersistentState<T> {
        if let Ok(f) = File::open(path.clone()).await {
            debug!("Loading raft state from persistent storage");
            let mut buf = Vec::new();
            let mut reader = BufReader::new(f);
            reader
                .read_buf(&mut buf)
                .await
                .expect("failed to read persistent state");
            let de_ser: PersistentState<T> = bincode::deserialize(&buf).unwrap();
            debug!("state: {:?}", de_ser);
            de_ser
        } else {
            warn!("No persisted state for raft. Creating a fresh state");
            let state = PersistentState {
                node_term: 0,
                voted_for: None,
                log: Vec::new(),
            };

            if let Ok(f) = File::create(&path).await {
                let mut writer = BufWriter::new(f);
                let ser_state = bincode::serialize(&state).unwrap();
                writer
                    .write_all(&ser_state)
                    .await
                    .expect("failed to persist initial state to disk");
                let _ = writer.flush().await;
                let _ = writer.shutdown().await;
                debug!("persisted initial raft state to disk");
            }
            state
        }
    }

    pub async fn persist(&self, path: PathBuf) {
        let state = self;
        let ser_state = bincode::serialize(&state).expect("failed to serialize persistent state");

        if let Ok(f) = File::open(path.clone()).await {
            let mut writer = BufWriter::new(f);
            writer.write_all(&ser_state).await.unwrap_or_else(|_| {
                debug!("failed to write persistent state to {:?}", path);
            });
            let _ = writer.flush().await;
            let _ = writer.shutdown().await;
            debug!("Written raft state to disk");
        } else {
            warn!("Failed to persist raft state to disk");
        }
    }
}

#[derive(Debug)]
pub struct VolatileState {
    pub node_type: NodeRole,

    pub votes_recieved: Vec<NodeId>,

    // volatile state on server
    pub commited_index: LogIndex,
    pub last_applied: LogIndex,

    // volatile state on leader
    // to be reinitialized after
    // every election
    pub next_index: HashMap<SocketAddr, LogIndex>,
    pub match_index: HashMap<SocketAddr, LogIndex>,

    pub heartbeat_timeout: Instant,
    pub election_timeout: Instant,
    pub persist_timeout: Instant,
}

impl VolatileState {
    fn init_state(conns: Vec<SocketAddr>) -> VolatileState {
        let mut rng = rand::thread_rng();

        let mut next_index: HashMap<SocketAddr, LogIndex> = HashMap::new();
        let mut match_index: HashMap<SocketAddr, LogIndex> = HashMap::new();

        for conn in conns {
            next_index.insert(conn, 0);
            match_index.insert(conn, 0);
        }

        VolatileState {
            node_type: NodeRole::Follower,
            votes_recieved: Vec::new(),
            commited_index: 0,
            last_applied: 0,
            next_index,
            match_index,
            heartbeat_timeout: Instant::now() + time::Duration::from_secs(rng.gen_range(1..4)),
            election_timeout: Instant::now() + time::Duration::from_secs(rng.gen_range(4..=8)),
            persist_timeout: Instant::now() + time::Duration::from_secs(5),
        }
    }

    pub fn reset_heartbeat_timeout(&mut self) {
        let mut rng = rand::thread_rng();
        self.heartbeat_timeout = Instant::now() + time::Duration::from_secs(rng.gen_range(1..4));
    }

    #[allow(unused)]
    pub fn reset_election_timeout(&mut self) {
        let mut rng = rand::thread_rng();
        self.election_timeout = Instant::now() + time::Duration::from_secs(rng.gen_range(4..=8));
    }

    #[allow(unused)]
    fn reset_persist_timeout(&mut self) {
        self.election_timeout = Instant::now() + time::Duration::from_secs(2);
    }
}

#[derive(Debug)]
pub struct State<T: Entry + Debug + Display> {
    pub persistent_state: PersistentState<T>,
    pub volatile_state: VolatileState,
    pub recieved_leader_heartbeat: AtomicBool,
}

impl<T: Entry + Debug + Display> State<T> {
    pub fn get_node_type(&self) -> NodeRole {
        self.volatile_state.node_type
    }

    pub fn transition_to_term(&mut self, node_type: NodeRole, to_term: NodeTerm, node_id: NodeId) {
        self.volatile_state.node_type = node_type;
        self.persistent_state.node_term = to_term;
        info!(
            node_id = node_id,
            "Transition to {:?} with term {}", self.volatile_state.node_type, node_id
        );
    }

    pub fn transition(&mut self, node_type: NodeRole, term_increase: NodeTerm, node_id: NodeId) {
        self.volatile_state.node_type = node_type;
        self.persistent_state.node_term += term_increase;
        info!(
            node_id = node_id,
            "Transition to {:?} with term {}", self.volatile_state.node_type, node_id
        );
    }
}

#[derive(Debug)]
pub struct Raft<T>
where
    T: Entry + Debug + Display + Serialize + DeserializeOwned + Clone,
{
    pub node_id: NodeId,
    pub config: Config,
    pub state: Arc<Mutex<State<T>>>,

    // termination condition for the
    // server ticker
    pub stopped: bool,

    // for the "rpc"
    pub server: Server<T>,
    pub client: Client,
}

impl<T> Raft<T>
where
    T: 'static + Entry + Debug + Display + Serialize + DeserializeOwned + Send + Clone,
{
    pub async fn new_from_config(config: &Config) -> Raft<T> {
        let state = State {
            persistent_state: PersistentState::init_state(config.raft.persist_path.clone()).await,
            volatile_state: VolatileState::init_state(config.raft.connections.clone()),
            recieved_leader_heartbeat: AtomicBool::new(false),
        };

        let state_ref = Arc::new(Mutex::new(state));

        Raft {
            node_id: config.node_id,
            config: config.to_owned(),
            state: Arc::clone(&state_ref),
            stopped: false,
            server: Server {
                state: Arc::clone(&state_ref),
                node_id: config.node_id,
            },
            client: Client,
        }
    }

    pub async fn maybe_send_heartbeat(&mut self) {
        let mut state = self.state.lock().await;
        if state.volatile_state.node_type != NodeRole::Leader {
            warn!(
                node_id = self.node_id,
                "Cant send heartbeat as {:?}", state.volatile_state.node_type
            );
            return;
        }

        if Instant::now() < state.volatile_state.heartbeat_timeout {
            warn!(node_id = self.node_id, "Not the time to send a heartbeat");
            return;
        }

        info!(node_id = self.node_id, "Can send heartbeat as leader");

        let connections = self.config.raft.connections.clone();
        let append_request = AppendEntriesRequest::<T> {
            term: state.persistent_state.node_term,
            leader_id: self.node_id,
            prev_log_term: 0,
            prev_log_index: 0,
            entries: Vec::new(),
            leader_commit: 0,
        };

        for conn in connections.iter() {
            if let Some(resp) = Client::send(
                RequestPattern::AppendEntriesRPC(append_request.clone()),
                conn.to_owned(),
            )
            .await
            {
                let de_resp: AppendEntriesResponse = bincode::deserialize(&resp).unwrap();
                if de_resp.success {
                    debug!("Node {} {} accepted append entry", de_resp.node_id, conn);
                } else {
                    warn!("Node {} {} rejected append entry", de_resp.node_id, conn);
                }
            } else {
                warn!("Node {} did not respond to the RPC", conn);
            }
        }

        state.volatile_state.reset_heartbeat_timeout();
    }

    pub async fn maybe_commit_log_entries(&mut self) {
        let state = self.state.lock().await;
        if state.volatile_state.node_type != NodeRole::Leader {
            warn!(
                node_id = self.node_id,
                "commit log entries as {:?}", state.volatile_state.node_type
            );
            return;
        }

        info!(node_id = self.node_id, "Can commit log entries as leader");
    }

    pub fn get_quorum_length(&self) -> i32 {
        let conns = self.config.raft.connections.len() + 1;
        ((conns as f32 / 2_f32).floor() + 1_f32) as i32
    }

    pub async fn transition_to_term(&mut self, node_type: NodeRole, to_term: NodeTerm) {
        let mut state = self.state.lock().await;
        state.transition_to_term(node_type, to_term, self.node_id);
    }

    pub async fn transition_wrapper(&mut self, node_type: NodeRole, term_increase: NodeTerm) {
        let mut state = self.state.lock().await;
        state.transition(node_type, term_increase, self.node_id);
    }

    #[allow(unused)]
    async fn try_persist_state(&self) {
        let state = self.state.lock().await;

        if Instant::now() > state.volatile_state.persist_timeout {
            state
                .persistent_state
                .persist(self.config.raft.persist_path.clone())
                .await;
        }
    }

    pub async fn ping_nodes(&mut self) {
        debug!("pinging nodes!!!");
        let state = self.state.lock().await;

        for conn in self.config.raft.connections.iter() {
            debug!("Sending ping to {:?}", conn);
            if let Some(resp) = Client::send(
                RequestPattern::<T>::PingRPC(PingRequest {
                    node_id: self.node_id,
                    term: state.persistent_state.node_term,
                }),
                conn.to_owned(),
            )
            .await
            {
                debug!("RECIEVED BYTES FROM PING {:?}", resp);
                let dec_rec: PingResponse = bincode::deserialize(&resp).unwrap();
                info!("REVIEVED PING RESPONSE FROM {:?}!! {:?}", conn, dec_rec);
            } else {
                warn!("FAILED PING TO {:?}", conn);
            }
        }
    }

    pub async fn handle_candidate_election(&mut self) {
        debug!("sending vote request rpcs");

        let mut state = self.state.lock().await;
        state.volatile_state.reset_election_timeout();

        state.volatile_state.votes_recieved.clear();
        state.volatile_state.votes_recieved.push(self.node_id);
        state.persistent_state.voted_for = Some(self.node_id);

        let connections = self.config.raft.connections.clone();
        let required_quorum = self.get_quorum_length() as usize;

        let vote_req: ElectionVoteRequest;
        let log_len = state.persistent_state.log.len();
        if let Some(last_log_entry) = state.persistent_state.log.last() {
            vote_req = ElectionVoteRequest {
                candidate_id: self.node_id,
                term: state.persistent_state.node_term,
                last_log_index: log_len - 1,
                last_log_term: last_log_entry.term,
            };
        } else {
            vote_req = ElectionVoteRequest {
                candidate_id: self.node_id,
                term: state.persistent_state.node_term,
                last_log_index: 0,
                last_log_term: 0,
            };
        }

        debug!("VOTE REQUEST FORMAT {:?}", vote_req);

        for conn in connections.iter() {
            debug!("Sending vote request to {:?}", conn);

            if Instant::now() > state.volatile_state.election_timeout {
                warn!("Election timed out for candidate!");
                drop(state);
                self.transition_wrapper(NodeRole::Candidate, 1).await;
                break;
            }

            if let Some(resp) = Client::send(
                RequestPattern::<T>::RequestVoteRPC(vote_req.clone()),
                conn.to_owned(),
            )
            .await
            {
                if let Ok(deser_resp) = bincode::deserialize::<ElectionVoteResponse>(&resp) {
                    debug!("response from {:?} => {:?}", conn, deser_resp);

                    if deser_resp.vote_granted {
                        state.volatile_state.votes_recieved.push(deser_resp.node_id);
                    } else if deser_resp.term >= state.persistent_state.node_term {
                        state.persistent_state.voted_for = None;
                        drop(state);
                        self.transition_to_term(NodeRole::Follower, deser_resp.term)
                            .await;
                        break;
                    }

                    debug!("GRANTED VOTES {:?}", state.volatile_state.votes_recieved);

                    if state.volatile_state.votes_recieved.len() >= required_quorum {
                        // can transition to leader
                        connections.iter().for_each(|addr| {
                            state
                                .volatile_state
                                .next_index
                                .insert(addr.to_owned(), log_len);
                            state.volatile_state.match_index.insert(addr.to_owned(), 0);
                        });
                        drop(state);
                        self.transition_wrapper(NodeRole::Leader, 0).await;
                        // send an empty append entries rpc
                        break;
                    }
                } else {
                    warn!(
                        "Node {:?} failed to write a response back to the stream!",
                        conn
                    );
                }
            }
        }
    }

    #[allow(unused_mut)]
    pub async fn maybe_transition_candidate(&mut self) {
        let mut state = self.state.lock().await;
        if state.volatile_state.node_type == NodeRole::Leader {
            warn!(
                node_id = self.node_id,
                "Cannot transition to candidate {:?}", state.volatile_state.node_type,
            );
            return;
        }

        // NOTE: Temporary
        // if state.persistent_state.voted_for != None {
        //     warn!(
        //         node_id = self.node_id,
        //         "Cannot transition to candidate already voted for {:?}",
        //         state.persistent_state.voted_for
        //     );
        //     state.persistent_state.voted_for = None;
        //     return;
        // }

        if Instant::now() > state.volatile_state.election_timeout
            && !state
                .recieved_leader_heartbeat
                .load(std::sync::atomic::Ordering::Acquire)
        {
            debug!(node_id = self.node_id, "Can transition to candidate");

            // we now transition to a candidate state
            drop(state);
            self.transition_wrapper(NodeRole::Candidate, 1).await;
            self.handle_candidate_election().await;
        } else {
            debug!(
                node_id = self.node_id,
                "Failed to transition to candidate, Leader {:?}", state.persistent_state.voted_for
            );
        }
    }

    pub async fn maybe_transition_leader(&mut self) {
        let state = self.state.lock().await;
        if state.volatile_state.node_type != NodeRole::Candidate {
            return;
        }
        drop(state);
        self.transition_wrapper(NodeRole::Leader, 0).await;
    }

    pub fn start_consensus() {
        todo!()
    }

    pub async fn start_raft_server(&self) {
        Server::start_listener(
            self.node_id,
            Arc::clone(&self.state),
            self.config.raft.listener_addr,
        )
        .await;
    }

    pub async fn tick(&mut self) {
        debug!("starting tick for node {}", self.node_id);
        loop {
            if self.stopped {
                return;
            }

            let state = self.state.lock().await;
            debug!(
                node_id = self.node_id,
                commited_index = %state.volatile_state.commited_index,
                "[NODE {}][TICKER][NODE ROLE {:?}] @ TERM {:?} Voted For {:?}",
                self.node_id,
                state.volatile_state.node_type,
                state.persistent_state.node_term,
                state.persistent_state.voted_for
            );

            // u cant unlock a mutex!
            drop(state);

            let sleep_time = helpers::gen_rand_heartbeat_time();
            // debug!(
            //     node_id = self.node_id,
            //     "tick with duration {:?}!", sleep_time
            // );
            tokio::time::sleep(sleep_time).await;

            // test pings :)
            // self.ping_nodes().await;

            // self.try_persist_state().await;

            // NOTE: leader methods
            self.maybe_send_heartbeat().await; // AppendEntry RPC
                                               // self.maybe_commit_log_entries().await; // CommitLogEntries RPC

            // NOTE: follower methods
            self.maybe_transition_candidate().await;

            // NOTE: candidate methods

            let state = self.state.lock().await;
            state
                .recieved_leader_heartbeat
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }
}

pub mod helpers {
    use rand::Rng;

    pub fn gen_rand_heartbeat_time() -> std::time::Duration {
        let mut rng = rand::thread_rng();
        std::time::Duration::from_secs(rng.gen_range(1..4))
    }

    pub fn gen_rand_election_time() -> std::time::Duration {
        let mut rng = rand::thread_rng();
        std::time::Duration::from_secs(rng.gen_range(4..=8))
    }
}
