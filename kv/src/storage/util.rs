use tracing::debug;

use super::kv::Operation;

pub fn parse_opr(opr: &str) -> Operation {
    let opr = opr.trim().to_lowercase();

    debug!("opr {}", opr);

    match opr.as_str() {
        "set" => Operation::Set,
        "get" => Operation::Get,
        "delete" => Operation::Delete,
        _ => Operation::Invalid,
    }
}

pub fn validate(command: String) -> Option<(Operation, Vec<String>)> {
    let command: Vec<_> = command.trim().split(" ").collect();
    debug!("command: {:?}", command);

    if command.len() == 1 {
        return None;
    }

    let return_cmd = command.clone().into_iter().map(String::from).collect();

    match parse_opr(command[0]) {
        Operation::Get => {
            if command.len() != 2 {
                return None;
            }
            return Some((Operation::Get, return_cmd));
        }
        Operation::Set => {
            if command.len() != 3 {
                return None;
            }
            return Some((Operation::Set, return_cmd));
        }
        Operation::Delete => {
            if command.len() != 2 {
                return None;
            }
            return Some((Operation::Delete, return_cmd));
        }
        Operation::Invalid => return Some((Operation::Invalid, return_cmd)),
    }
}
