extern crate rand;

use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use rand::{thread_rng, Rng};

use crate::config::Config;

pub struct Peer {
    config: Config,

    uri: String,
    ping_at: Instant,
    stable_at: Instant,
    remove_at: Instant,
}

impl Peer {
    pub fn new(uri: String, config: Config) -> Self {
        let now = Instant::now();
        let remove_at = now + config.alive_timeout;
        let stable_at = now + config.stable_delay;
        let ping_at = now + Peer::ping_delay(&config);

        Self {
            config,
            uri,
            ping_at,
            stable_at,
            remove_at,
        }
    }

    fn ping_delay(config: &Config) -> Duration {
        let min = config.min_ping_every;
        let max = config.max_ping_every;

        let min = min.as_secs() as f64 + f64::from(min.subsec_nanos()) * 1e-9;
        let max = max.as_secs() as f64 + f64::from(max.subsec_nanos()) * 1e-9;

        let val = thread_rng().gen_range(min, max);

        let secs = val as u64;
        let nsecs = ((val - secs as f64) * 1e9) as u32;

        Duration::new(secs, nsecs)
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn mark_alive(&mut self) {
        let now = Instant::now();
        self.remove_at = now + self.config.alive_timeout;
        self.ping_at = now + Peer::ping_delay(&self.config);
    }

    pub fn remove_at(&self) -> Instant {
        self.remove_at
    }

    pub fn ping_at(&self) -> Instant {
        self.ping_at
    }

    pub fn stable_at(&self) -> Instant {
        self.stable_at
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
