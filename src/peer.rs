use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::config::Config;

pub struct Peer {
    uri: String,
    last_ping: Instant,
    last_alive: Instant,
}

impl Peer {
    pub fn new(uri: String) -> Self {
        let now = Instant::now();
        Self {
            uri,
            last_ping: now,
            last_alive: now,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn mark_alive(&mut self) {
        let now = Instant::now();
        self.last_alive = now;
        self.last_ping = now;
    }

    pub fn ping_at(&mut self, config: &Config) -> Instant {
        self.last_ping += config.ping_every;
        self.last_ping
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl Eq for Peer {}

impl Hash for Peer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
    }
}
