use std::fmt::{Debug, Display};

use super::state::NodeTerm;

pub type LogIndex = usize;

#[derive(Debug)]
pub enum Command {
    Set,
    Remove,
}

pub trait Entry {
    fn deliver(&mut self);
}

#[derive(Debug)]
pub struct LogEntry<T>
where
    T: Entry + Debug + Display,
{
    pub command: Command,
    pub value: Option<T>,
    pub term: NodeTerm,
}

#[derive(Debug)]
pub struct Log<T>
where
    T: Entry + Debug + Display,
{
    pub entries: Vec<LogEntry<T>>,
}

impl<T> Log<T>
where
    T: Entry + Debug + Display,
{
    pub fn append_log(&mut self, entry: LogEntry<T>) {
        self.entries.push(entry);
    }

    pub fn create_empty_log() -> Log<T> {
        Log {
            entries: Vec::new(),
        }
    }
}
