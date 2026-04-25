use crate::io::events::{ProgressEvent, SinkEvent};
use crate::io::traits::{Progress, Sink};

pub struct NullSink;

impl Sink for NullSink {
    fn emit(&mut self, event: SinkEvent) {
        drop(event);
    }
}

pub struct NullProgress;

impl Progress for NullProgress {
    fn emit(&mut self, event: ProgressEvent) {
        drop(event);
    }
}
