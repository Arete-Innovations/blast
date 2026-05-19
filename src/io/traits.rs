use crate::io::events::{ProgressEvent, SinkEvent};

pub trait Sink: Send {
    fn emit(&mut self, event: SinkEvent);
}

pub trait Progress: Send {
    fn emit(&mut self, event: ProgressEvent);
}

pub trait SinkExt: Sink {
    fn info(&mut self, msg: impl Into<String>) {
        self.emit(SinkEvent::Info(msg.into()));
    }

    fn warn(&mut self, msg: impl Into<String>) {
        self.emit(SinkEvent::Warn(msg.into()));
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.emit(SinkEvent::Error(msg.into()));
    }

    fn success(&mut self, msg: impl Into<String>) {
        self.emit(SinkEvent::Success(msg.into()));
    }

    fn debug(&mut self, msg: impl Into<String>) {
        self.emit(SinkEvent::Debug(msg.into()));
    }
}

impl<T: Sink + ?Sized> SinkExt for T {}

pub trait ProgressExt: Progress {
    fn step_start(&mut self, label: impl Into<String>) {
        self.emit(ProgressEvent::StepStart { label: label.into() });
    }

    fn step_done(&mut self, label: impl Into<String>) {
        self.emit(ProgressEvent::StepDone { label: label.into() });
    }

}

impl<T: Progress + ?Sized> ProgressExt for T {}
