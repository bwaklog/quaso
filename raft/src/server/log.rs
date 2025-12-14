use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

use super::state::NodeTerm;

pub type LogIndex = usize;

pub trait Entry {
    fn deliver(&mut self);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry<T>
where
    T: Entry + Debug + Display + Clone,
{
    pub value: T,
    pub term: NodeTerm,
}

impl<T> Clone for LogEntry<T>
where
    T: Entry + Debug + Display + Clone,
{
    fn clone(&self) -> Self {
        LogEntry {
            term: self.term,
            value: self.value.clone(),
        }
    }
}

impl<T> LogEntry<T>
where
    T: Entry + Debug + Display + Clone,
{
    pub fn new(value: T, term: NodeTerm) -> Self {
        LogEntry { value, term }
    }
}

#[cfg(test)]
mod generic_serialization {
    use std::fmt::{Debug, Display};

    use super::LogEntry;
    use serde::{Deserialize, Serialize};

    use super::Entry;

    #[derive(Debug, Serialize, Deserialize, Clone)]
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
            value: Pair {
                key: "hello".to_string(),
                val: "world".to_string(),
            },
            term: 1,
        };

        let serde_entry = bincode::serialize(&single_entry).unwrap();

        println!("{:?}", single_entry);
        println!("{:?}", serde_entry);
    }

    #[test]
    fn deserialize_generic() {
        let entry_string_string: LogEntry<Pair<String, String>> = LogEntry {
            value: Pair {
                key: "hello".to_string(),
                val: "world".to_string(),
            },
            term: 1,
        };

        let ser_val = bincode::serialize(&entry_string_string).unwrap();

        let deser_val: LogEntry<Pair<String, String>> =
            bincode::deserialize(&ser_val).expect("failed to deserialize");

        println!("{:?} => {:?}", ser_val, deser_val);
    }
}
