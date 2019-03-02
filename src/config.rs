extern crate serde;

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingEvery {
    pub min: Duration,
    pub max: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    // Optional https_port to advertise to other peers
    pub https_port: Option<u16>,

    // Container secret for hmac
    pub container_secret: Vec<u8>,

    // Hash seed and auth key
    pub hash_seed: (u64, u64),

    // Number of copies of each value
    pub replicate: u32,

    // Initial peer uris
    pub initial_peers: Vec<String>,

    // How often to ping other nodes
    pub ping_every: PingEvery,

    // At least one ping should arrive during this time for requests to
    // be forwarded to this remote node
    pub alive_timeout: Duration,

    // Remote node would be forgotten after this timeout
    pub remove_timeout: Duration,

    // How many second to wait before considering node stable
    pub stable_delay: Duration,

    // How often to rebalance keys between servers
    pub rebalance_every: Duration,
}

impl Config {
    pub fn new(container_secret: Vec<u8>, hash_seed: (u64, u64)) -> Self {
        Config::from(UserConfig {
            https_port: None,
            container_secret,
            hash_seed,
            replicate: None,
            initial_peers: vec![],
            ping_every: None,
            alive_timeout: None,
            remove_timeout: None,
            stable_delay: None,
            rebalance_every: None,
        })
    }

    pub fn get_auth(&self) -> String {
        format!("Bearer {:016x}-{:016x}", self.hash_seed.0, self.hash_seed.1)
    }
}

impl From<UserConfig> for Config {
    fn from(config: UserConfig) -> Self {
        Self {
            https_port: config.https_port,
            container_secret: config.container_secret,
            hash_seed: config.hash_seed,
            replicate: config.replicate.unwrap_or(2),
            initial_peers: config.initial_peers,
            ping_every: config.ping_every.unwrap_or_else(|| PingEvery {
                min: Duration::from_secs(1),
                max: Duration::from_secs(3),
            }),
            alive_timeout: config
                .alive_timeout
                .unwrap_or_else(|| Duration::from_secs(6)),
            remove_timeout: config
                .remove_timeout
                .unwrap_or_else(|| Duration::from_secs(300)),
            stable_delay: config
                .stable_delay
                .unwrap_or_else(|| Duration::from_secs(12)),
            rebalance_every: config
                .rebalance_every
                .unwrap_or_else(|| Duration::from_secs(12)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    // Optional https_port to advertise to other peers
    pub https_port: Option<u16>,

    // Container secret for hmac
    pub container_secret: Vec<u8>,

    // Hash seed and auth key
    pub hash_seed: (u64, u64),

    // Number of copies of each value
    pub replicate: Option<u32>,

    // Initial peer uris
    pub initial_peers: Vec<String>,

    // How often to ping other nodes
    pub ping_every: Option<PingEvery>,

    // At least one ping should arrive during this time for requests to
    // be forwarded to this remote node
    pub alive_timeout: Option<Duration>,

    // Remote node would be forgotten after this timeout
    pub remove_timeout: Option<Duration>,

    // How many second to wait before considering node stable
    pub stable_delay: Option<Duration>,

    // How often to rebalance keys between servers
    pub rebalance_every: Option<Duration>,
}
