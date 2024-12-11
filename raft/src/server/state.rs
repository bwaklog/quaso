use core::time;
use std::fmt::{Debug, Display};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use std::{sync::Mutex, thread};

use rand::Rng;
use tracing::{debug, info, warn};

use crate::utils::Config;

use super::log::*;

pub type NodeTerm = u32;
pub type NodeId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
pub struct PersistentState<T: Entry + Debug + Display> {
    pub node_term: NodeTerm,
    pub voted_for: Option<NodeId>,
    pub log: Log<T>,
}

impl<T: Entry + Debug + Display> PersistentState<T> {
    fn init_state() -> PersistentState<T> {
        PersistentState {
            node_term: 0,
            voted_for: None,
            log: Log::create_empty_log(),
        }
    }
}

#[derive(Debug)]
pub struct VolatileState {
    pub node_type: NodeType,

    pub votes_recieved: Vec<NodeId>,

    // volatile state on server
    pub commited_index: LogIndex,
    pub last_applied: LogIndex,

    // volatile state on leader
    // to be reinitialized after
    // every election
    pub next_index: LogIndex,
    pub match_index: LogIndex,

    pub election_timeout: Instant,

    pub recieved_leader_heartbeat: AtomicBool,
}

impl VolatileState {
    fn init_state() -> VolatileState {
        let mut rng = rand::thread_rng();

        VolatileState {
            node_type: NodeType::Follower,
            votes_recieved: Vec::new(),
            commited_index: 0,
            last_applied: 0,
            next_index: 0,
            match_index: 0,
            election_timeout: Instant::now() + time::Duration::from_secs(rng.gen_range(1..=8)),
            recieved_leader_heartbeat: AtomicBool::new(false),
        }
    }

    #[allow(unused)]
    fn reset_election_timeout(&mut self) {
        let mut rng = rand::thread_rng();
        self.election_timeout = Instant::now() + time::Duration::from_secs(rng.gen_range(1..=8));
    }
}

#[derive(Debug)]
pub struct State<T: Entry + Debug + Display> {
    pub persistent_state: PersistentState<T>,
    pub volatile_state: VolatileState,
}

impl<T: Entry + Debug + Display> State<T> {
    pub fn get_node_type(&self) -> NodeType {
        self.volatile_state.node_type
    }
}

#[derive(Debug)]
pub struct Raft<T: Entry + Debug + Display> {
    pub node_id: NodeId,
    pub config: Config,
    pub state: Mutex<State<T>>,
    pub stopped: bool,
}

impl<T: Entry + Debug + Display> Raft<T> {
    pub fn new_from_config(config: &Config) -> Raft<T> {
        let state = State {
            persistent_state: PersistentState::init_state(),
            volatile_state: VolatileState::init_state(),
        };

        Raft {
            node_id: config.node_id,
            config: config.to_owned(),
            state: Mutex::new(state),
            stopped: false,
        }
    }

    pub fn maybe_send_heartbeat(&mut self) {
        let state = self.state.lock().unwrap();
        if state.volatile_state.node_type != NodeType::Leader {
            warn!(
                node_id = self.node_id,
                "Cant send heartbeat as {:?}", state.volatile_state.node_type
            );
            return;
        }

        info!(node_id = self.node_id, "Can send heartbeat as leader");
    }

    pub fn maybe_commit_log_entries(&mut self) {
        let state = self.state.lock().unwrap();
        if state.volatile_state.node_type != NodeType::Leader {
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

    pub fn transition(&mut self, node_type: NodeType, term_increase: NodeTerm) {
        let mut state = self.state.lock().unwrap();
        state.volatile_state.node_type = node_type;
        state.persistent_state.node_term += term_increase;
        info!(
            node_id = self.node_id,
            "Transition to {:?} with term {}", state.volatile_state.node_type, self.node_id
        );
    }

    pub fn handle_candidate_election(&mut self) {
        debug!("sending vote request rpcs");

        // 'election_loop: loop {
        //     let mut state = self.state.lock().unwrap();
        //     state.volatile_state.reset_election_timeout();
        //
        //     for conn in self.config.raft.connections.iter() {
        //         debug!("Sending vote request rpc to {:?}", conn);
        //
        //         if Instant::now() > state.volatile_state.election_timeout {
        //             warn!("Election timed out for Candidate! resstarting election");
        //             break;
        //         }
        //     }
        //
        //     let quorum_len = self.get_quorum_length();
        //     let votes_recieved = state.volatile_state.votes_recieved.len();
        //     println!("{} ~ {}", quorum_len, votes_recieved);
        //     if state.volatile_state.votes_recieved.len() as i32 >= quorum_len {
        //         info!("Safely can transition to a leader");
        //         drop(state);
        //         self.maybe_transition_leader();
        //         break 'election_loop;
        //     }
        // }
    }

    #[allow(unused_mut)]
    pub fn maybe_transition_candidate(&mut self) {
        let mut state = self.state.lock().unwrap();
        if state.volatile_state.node_type != NodeType::Follower {
            warn!(
                node_id = self.node_id,
                "Cannot transition to candidate {:?}", state.volatile_state.node_type,
            );
            return;
        }

        if Instant::now() > state.volatile_state.election_timeout {
            debug!(node_id = self.node_id, "Can transition to candidate");
            info!(node_id = self.node_id, "Can transition to candidate");

            // we now transition to a candidate state
            drop(state);
            self.transition(NodeType::Candidate, 1);
            self.handle_candidate_election();
        }
    }

    pub fn maybe_transition_leader(&mut self) {
        let state = self.state.lock().unwrap();
        if state.volatile_state.node_type != NodeType::Candidate {
            return;
        }
        drop(state);
        self.transition(NodeType::Leader, 0);
    }

    pub fn start_consensus() {
        todo!()
    }

    pub fn tick(&mut self) {
        debug!("starting tick for node {}", self.node_id);
        loop {
            if self.stopped {
                return;
            }

            let state = self.state.lock().unwrap();
            debug!(
                node_id = self.node_id,
                commited_index = %state.volatile_state.commited_index,
                "commit_index",
            );

            // u cant unlock a mutex!
            drop(state);

            let mut rng = rand::thread_rng();
            let sleep_time = Duration::from_secs(rng.gen_range(1..=8));
            debug!(
                node_id = self.node_id,
                "tick with duration {:?}!", sleep_time
            );
            thread::sleep(sleep_time);

            // NOTE: leader methods
            self.maybe_send_heartbeat(); // AppendEntry RPC
            self.maybe_commit_log_entries(); // CommitLogEntries RPC

            // NOTE: follower methods
            self.maybe_transition_candidate();

            // NOTE: candidate methods
        }
    }
}
