use core::*;
use std::io::{Error, ErrorKind};

use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use yaml_rust::YamlLoader;

/// Vec<u8> -> address
pub static RULES: Lazy<Mutex<Vec<(Vec<u8>, String)>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CONFIG_PATHS: Lazy<Vec<&str>> = Lazy::new(|| vec!["config.yml", "oneport.yml", "oneport/config.yml", "oneport/oneport.yml"]);
static LISTEN: &str = "0.0.0.0:1111";
static LOCAL_API: &str = "127.0.0.111:1111";

/// 解引用内置规则
fn dereference(key: &str) -> Vec<Vec<u8>> {
    if !key.starts_with("$") {
        return vec![key.into()];
    }
    match key {
        "$SSH" => vec![vec![83]],
        "$RDP" => vec![vec![3]],
        "$HTTP" => vec![b"GET".to_vec(), b"POST".to_vec(), b"OPTIONS".to_vec(), b"DELETE".to_vec(), b"PUT".to_vec(), b"HEAD".to_vec()],
        // 如果不想匹配内置规则，只需将首字符 "$" 转义为 "$$"
        _ => vec![key.replacen("$$", "$", 1).into()],
    }
}

pub async fn find_config(file: Option<String>) -> Result<String, ()> {
    match file {
        Some(file) => {
            i!("Finding config: {file}");
            let config = match tokio::fs::read(file).await {
                Ok(v) => v,
                _ => return Err(()),
            };
            let config = String::from_utf8(config).unwrap_or_default();
            return Ok(config);
        }
        None => {
            for file in CONFIG_PATHS.as_slice() {
                i!("Finding config: {file}");
                let config = match tokio::fs::read(file).await {
                    Ok(v) => v,
                    _ => continue,
                };
                let config = String::from_utf8(config).unwrap_or_default();
                return Ok(config);
            }
        }
    }
    Err(())
}

pub async fn load_config(config: &str) -> std::io::Result<(String, String)> {
    let config = YamlLoader::load_from_str(config)
        .map_err(|_err| {
            Error::from(ErrorKind::InvalidData)
        })?;
    if config.len() < 1 {
        return Err(Error::from(ErrorKind::InvalidData));
    }
    let doc = &config[0];
    // Maybe BadValue if not found this field, as_vec() will be Err
    let rules = &doc["rules"];
    // rules: item[]
    // item: { rule: number | number[] | string, address: string }
    let mut mutex_rules = RULES.lock().await;
    mutex_rules.clear();
    for item in rules.as_vec().unwrap_or(&vec![]) {
        let rule = &item["rule"];
        let multi_rule = if let Some(byte) = rule.as_i64() {
            vec![vec![byte as u8]]
        } else if rule.is_array() {
            let rule = rule.as_vec().unwrap();
            let rule = rule.iter().map(|it| { it.as_i64().unwrap_or_default() as u8 });
            let rule: Vec<u8> = rule.collect();
            vec![rule]
        } else if let Some(rule) = rule.as_str() {
            dereference(rule)
        } else {
            vec![vec![]]
        };
        for rule in multi_rule {
            let address = if let Some(v) = item["address"].as_str() { v } else { e!("{:?} -> Invalid address", rule); continue; };
            i!("{:?} -> {address}", rule);
            mutex_rules.push((rule, address.into()));
        }
    }
    let config = &doc["config"];
    let listen = config["listen"].as_str().unwrap_or(LISTEN).into();
    let api = config["api"].as_str().unwrap_or(LOCAL_API).into();
    Ok((listen, api))
}