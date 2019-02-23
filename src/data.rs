extern crate mime_guess;
extern crate tar;

use std::collections::HashMap;
use std::io::Read;

use crate::error::Error;

pub struct DataFile {
    pub mime: String,
    pub content: Vec<u8>,
}

pub struct Data {
    blob: Vec<u8>,
    map: HashMap<String, DataFile>,
}

impl Data {
    pub fn from_tar(blob: Vec<u8>) -> Result<Self, Error> {
        let mut archive = tar::Archive::new(&blob[..]);
        let mut map = HashMap::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let mut content = Vec::with_capacity(entry.header().size()? as usize);

            entry.read_to_end(&mut content)?;
            let path = entry.header().path()?;
            let ext = path
                .extension()
                .map(|ext| ext.to_str())
                .unwrap_or(None)
                .unwrap_or("");
            let mime = format!("{}", mime_guess::get_mime_type(ext));

            if let Some(path) = path.to_str() {
                trace!("new file: {} size: {}", path, content.len());
                let path = path.to_string();

                // Redirect / to /index.html
                if path == "index.htm" || path == "index.html" {
                    map.insert(
                        "".to_string(),
                        DataFile {
                            mime: mime.clone(),
                            content: content.clone(),
                        },
                    );
                }

                map.insert(path, DataFile { mime, content });
            }
        }
        Ok(Data { blob, map })
    }

    pub fn serve(&self, uri: &str) -> Option<&DataFile> {
        self.map.get(uri)
    }
}

impl From<&Data> for Vec<u8> {
    fn from(d: &Data) -> Self {
        d.blob.clone()
    }
}
