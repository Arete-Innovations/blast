use crate::governor::rules::helpers::{is_comment_line, rel_path_str, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref WS_RE: Regex = match Regex::new(r"\bnew\s+WebSocket\s*\(|\bsocket\.io\b") {
        Ok(r) => r,
        Err(_re_err) => panic!("WebSocketOutsideRelay regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct WebSocketOutsideRelay;

impl WebSocketOutsideRelay {
    pub fn new() -> Self {
        Self
    }
}

fn is_relay_client(file: &Path) -> bool {
    rel_path_str(file).ends_with("frontend/src/generated/ws/client.ts")
        || rel_path_str(file).ends_with("generated/ws/client.ts")
}

impl Rule for WebSocketOutsideRelay {
    fn name(&self) -> &'static str {
        "WebSocketOutsideRelay"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        _config: &FeLintState,
    ) -> Option<Violation> {
        if is_relay_client(file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if !WS_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "WebSocketOutsideRelay",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use useTopic()/useChannel() composables — Relay owns the singleton socket",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(file: &str, line: &str) -> Option<Violation> {
        let rule = WebSocketOutsideRelay::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from(file), line, 1, &cfg)
    }

    #[test]
    fn flags_new_websocket_in_custom() {
        let v = run(
            "frontend/src/custom/composables/useFoo.ts",
            "const ws = new WebSocket('wss://x')",
        );
        assert!(v.is_some());
    }

    #[test]
    fn flags_socket_io_import() {
        let v = run(
            "frontend/src/custom/x.ts",
            "import { io } from 'socket.io-client'",
        );
        assert!(v.is_some());
    }

    #[test]
    fn allows_in_generated_relay_client() {
        let v = run(
            "frontend/src/generated/ws/client.ts",
            "const ws = new WebSocket(url)",
        );
        assert!(v.is_none());
    }
}
