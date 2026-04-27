use std::time::Duration;

use reqwest::Response;

use crate::cata_log;

pub fn parse_retry_after(response: &Response) -> Option<Duration> {
    let header = match response.headers().get("Retry-After") {
        Some(h) => h,
        None => return None,
    };
    let s = match header.to_str() {
        Ok(s) => s,
        Err(e) => {
            cata_log!(Debug, format!("Retry-After header is not utf-8: {}", e));
            return None;
        }
    };
    match s.trim().parse::<u64>() {
        Ok(secs) => Some(Duration::from_secs(secs)),
        Err(e) => {
            cata_log!(Debug, format!("Retry-After parse failed for value '{}': {} (HTTP-date format not supported)", s, e));
            None
        }
    }
}
