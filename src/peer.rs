use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::config::Config;

pub struct Peer {
    config: Config,

    uri: String,
    ping_at: Instant,
    remove_at: Instant,
}

impl Peer {
    pub fn new(uri: String, config: Config) -> Self {
        let now = Instant::now();
        let ping_at = now + config.ping_every;
        let remove_at = now + config.alive_timeout;

        Self {
            config,
            uri,
            ping_at,
            remove_at,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn mark_alive(&mut self) {
        let now = Instant::now();
        self.remove_at = now + self.config.alive_timeout;
        self.ping_at = now + self.config.ping_every;
    }

    pub fn remove_at(&self) -> Instant {
        self.remove_at
    }

    pub fn ping_at(&self) -> Instant {
        self.ping_at
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
