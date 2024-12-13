use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

use super::state::NodeTerm;

pub type LogIndex = usize;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Command {
    Set,
    Remove,
}

pub trait Entry {
    fn deliver(&mut self);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry<T>
where
    T: Entry + Debug + Display,
{
    pub command: Command,
    pub value: Option<T>,
    pub term: NodeTerm,
}

impl<T> LogEntry<T>
where
    T: Entry + Debug + Display,
{
    pub fn new(command: Command, value: T, term: NodeTerm) -> Self {
        LogEntry {
            command,
            value: Some(value),
            term,
        }
    }
}

#[cfg(test)]
mod generic_serialization {
    use std::fmt::{Debug, Display};

    use super::{LogEntry, Command};
    use serde::{Deserialize, Serialize};

    use super::Entry;

    #[derive(Debug, Serialize, Deserialize)]
    struct Pair<K, V>
    where
        K: Display,
        V: Display,
    {
        key: K,
        val: V,
    }

    impl<K, V> Display for Pair<K, V>
    where
        K: Display,
        V: Display,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{{ key: {}, value: {} }}", self.key, self.val)
        }
    }

    impl<K, V> Entry for Pair<K, V>
    where
        K: Display,
        V: Display,
    {
        fn deliver(&mut self) {}
    }

    #[test]
    fn serialization() {
        let single_entry: LogEntry<Pair<String, String>> = LogEntry {
            command: Command::Set,
            value: Some(Pair {
                key: "hello".to_string(),
                val: "world".to_string(),
            }),
            term: 1,
        };

        let serde_entry = bincode::serialize(&single_entry).unwrap();

        println!("{:?}", single_entry);
        println!("{:?}", serde_entry);
    }

    #[test]
    fn deserialize_generic() {
        let entry_string_string: LogEntry<Pair<String, String>> = LogEntry {
            command: Command::Set,
            value: Some(Pair {
                key: "hello".to_string(),
                val: "world".to_string(),
            }),
            term: 1,
        };

        let ser_val = bincode::serialize(&entry_string_string).unwrap();

        let deser_val: LogEntry<Pair<String, String>> =
            bincode::deserialize(&ser_val).expect("failed to deserialize");

        println!("{:?} => {:?}", ser_val, deser_val);
    }
}
