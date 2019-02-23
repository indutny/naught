pub struct Data {
    blob: Vec<u8>,
}

impl Data {
    pub fn from_blob(blob: Vec<u8>) -> Self {
        Data { blob }
    }

    pub fn serve(&self, _uri: &str) -> &[u8] {
        &self.blob
    }
}

impl From<&Data> for Vec<u8> {
    fn from(d: &Data) -> Self {
        d.blob.clone()
    }
}
