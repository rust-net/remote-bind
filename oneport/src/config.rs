use core::*;
use std::io::{Error, ErrorKind};

use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use yaml_rust::YamlLoader;

/// Vec<u8> -> address
pub static RULES: Lazy<Mutex<Vec<(Vec<u8>, String)>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CONFIG_PATHS: Lazy<Vec<&str>> = Lazy::new(|| vec!["config.yml", "oneport.yml", "oneport/config.yml", "oneport/oneport.yml"]);
static LISTEN: &str = "0.0.0.0:1111";
static LOCAL_API: &str = "127.0.0.111:11111";

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

/// 以UTF-8编码读取指定的配置文件文本内容, 如果file为None, 则使用默认配置文件
pub async fn read_config(file: Option<String>) -> Option<String> {
    match find_config(file.clone()).await {
        Some(v) => Some(v),
        None => {
            e!(
                "Not found the config file({})!",
                if file.is_none() {
                    "such as: config.yml, oneport.yml"
                } else {
                    file.as_ref().unwrap()
                }
            );
            None
        }
    }
}

/// 如果未从命令行指定配置文件, 则尝试加载默认配置文件
async fn find_config(file: Option<String>) -> Option<String> {
    match file {
        Some(file) => {
            i!("Finding config: {file}");
            let config = match tokio::fs::read(file).await {
                Ok(v) => v,
                _ => return None,
            };
            // BOM 编码的 UTF-8 字符串将从以下三个字节开始：EF BB BF
            // 从文件/流中提取字符串时，必须忽略这些字节（如果存在）。
            let config = if config[..3] == vec![0xEF, 0xBB, 0xBF] {
                &config[3..]
            } else {
                &config
            };
            let config = String::from_utf8_lossy(config).to_string();
            Some(config)
        }
        None => {
            for file in CONFIG_PATHS.as_slice() {
                i!("Finding config: {file}");
                let config = match tokio::fs::read(file).await {
                    Ok(v) => v,
                    _ => continue,
                };
                let config = if config[..3] == vec![0xEF, 0xBB, 0xBF] {
                    &config[3..]
                } else {
                    &config
                };
                let config = String::from_utf8_lossy(config).to_string();
                return Some(config);
            }
            None
        }
    }
}

/// 根据JSON字符串加载规则, 返回监听地址和API地址
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