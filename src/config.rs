use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    // Hash seed and auth key
    pub hash_seed: (u64, u64),

    // Number of copies of each value
    pub replicate: u32,

    // Initial peer uris
    pub initial_peers: Vec<String>,

    // How often to ping other nodes
    pub min_ping_every: Duration,
    pub max_ping_every: Duration,

    // How many ping retries to allow
    pub alive_timeout: Duration,

    // How many second to wait before considering node stable
    pub stable_delay: Duration,
}

impl Config {
    pub fn new(hash_seed: (u64, u64)) -> Self {
        Self {
            hash_seed,
            replicate: 2,
            initial_peers: vec![],
            min_ping_every: Duration::from_secs(1),
            max_ping_every: Duration::from_secs(5),
            alive_timeout: Duration::from_secs(30),
            stable_delay: Duration::from_secs(10),
        }
    }
}
