#[derive(Debug, Clone)]
pub enum AccelEvent {
    Interrupt(u32),
    Timer,
    Io,
}

pub trait AccelEventSource {
    fn poll_event(&mut self) -> Option<AccelEvent> {
        None
    }
}
