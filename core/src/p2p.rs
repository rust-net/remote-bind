use std::error::Error;

use quinn::Endpoint;

use crate::{p2p_utils::{make_client_endpoint, make_server_endpoint}, unsafe_quic_client};

pub fn get_client_endpoint(bind: Option<&str>) -> Result<Endpoint, Box<dyn Error>> {
    let client_addr = bind.unwrap_or("0.0.0.0:0").parse().unwrap();
    let mut endpoint = make_client_endpoint(client_addr, &[])?;
    endpoint.set_default_client_config(unsafe_quic_client::get_config());
    Ok(endpoint)
}

pub fn get_server_endpoint(bind: Option<&str>) -> Result<Endpoint, Box<dyn Error>> {
    let server_addr = bind.unwrap_or("0.0.0.0:0").parse().unwrap();
    let (endpoint, _server_cert) = make_server_endpoint(server_addr)?;
    Ok(endpoint)
}