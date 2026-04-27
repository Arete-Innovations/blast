use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::Engine as _;
use rand::RngCore;

use crate::{cata_log, meltdown::*};

pub fn hash_password(plain: &str) -> Result<String, MeltDown> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let phc = argon
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| MeltDown::new(MeltType::Unexpected("argon2_hash".into()), format!("argon2 hash: {e}")))?;
    Ok(phc.to_string())
}

pub fn verify_password(plain: &str, phc: &str) -> Result<bool, MeltDown> {
    let parsed = PasswordHash::new(phc)
        .map_err(|e| MeltDown::new(MeltType::Unexpected("argon2_parse".into()), format!("argon2 parse: {e}")))?;
    let argon = Argon2::default();
    Ok(argon.verify_password(plain.as_bytes(), &parsed).is_ok())
}

pub fn mint_session_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            cata_log!(Error, format!("system clock before epoch: {}", e));
            0
        }
    }
}
