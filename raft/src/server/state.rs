use std::sync::Mutex;

use crate::utils::Config;

use super::log::*;

pub type NodeTerm = u32;
pub type NodeId = u64;

#[derive(Debug)]
pub enum NodeType {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
pub struct PersistentState<T: Entry> {
    pub node_term: NodeTerm,
    pub voted_for: Option<NodeId>,
    pub log: Log<T>,
}

impl<T: Entry> PersistentState<T> {
    fn init_state() -> PersistentState<T> {
        return PersistentState {
            node_term: 0,
            voted_for: None,
            log: Log::create_empty_log(),
        };
    }
}

#[derive(Debug)]
pub struct VolatileState {
    // volatile state on server
    pub commited_index: LogIndex,
    pub last_applied: LogIndex,

    // volatile state on leader
    // to be reinitialized after
    // every election
    pub next_index: LogIndex,
    pub match_index: LogIndex,
}

impl VolatileState {
    fn init_state() -> VolatileState {
        return VolatileState {
            commited_index: 0,
            last_applied: 0,
            next_index: 0,
            match_index: 0,
        };
    }
}

#[derive(Debug)]
pub struct State<T: Entry> {
    pub node_type: NodeType,

    pub persistent_state: PersistentState<T>,
    pub volatile_state: VolatileState,
}

#[derive(Debug)]
pub struct Raft<T: Entry> {
    pub config: Config,
    pub state: Mutex<State<T>>,
}

impl<T: Entry> Raft<T> {
    pub fn new_from_config(config: &Config) -> Raft<T> {
        let state = State {
            node_type: NodeType::Follower,
            persistent_state: PersistentState::init_state(),
            volatile_state: VolatileState::init_state(),
        };

        Raft {
            config: config.to_owned(),
            state: Mutex::new(state),
        }
    }

    pub fn start_consensus() {
        todo!()
    }
}
