use crate::error::BlastResult;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const READ_BUFFER_BYTES: usize = 64 * 1024;

pub fn content_hash(path: &Path) -> BlastResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; READ_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

