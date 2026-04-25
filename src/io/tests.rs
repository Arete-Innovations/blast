#[cfg(test)]
use crate::io::cli::{render_body, CliSink, CliSinkConfig};
#[cfg(test)]
use crate::io::events::{ProgressEvent, SinkEvent, SinkLevel};
#[cfg(test)]
use crate::io::null::{NullProgress, NullSink};
#[cfg(test)]
use crate::io::recorder::{RecorderProgress, RecorderSink};
#[cfg(test)]
use crate::io::traits::{ProgressExt, Sink, SinkExt};
#[cfg(test)]
use std::io::Cursor;
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
struct SharedWriter(Arc<Mutex<Cursor<Vec<u8>>>>);

#[cfg(test)]
impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().expect("shared writer mutex poisoned");
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self.0.lock().expect("shared writer mutex poisoned");
        guard.flush()
    }
}

#[cfg(test)]
fn shared_buf() -> (Arc<Mutex<Cursor<Vec<u8>>>>, Box<dyn std::io::Write + Send>) {
    let buf = Arc::new(Mutex::new(Cursor::new(Vec::new())));
    let writer = SharedWriter(Arc::clone(&buf));
    (buf, Box::new(writer))
}

#[cfg(test)]
fn drain(buf: &Arc<Mutex<Cursor<Vec<u8>>>>) -> String {
    let guard = buf.lock().expect("buf mutex poisoned");
    String::from_utf8(guard.get_ref().clone()).expect("utf8")
}

#[test]
fn recorder_sink_captures_events_in_order() {
    let mut sink = RecorderSink::new();
    sink.info("first");
    sink.warn("second");
    sink.error("third");
    sink.success("fourth");
    sink.debug("fifth");

    let events = sink.events();
    assert_eq!(events.len(), 5);
    assert_eq!(events[0], SinkEvent::Info("first".to_string()));
    assert_eq!(events[1], SinkEvent::Warn("second".to_string()));
    assert_eq!(events[2], SinkEvent::Error("third".to_string()));
    assert_eq!(events[3], SinkEvent::Success("fourth".to_string()));
    assert_eq!(events[4], SinkEvent::Debug("fifth".to_string()));
}

#[test]
fn recorder_sink_take_drains_events() {
    let mut sink = RecorderSink::new();
    sink.info("a");
    sink.info("b");
    let drained = sink.take();
    assert_eq!(drained.len(), 2);
    assert_eq!(sink.events().len(), 0);
}

#[test]
fn recorder_sink_records_structured_diagnostic() {
    let mut sink = RecorderSink::new();
    sink.diagnostic(
        "lint.violation",
        vec![
            ("rule".to_string(), "ERROR:3".to_string()),
            ("file".to_string(), "src/io/mod.rs".to_string()),
        ],
    );
    let events = sink.events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        SinkEvent::StructuredDiagnostic { kind, fields } => {
            assert_eq!(kind, "lint.violation");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "rule");
            assert_eq!(fields[0].1, "ERROR:3");
        }
        other => panic!("expected StructuredDiagnostic, got {:?}", other),
    }
}

#[test]
fn null_sink_swallows_events_silently() {
    let mut sink = NullSink;
    sink.info("ignored");
    sink.warn("also ignored");
    sink.error("nope");
    sink.success("hush");
    sink.debug("nothing");
    sink.diagnostic("kind", vec![]);
}

#[test]
fn null_progress_swallows_events_silently() {
    let mut progress = NullProgress;
    progress.step_start("compile");
    progress.step_done("compile");
    progress.step_fail("link", "missing symbol");
    progress.tick(3, 10);
}

#[test]
fn sink_ext_routes_through_emit() {
    let mut sink = RecorderSink::new();
    Sink::emit(&mut sink, SinkEvent::Info("explicit".to_string()));
    SinkExt::info(&mut sink, "via ext");
    assert_eq!(sink.events().len(), 2);
    assert_eq!(sink.events()[0], SinkEvent::Info("explicit".to_string()));
    assert_eq!(sink.events()[1], SinkEvent::Info("via ext".to_string()));
}

#[test]
fn progress_ext_routes_through_emit() {
    let mut progress = RecorderProgress::new();
    progress.step_start("compile primer");
    progress.tick(1, 4);
    progress.step_done("compile primer");
    progress.step_fail("compile blueprint", "syntax error");

    let events = progress.events();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0],
        ProgressEvent::StepStart {
            label: "compile primer".to_string(),
        }
    );
    assert_eq!(events[1], ProgressEvent::Tick { current: 1, total: 4 });
    assert_eq!(
        events[2],
        ProgressEvent::StepDone {
            label: "compile primer".to_string(),
        }
    );
    assert_eq!(
        events[3],
        ProgressEvent::StepFail {
            label: "compile blueprint".to_string(),
            reason: "syntax error".to_string(),
        }
    );
}

#[test]
fn sink_event_level_mapping_is_total() {
    assert_eq!(
        SinkEvent::Info("x".to_string()).level(),
        SinkLevel::Info
    );
    assert_eq!(
        SinkEvent::Warn("x".to_string()).level(),
        SinkLevel::Warn
    );
    assert_eq!(
        SinkEvent::Error("x".to_string()).level(),
        SinkLevel::Error
    );
    assert_eq!(
        SinkEvent::Success("x".to_string()).level(),
        SinkLevel::Success
    );
    assert_eq!(
        SinkEvent::Debug("x".to_string()).level(),
        SinkLevel::Debug
    );
    assert_eq!(
        SinkEvent::StructuredDiagnostic {
            kind: "k".to_string(),
            fields: vec![],
        }
        .level(),
        SinkLevel::Diagnostic
    );
}

#[test]
fn sink_level_icon_and_label_are_total() {
    let levels = [
        SinkLevel::Debug,
        SinkLevel::Info,
        SinkLevel::Warn,
        SinkLevel::Error,
        SinkLevel::Success,
        SinkLevel::Diagnostic,
    ];
    for level in levels {
        assert!(!level.icon().is_empty());
        assert!(!level.label().is_empty());
    }
}

#[test]
fn render_body_formats_message_variants_verbatim() {
    let event = SinkEvent::Info("plain message".to_string());
    assert_eq!(render_body(&event), "plain message");
}

#[test]
fn render_body_formats_structured_diagnostic_with_fields() {
    let event = SinkEvent::StructuredDiagnostic {
        kind: "primer.missing".to_string(),
        fields: vec![
            ("resource".to_string(), "users".to_string()),
            ("file".to_string(), "primer/src/users.rs".to_string()),
        ],
    };
    let rendered = render_body(&event);
    assert!(rendered.starts_with("primer.missing"));
    assert!(rendered.contains("resource = users"));
    assert!(rendered.contains("file = primer/src/users.rs"));
}

#[test]
fn cli_sink_writes_info_to_stdout_writer() {
    let (out_buf, out_writer) = shared_buf();
    let (err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: None,
            verbose: false,
            quiet: false,
            colorize: false,
        },
    );
    sink.info("hello world");
    let stdout = drain(&out_buf);
    let stderr = drain(&err_buf);
    assert!(stdout.contains("hello world"));
    assert!(stdout.contains(SinkLevel::Info.icon()));
    assert!(stderr.is_empty());
}

#[test]
fn cli_sink_routes_errors_to_stderr_writer() {
    let (out_buf, out_writer) = shared_buf();
    let (err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: None,
            verbose: false,
            quiet: false,
            colorize: false,
        },
    );
    sink.error("boom");
    let stdout = drain(&out_buf);
    let stderr = drain(&err_buf);
    assert!(stderr.contains("boom"));
    assert!(stderr.contains(SinkLevel::Error.icon()));
    assert!(stdout.is_empty());
}

#[test]
fn cli_sink_suppresses_debug_when_not_verbose() {
    let (out_buf, out_writer) = shared_buf();
    let (_err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: None,
            verbose: false,
            quiet: false,
            colorize: false,
        },
    );
    sink.debug("hidden");
    assert!(drain(&out_buf).is_empty());
}

#[test]
fn cli_sink_emits_debug_when_verbose() {
    let (out_buf, out_writer) = shared_buf();
    let (_err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: None,
            verbose: true,
            quiet: false,
            colorize: false,
        },
    );
    sink.debug("visible");
    assert!(drain(&out_buf).contains("visible"));
}

#[test]
fn cli_sink_quiet_silences_all_levels() {
    let (out_buf, out_writer) = shared_buf();
    let (err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: None,
            verbose: true,
            quiet: true,
            colorize: false,
        },
    );
    sink.info("x");
    sink.warn("y");
    sink.error("z");
    sink.success("w");
    sink.debug("d");
    assert!(drain(&out_buf).is_empty());
    assert!(drain(&err_buf).is_empty());
}

#[test]
fn cli_sink_writes_to_log_file_when_configured() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("blast.log");
    let (_out_buf, out_writer) = shared_buf();
    let (_err_buf, err_writer) = shared_buf();
    let mut sink = CliSink::with_writers(
        out_writer,
        err_writer,
        CliSinkConfig {
            log_file: Some(log_path.clone()),
            verbose: true,
            quiet: false,
            colorize: false,
        },
    );
    sink.info("logged message");
    sink.error("something failed");
    let log_contents = std::fs::read_to_string(&log_path).expect("read log");
    assert!(log_contents.contains("logged message"));
    assert!(log_contents.contains("something failed"));
    assert!(log_contents.contains("INFO"));
    assert!(log_contents.contains("ERROR"));
}
