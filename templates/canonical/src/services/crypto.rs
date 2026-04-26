use rand::RngCore;
use sha2::{Digest, Sha256};

pub const SESSION_TOKEN_PREFIX: &str = "cb_";

const TOKEN_BYTES: usize = 24;

pub fn generate_session_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let body = bs58::encode(buf).into_string();
    format!("{SESSION_TOKEN_PREFIX}{body}")
}

pub fn sha256(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}
