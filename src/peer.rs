pub struct Peer {
    id: u64,
    base_uri: String,
}

impl Peer {
    pub fn new(id: u64, base_uri: String) -> Self {
        Peer { id, base_uri }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn base_uri(&self) -> &str {
        &self.base_uri
    }
}
