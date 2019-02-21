extern crate rand;

use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use rand::{thread_rng, Rng};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct Peer {
    config: Config,

    uri: String,
    ping_at: Instant,
    stable_at: Instant,
    inactive_at: Instant,
    remove_at: Instant,
}

impl Peer {
    pub fn new(uri: String, config: Config) -> Self {
        let now = Instant::now();

        // Initial ping at should be immediate
        let ping_at = now;

        // The rest are regular
        let stable_at = now + config.stable_delay;

        // Until the peer pings - it should be treated as inactive
        let inactive_at = now - config.ping_every.max;
        let remove_at = now + config.remove_timeout;

        Self {
            config,
            uri,
            ping_at,
            stable_at,
            inactive_at,
            remove_at,
        }
    }

    fn ping_delay(config: &Config) -> Duration {
        let min = config.ping_every.min;
        let max = config.ping_every.max;

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
        self.remove_at = now + self.config.alive_timeout + self.config.remove_timeout;
        self.inactive_at = now + self.config.alive_timeout;
        self.ping_at = now + Peer::ping_delay(&self.config);
    }

    pub fn should_remove(&self, now: Instant) -> bool {
        self.remove_at <= now
    }

    pub fn should_ping(&self, now: Instant) -> bool {
        self.ping_at <= now
    }

    pub fn is_stable(&self, now: Instant) -> bool {
        self.stable_at <= now
    }

    pub fn is_active(&self, now: Instant) -> bool {
        now < self.inactive_at
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
