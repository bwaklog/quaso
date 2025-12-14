pub fn parse_opr(opr: &str) -> Option<String> {
    let opr = opr.trim().to_lowercase();

    match opr.as_str() {
        "set" | "get" | "delete" => Some(opr.to_string()),
        _ => None,
    }
}

pub fn validate(command: String) -> Option<(String, Vec<String>)> {
    let command: Vec<_> = command.trim().split(" ").collect();

    if command.len() == 1 {
        return None;
    }

    let return_cmd = command.clone().into_iter().map(String::from).collect();

    let Some(opr) = parse_opr(command[0]) else {
        return None;
    };

    return Some((opr, return_cmd));
}
