use core::*;
use std::{collections::HashMap, any::Any};

use once_cell::sync::Lazy;
use tokio::{net::TcpListener, sync::Mutex};

/// Vec<u8> -> address
static RULES: Lazy<Mutex<HashMap<Vec<u8>, String>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

#[tokio::main]
async fn main() {
    let config = tokio::fs::read("oneport/config.yml").await.unwrap();
    let config = String::from_utf8(config).unwrap();
    let config = yaml_rust::YamlLoader::load_from_str(&config).unwrap();
    let doc = &config[0];
    let rules = &doc["rules"];
    for item in rules.as_vec().unwrap() {
        let rule = &item["rule"];
        let address = item["address"].as_str().unwrap();
        if let Some(rule) = rule.as_i64() {
            wtf!(rule)
        } else if rule.is_array() {
            let rule = rule.as_vec().unwrap();
            let rule = rule.iter().map(|it| { it.as_i64().unwrap() });
            let rule: Vec<i64> = rule.collect();
            wtf!(rule)
        }
    }
    boot().await;
}

/// 解引用内置规则
fn dereference(key: &str) -> Vec<u8> {
    if !key.starts_with("$") {
        return key.into();
    }
    let key = key.replace("$$", "$");
    match key.as_str() {
        "$SSH" => vec![83],
        "$RDP" => vec![3],
        "$HTTP" => vec!['H' as u8],
        _ => key.into(),
    }
}

async fn boot() {
    let port = 1111;
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await.unwrap();
    i!("PORT({port}) -> Listening!");
    loop {
        let (visitor, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => unreachable!("{:?}", e),
        };
        visitor.readable().await.unwrap();
        let mut msg = vec![0; 1024];
        match visitor.try_read(&mut msg) {
            Ok(n) => {
                msg.truncate(n);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                i!("误报");
                continue;
            }
            Err(e) => {
                e!("{}", e.to_string());
                break;
            }
        }
        for i in 0..4 {
            i!("msg[{}] = {:x}", i, msg[i]);
        }
    }
}
