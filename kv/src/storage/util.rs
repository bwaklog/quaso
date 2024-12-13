use super::kv::{Command, Operation};

use std::{fmt::Debug, hash::Hash};

pub fn parse_opr(opr: &str) -> Option<Operation> {
    let opr = opr.trim().to_lowercase();

    match opr.as_str() {
        "set" => Some(Operation::Set),
        "get" => Some(Operation::Get),
        "delete" => Some(Operation::Delete),
        _ => None,
    }
}

pub fn parse<K, V>(command: String) -> Option<Command<K, V>>
where
    K: Eq + Hash + Debug,
    V: Debug,
{
    let command: Vec<_> = command.split("").collect();

    if command.len() == 1 {
        return None;
    }

    if let Some(opr) = parse_opr(command[0]) {
        match opr {
            Operation::Get => {
                if command.len() != 2 {
                    return None;
                }
            }
            Operation::Set => {}
            Operation::Delete => {}
        }
    }

    None
}
