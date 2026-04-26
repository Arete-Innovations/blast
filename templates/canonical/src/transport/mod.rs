//! Transport layer — thin external entry points (http/ws/fuses).
//!
//! Per `doc/SPEC_ARCHITECTURE.md`, transport modules call flows only
//! and never reach into models/services directly.

pub mod http;
