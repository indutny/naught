extern crate serde;

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingEvery {
    pub min: Duration,
    pub max: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
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
        Self {
            container_secret,
            hash_seed,
            replicate: 1,
            initial_peers: vec![],
            ping_every: PingEvery {
                min: Duration::from_secs(1),
                max: Duration::from_secs(3),
            },
            alive_timeout: Duration::from_secs(6),
            remove_timeout: Duration::from_secs(300),
            stable_delay: Duration::from_secs(12),
            rebalance_every: Duration::from_secs(12),
        }
    }
}
