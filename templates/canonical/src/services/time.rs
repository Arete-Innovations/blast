use crate::cata_log;

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
