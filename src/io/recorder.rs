use crate::io::events::{ProgressEvent, SinkEvent};
use crate::io::traits::{Progress, Sink};

#[derive(Default)]
pub struct RecorderSink {
    pub events: Vec<SinkEvent>,
}

impl RecorderSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[SinkEvent] {
        &self.events
    }

    pub fn take(&mut self) -> Vec<SinkEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Sink for RecorderSink {
    fn emit(&mut self, event: SinkEvent) {
        self.events.push(event);
    }
}

#[derive(Default)]
pub struct RecorderProgress {
    pub events: Vec<ProgressEvent>,
}

impl RecorderProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[ProgressEvent] {
        &self.events
    }

    pub fn take(&mut self) -> Vec<ProgressEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Progress for RecorderProgress {
    fn emit(&mut self, event: ProgressEvent) {
        self.events.push(event);
    }
}
