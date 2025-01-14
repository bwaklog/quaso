// NOTE: SEP 30, 2024
// commit pending tests

use core::{fmt, net};
use std::{fs, path::PathBuf, str::FromStr};
// FIX: Handle error type for yaml
use yaml_rust2::{Yaml, YamlLoader};

pub type HelperErrorResult<T> = Result<T, HelperErrors>;

#[derive(Debug)]
pub enum HelperErrors {
    IOError(std::io::Error),
    SocketParserError(net::AddrParseError),
    ParserYamlError,
}

impl fmt::Display for HelperErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelperErrors::ParserYamlError => write!(
                f,
                "Custom yaml_rust2::Yaml parser error {}:{}",
                file!(),
                line!()
            ),
            HelperErrors::SocketParserError(err) => write!(f, "Socket Parse Error: {}", err),
            HelperErrors::IOError(err) => write!(f, "IOError: {}", err),
        }
    }
}

impl From<std::net::AddrParseError> for HelperErrors {
    fn from(value: std::net::AddrParseError) -> Self {
        HelperErrors::SocketParserError(value)
    }
}

impl From<std::io::Error> for HelperErrors {
    fn from(value: std::io::Error) -> Self {
        HelperErrors::IOError(value)
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct RaftConfig {
    pub persist_path: PathBuf,
    pub listener_addr: String,
    pub connections: Vec<String>,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub server_addr: String,
    pub local_path: PathBuf,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Config {
    pub node_id: u64,
    pub node_name: String,
    pub raft: RaftConfig,
    pub store: StoreConfig,
}

pub fn parse_config(state_path: PathBuf) -> HelperErrorResult<Config> {
    let content = fs::read_to_string(state_path.as_path())?;
    let yaml_content = YamlLoader::load_from_str(&content).unwrap();

    let metadata = from_yaml_hash("metadata", yaml_content.first().unwrap()).unwrap();
    let services_yaml: &Yaml = from_yaml_hash("services", yaml_content.first().unwrap()).unwrap();
    let raft_conf_yaml: &Yaml = from_yaml_hash("raft", services_yaml).unwrap();
    let store_conf_yaml: &Yaml = from_yaml_hash("store", services_yaml).unwrap();

    // Metadata
    let node_name = from_yaml_hash("node_name", metadata).and_then(yaml_enum_to_string)?;

    let node_id: u64 = from_yaml_hash("node_id", metadata)
        .and_then(yaml_enum_to_string)?
        .trim()
        .parse()
        .unwrap();

    let persist_path = from_yaml_hash("persist_file", raft_conf_yaml)
        .and_then(yaml_enum_to_string)
        .map(PathBuf::from)?;
    // .and_then(|path_str| {
    //     let path = PathBuf::from(path_str);
    //     Ok(path)
    // })?;

    let listener_addr =
        from_yaml_hash("listiner_addr", raft_conf_yaml).and_then(yaml_enum_to_string)?;
    // .and_then(|sock_addr| {
    //     let sock_v4 = SocketAddrV4::from_str(&sock_addr)?;
    //     Ok(SocketAddr::V4(sock_v4))
    // })?;

    let connections = from_yaml_hash("connections", raft_conf_yaml)
        .and_then(yaml_to_vec_string)?
        .into_iter()
        // .map(|ip_string| {
        //     let ip_addr: &str = ip_string.as_ref();
        //     SocketAddr::V4(SocketAddrV4::from_str(ip_addr).unwrap())
        // })
        .collect();

    // Store Config
    let server_addr =
        from_yaml_hash("server_addr", store_conf_yaml).and_then(yaml_enum_to_string)?;
    // .and_then(|sock_addr| {
    //     let sock_v4 = SocketAddrV4::from_str(&sock_addr)?;
    //     Ok(SocketAddr::V4(sock_v4))
    // })?;
    let local_path_string =
        from_yaml_hash("local_path", store_conf_yaml).and_then(yaml_enum_to_string)?;

    let local_path = PathBuf::from(local_path_string);

    let raft_conf: RaftConfig = RaftConfig {
        persist_path,
        listener_addr,
        connections,
    };

    let store_conf: StoreConfig = StoreConfig {
        server_addr,
        local_path,
    };

    let conf: Config = Config {
        node_id,
        node_name,
        raft: raft_conf,
        store: store_conf,
    };

    Ok(conf)

    // return conf;
}

fn from_yaml_hash<'a>(key: &'a str, yaml_content: &'a Yaml) -> Result<&'a Yaml, HelperErrors> {
    match yaml_content {
        Yaml::Null => Err(HelperErrors::ParserYamlError),
        Yaml::Hash(val) => {
            let result = &val[&Yaml::String(String::from_str(key).unwrap())];
            Ok(result)
        }
        _ => Err(HelperErrors::ParserYamlError),
    }
}

//
// NOTE: VERY Bad function names
//

fn yaml_enum_to_string(yaml_inp: &Yaml) -> Result<String, HelperErrors> {
    if let Yaml::String(val) = yaml_inp {
        Ok(val.to_owned())
    } else if let Yaml::Integer(val) = yaml_inp {
        Ok(format!("{}", val))
    } else {
        Err(HelperErrors::ParserYamlError)
    }
}

fn yaml_to_vec_string(yaml_inp: &Yaml) -> Result<Vec<String>, HelperErrors> {
    let mut ret_vec: Vec<String> = Vec::new();
    if let Yaml::Array(yaml_arr) = yaml_inp {
        for yaml_val in yaml_arr {
            if let Yaml::String(string_val) = yaml_val {
                // NOTE: is to_owned correct?
                ret_vec.push(string_val.to_owned());
            } else {
                return Err(HelperErrors::ParserYamlError);
            }
        }
    } else {
        return Err(HelperErrors::ParserYamlError);
    }
    Ok(ret_vec)
}
