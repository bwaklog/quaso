// use rmp_serde::{Deserializer, Serializer};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tracing::{debug, info, warn};

use super::{
    log::{Entry, LogEntry, LogIndex},
    state::{NodeId, NodeRole, NodeTerm, State},
};

use std::{
    cmp::Ordering,
    fmt::{Debug, Display},
    net::SocketAddr,
};
use std::{future::Future, sync::Arc};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PingRequest {
    pub term: NodeTerm,
    pub node_id: NodeId,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct PingResponse {
    pub term: NodeTerm,
    pub node_id: NodeId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ElectionVoteRequest {
    pub term: NodeTerm,
    pub candidate_id: NodeId,
    pub last_log_index: LogIndex,
    pub last_log_term: NodeTerm,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct ElectionVoteResponse {
    pub node_id: NodeId,
    pub term: NodeTerm,
    pub vote_granted: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppendEntriesRequest<T>
where
    T: Entry + Debug + Display + Clone,
{
    pub term: NodeTerm,
    pub leader_id: NodeId,
    pub prev_log_index: LogIndex,
    pub prev_log_term: NodeTerm,
    pub entries: Vec<LogEntry<T>>,
    pub leader_commit: LogIndex,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct AppendEntriesResponse {
    pub node_id: NodeId,
    pub term: NodeTerm,
    pub success: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RequestPattern<T>
where
    T: Entry + Debug + Display + Clone,
{
    PingRPC(PingRequest),
    AppendEntriesRPC(AppendEntriesRequest<T>),
    RequestVoteRPC(ElectionVoteRequest),
}

impl<T> RequestPattern<T>
where
    T: Entry + Debug + Display + Serialize + DeserializeOwned + Clone,
{
    pub fn serialize_request(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize_request(req: Vec<u8>) -> Self {
        bincode::deserialize(&req).unwrap()
    }
}

///
/// [  Client  ]<---------[  Server ]
///      |                      |
///   encode                    |
///      |                  encode resp
///    network                  |
///    interface -------> [ req handler ]
///
pub trait Service<T>
where
    T: Entry + Debug + Display + Clone,
{
    fn ping_node(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: PingRequest,
    ) -> impl Future<Output = PingResponse>;
    fn append_entries(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: AppendEntriesRequest<T>,
    ) -> impl Future<Output = AppendEntriesResponse>;
    fn request_vote(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: ElectionVoteRequest,
    ) -> impl Future<Output = ElectionVoteResponse>;
}

#[derive(Debug, Clone)]
pub struct Server<T>
where
    T: Entry + Debug + Display + Serialize + DeserializeOwned + Clone,
{
    pub state: Arc<Mutex<State<T>>>,
    pub node_id: NodeId,
}

#[derive(Debug, Clone)]
pub struct Client;

#[allow(unused)]
impl<T> Service<T> for Server<T>
where
    T: 'static + Entry + Debug + Display + Serialize + DeserializeOwned + Clone,
{
    async fn ping_node(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: PingRequest,
    ) -> PingResponse {
        let state = state.lock().await;

        PingResponse {
            term: state.persistent_state.node_term,
            node_id,
        }
    }

    async fn request_vote(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: ElectionVoteRequest,
    ) -> ElectionVoteResponse {
        let mut state = state.lock().await;

        info!("recieved election request from {:?}", req.candidate_id);

        // check for difference in terms
        if req.term < state.persistent_state.node_term {
            warn!(
                "Vote not granted as req.term {} < self.term {}",
                req.term, state.persistent_state.node_term
            );
            return ElectionVoteResponse {
                node_id,
                term: state.persistent_state.node_term,
                vote_granted: false,
            };
        }

        // check if voted for any other candidate in the same term
        if ![Some(req.candidate_id), None].contains(&state.persistent_state.voted_for)
            && req.term == state.persistent_state.node_term
        {
            warn!(
                "Vote not granted as voted for {:?} in the same term {}",
                state.persistent_state.voted_for, state.persistent_state.node_term
            );
            return ElectionVoteResponse {
                node_id,
                term: state.persistent_state.node_term,
                vote_granted: false,
            };
        }

        let mut grant = false;

        // comparing loge entries
        // NOTE: recheck this condition
        // "Raft determines which of two logs is more up-to-date
        // by comparing the index and term of the last entries in
        // the logs. If the logs have last entries with different
        // terms, then the log with the later term is more up-to
        // -date. If the logs end up with the same term, then
        // whichever log is longer is more up-to-date"
        let log_len = state.persistent_state.log.len();


        if let Some(voter_last_entry) = state.persistent_state.log.last() {
            match voter_last_entry.term.cmp(&req.last_log_term) {
                Ordering::Greater => {
                    // unequal, hence entry with latter term is more up-to-date
                    warn!("Not granting vote (log not up-to-date) as voter_last_entry.term < req.last_log_term");
                    grant = false;
                }
                Ordering::Less => {
                    // candidate has a more up-to-date
                    debug!(
                    "Granting vote (log up-to-date) as voter_last_entry.term < req.last_log_term"
                );
                    grant = true;
                }
                Ordering::Equal => {
                    // compare lengths
                    debug!("current log_len {} and req.last_log_index {}", log_len, req.last_log_index);
                    if log_len <= req.last_log_index {
                        debug!(
                            "Granting vote (log up-to-date) based on log length as terms are equal"
                        );
                        grant = true;
                    } else {
                        debug!("Not granting vote (log not up-to-date) based on log length as terms are equal");
                    }
                }
            }
        } else {
            // NOTE: not sure if this is right read the paper!!
            if state.persistent_state.voted_for.is_none() {
                debug!("Granting vote as I have not voted for anyone yet");
                grant = true;
            } else {
                debug!(
                    "Voted for {:?} in term {:?}",
                    state.persistent_state.voted_for, state.persistent_state.node_term
                );
            }
        }

        if grant {
            state.persistent_state.voted_for = Some(req.candidate_id);
            state.transition_to_term(NodeRole::Follower, req.term, node_id);
            state.volatile_state.reset_election_timeout();
            state
                .recieved_leader_heartbeat
                .store(true, std::sync::atomic::Ordering::Release);
        }

        debug!("{:?}", state.volatile_state.node_type);

        ElectionVoteResponse {
            node_id,
            term: state.persistent_state.node_term,
            vote_granted: grant,
        }
    }

    async fn append_entries(
        node_id: NodeId,
        state: Arc<Mutex<State<T>>>,
        req: AppendEntriesRequest<T>,
    ) -> AppendEntriesResponse {
        debug!("Recieved append entries from {:?}", req);
        let mut state = state.lock().await;

        // Reply falses if term < current term (5.1)
        if req.term < state.persistent_state.node_term
            && state.volatile_state.node_type == NodeRole::Leader
        {
            warn!(
                "Reject AppendEntries as req.term {} < current Term {}",
                req.term, state.persistent_state.node_term
            );
            // reject
            return AppendEntriesResponse {
                node_id,
                term: state.persistent_state.node_term,
                success: false,
            };
        }

        // Reply false if log doesn't contain an entry at
        // prevLogIndex whose term matches prevLogTerm (5.3)
        let log_len = state.persistent_state.log.len();
        if log_len > 0 {
            // if the prevLogIndex is within the current log
            // and the terms dont match
            if req.prev_log_index <= log_len
                && state.persistent_state.log[req.prev_log_index - 1].term != req.prev_log_term
            {
                warn!(
                    "Reject as prevLogIndex {} for current node does not have the same prev_log_term({})",
                    req.prev_log_index,
                    req.prev_log_term
                );
                // reject
                return AppendEntriesResponse {
                    node_id,
                    term: state.persistent_state.node_term,
                    success: false,
                };
            }

            // if the prevLogIndex is out of bounds of the available log
            if req.prev_log_index > log_len {
                warn!(
                    "Reject as prevLogIndex {} is out of bounds of the current log. There are Inconsistencies",
                    req.prev_log_index,
                );
                // reject
                return AppendEntriesResponse {
                    node_id,
                    term: state.persistent_state.node_term,
                    success: false,
                };
            }
        }

        // NOTE: this is redundant
        if req.prev_log_index > log_len {
            warn!(
                "Reject as prevLogIndex {} is out of bounds of the current log. There are inconsistentcies!",
                req.prev_log_index
            );
            return AppendEntriesResponse {
                node_id,
                term: state.persistent_state.node_term,
                success: false,
            };
        }

        if req.term > state.persistent_state.node_term {
            // debug!(
            //     "req.term {} > currentTerm {}",
            //     req.term, state.persistent_state.node_term
            // );
            state.transition_to_term(NodeRole::Follower, req.term, node_id);
            // this field should be set to None?
            state.persistent_state.voted_for = Some(req.leader_id);
        }

        // --- beyond this line we can start accepting
        // the request
        // hence we can reset the election timer
        // and also check the box that we have recieved
        // a ping to the atomic bool
        state
            .recieved_leader_heartbeat
            .store(true, std::sync::atomic::Ordering::Release);
        state.volatile_state.reset_election_timeout();
        debug!("recieved heartbeat from {}, resetting election timeout and recieved_leader_heartbeat = Atomic true", req.leader_id);
        state.transition_to_term(NodeRole::Follower, req.term, node_id);

        // NOTE: for now not replicating

        // If an existing entry conflicts with a new one (
        // same index but different terms), delete the existing
        // entry and all that follow it (5.3)
        let log_len = state.persistent_state.log.len();
        let log_offset_index = req.prev_log_index;

        let new_log_size = req.entries.len() + log_offset_index;
        debug!("new log size {} vs old {}", new_log_size, log_len);

        if new_log_size < log_len {
            debug!("will truncaate log");
            // we need to rewrite over log entries
            state.persistent_state.log.truncate(log_offset_index - 1);
            state.persistent_state.log.extend(req.entries.clone());
        } else {
            debug!("appending new entries");
            // Append any new entries not already in the log
            state.persistent_state.log.extend(req.entries.clone());
        }

        // If leaderCommit > commitIndex, set commitIndex
        // = min(leaderCommit, index of last new entry)
        // NOTE: need to write commit logic on leaders

        AppendEntriesResponse {
            node_id,
            term: state.persistent_state.node_term,
            success: true,
        }
    }
}

// NOTE: too much nesting
#[allow(unused)]
impl<T> Server<T>
where
    T: 'static + Entry + Debug + Display + Serialize + DeserializeOwned + Send + Clone,
{
    pub async fn start_listener(node_id: NodeId, state: Arc<Mutex<State<T>>>, addr: SocketAddr) {
        let listener = TcpListener::bind(addr)
            .await
            .expect("failed to start a tcp listener for raft");

        debug!("Started Raft server at {:?}", addr);

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let mut buf: Vec<u8> = Vec::new();
                    let mut temp: Vec<u8> = Vec::new();

                    // request coming in are going to be serialized to
                    // Vec<u8>. This can easily be deserialized into the
                    // specific RequestPattern<T>

                    loop {
                        if stream.read_buf(&mut temp).await.is_ok() {
                            // debug!("recieved {:?}", temp);
                            if temp.is_empty() {
                                break;
                            }
                            buf.extend(temp.clone());
                            temp.clear();
                        } else {
                            warn!("failed to read stream {:?}", stream);
                        }
                    }

                    if !buf.is_empty() {
                        let req: RequestPattern<T> = bincode::deserialize(&buf).unwrap();
                        // debug!("DESERIALIZED REQU {:?} => {:?}", buf, req);
                        match req {
                            RequestPattern::PingRPC(req) => {
                                let resp =
                                    Server::ping_node(node_id, Arc::clone(&state), req).await;
                                let ser_resp = bincode::serialize(&resp).unwrap();
                                stream.write_all(&ser_resp).await.unwrap();
                                stream.flush().await.unwrap();
                            }
                            RequestPattern::RequestVoteRPC(req) => {
                                let resp =
                                    Server::request_vote(node_id, Arc::clone(&state), req).await;
                                let ser_resp = bincode::serialize(&resp).unwrap();
                                stream.write_all(&ser_resp).await.unwrap();
                                stream.flush().await.unwrap();
                            }
                            RequestPattern::AppendEntriesRPC(req) => {
                                let resp =
                                    Server::append_entries(node_id, Arc::clone(&state), req).await;
                                let ser_resp = bincode::serialize(&resp).unwrap();
                                stream.write_all(&ser_resp).await.unwrap();
                                stream.flush().await.unwrap();
                            }
                        }
                    }

                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}

impl Client {
    /// Flexible function that sends any RequestPattern<T>
    /// down to the server in a serialized fomat by
    /// bincode
    /// Deserialization is not handled by this, rather
    /// the caller will need to handle this pattern
    ///
    /// This flow is still on the drawing board :)
    pub async fn send<T: Entry + Debug + Display + Serialize + DeserializeOwned + Clone>(
        req: RequestPattern<T>,
        addr: SocketAddr,
    ) -> Option<Vec<u8>> {
        if let Ok(ser_req) = bincode::serialize(&req) {
            // debug!("serialize_request {:?} => {:?}", req, ser_req);

            if let Ok(mut stream) = TcpStream::connect(addr).await {
                // debug!(
                //     "established connection with {:?} and sending {:?}",
                //     addr, ser_req
                // );
                stream.write_all(&ser_req).await.unwrap();
                let _ = stream.shutdown().await;

                // debug!("waiting block begin");
                let mut buf: Vec<u8> = Vec::new();
                if stream.read_buf(&mut buf).await.is_ok() {
                    // debug!("waiting block end, success, recieved {:?}", buf);
                    Some(buf)
                } else {
                    debug!("failed to read from stream");
                    let _ = stream.shutdown().await;
                    None
                }
            } else {
                debug!("failed to establish a stream");
                None
            }
        } else {
            None
        }
    }
}

#[allow(unused)]
#[cfg(test)]
mod rpc_tests {
    use std::{
        fmt::{write, Debug, Display},
        hash::Hash,
    };

    use crate::server::{
        log::{Command, LogEntry},
        rpc::{AppendEntriesRequest, ElectionVoteRequest, PingRequest},
    };
    use serde::{Deserialize, Serialize};

    use super::{AppendEntriesResponse, ElectionVoteResponse, Entry, PingResponse};

    #[derive(Debug, Deserialize, Serialize)]
    enum RequestPattern<T>
    where
        T: Entry + Debug + Display + Clone,
    {
        PingRPC(PingRequest),
        AppendEntriesRPC(AppendEntriesRequest<T>),
        RequestVoteRPC(ElectionVoteRequest),
    }

    #[derive(Debug, Deserialize, Serialize)]
    enum ResponsePattern<T>
    where
        T: Entry + Debug + Display + Clone,
    {
        PingRPC(PingRequest),
        AppendEntriesRPC(AppendEntriesRequest<T>),
        RequestVoteRPC(ElectionVoteRequest),
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Pair<K, V>
    where
        K: Eq + Hash + Debug + Display,
        V: Debug + Display,
    {
        key: K,
        val: V,
    }

    impl<K, V> Pair<K, V>
    where
        K: Eq + Hash + Debug + Display + Clone,
        V: Debug + Display + Clone,
    {
        fn new(key: K, val: V) -> Self {
            Pair { key, val }
        }
    }

    impl<K, V> Clone for Pair<K, V>
    where
        K: Eq + Hash + Debug + Display + Clone,
        V: Debug + Display + Clone,
    {
        fn clone(&self) -> Self {
            Pair {
                key: self.key.clone(),
                val: self.val.clone(),
            }
        }
    }

    impl<K, V> Entry for Pair<K, V>
    where
        K: Eq + Hash + Debug + Display + Clone,
        V: Debug + Display + Clone,
    {
        fn deliver(&mut self) {}
    }

    impl<K, V> Display for Pair<K, V>
    where
        K: Eq + Hash + Debug + Display + Clone,
        V: Debug + Display + Clone,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{{ key: {}, value: {} }}", self.key, self.val)
        }
    }

    fn test_process(bytes_over_network: Vec<u8>) -> Vec<u8> {
        let deser: RequestPattern<Pair<u64, String>> =
            bincode::deserialize(&bytes_over_network).unwrap();

        match deser {
            RequestPattern::PingRPC(_) => {
                let resp = PingResponse {
                    term: 1,
                    node_id: 32,
                };
                bincode::serialize(&resp).unwrap()
            }
            RequestPattern::RequestVoteRPC(req) => {
                let resp = ElectionVoteResponse {
                    term: req.term,
                    node_id: 32,
                    vote_granted: true,
                };
                bincode::serialize(&resp).unwrap()
            }
            RequestPattern::AppendEntriesRPC(_) => {
                let resp = AppendEntriesResponse {
                    node_id: 32,
                    term: 1,
                    success: true,
                };
                bincode::serialize(&resp).unwrap()
            }
        }
    }

    #[test]
    fn serialize_request() {
        let log: Vec<LogEntry<Pair<u64, String>>> = vec![
            LogEntry::new(Command::Set, Pair::new(64, "viola".to_string()), 1),
            LogEntry::new(Command::Set, Pair::new(32, "hola".to_string()), 1),
        ];

        let append_rpc: AppendEntriesRequest<Pair<u64, String>> = AppendEntriesRequest {
            term: 1,
            leader_id: 64,
            prev_log_term: 0,
            prev_log_index: 0,
            entries: log,
            leader_commit: 0,
        };

        let req_type = RequestPattern::AppendEntriesRPC(append_rpc);
        let bincode_req = bincode::serialize(&req_type).unwrap();
        println!("{:?}", bincode_req);

        let resp = test_process(bincode_req);
        let resp_deser: AppendEntriesResponse = bincode::deserialize(&resp).unwrap();

        assert_eq!(
            resp_deser,
            AppendEntriesResponse {
                term: 1,
                success: true,
                node_id: 32,
            }
        )
    }
}
