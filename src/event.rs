use crossterm::event::Event;
use std::ops::Deref;

#[derive(Debug)]
pub struct ParallelyEvent {
    inner: crossterm::event::Event,
    propagate: bool,
}

impl ParallelyEvent {
    pub fn new(inner: crossterm::event::Event) -> Self {
        Self {
            inner,
            propagate: true,
        }
    }

    pub fn propagate(&self) -> bool {
        self.propagate
    }

    pub fn stop_propagation(&mut self) {
        self.propagate = false;
    }
}

impl AsRef<crossterm::event::Event> for ParallelyEvent {
    fn as_ref(&self) -> &Event {
        &self.inner
    }
}

impl Deref for ParallelyEvent {
    type Target = crossterm::event::Event;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<crossterm::event::Event> for ParallelyEvent {
    fn from(event: crossterm::event::Event) -> Self {
        Self::new(event)
    }
}
