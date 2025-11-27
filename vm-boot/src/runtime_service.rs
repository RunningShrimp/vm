use crate::runtime::{RuntimeController, RuntimeEvent, RuntimeEventListener};
use vm_accel::event::{AccelEventSource, AccelEvent};

pub struct RuntimeService<L: RuntimeEventListener, E: AccelEventSource> {
    controller: RuntimeController,
    listener: L,
    accel: Option<E>,
}

impl<L: RuntimeEventListener, E: AccelEventSource> RuntimeService<L, E> {
    pub fn new(controller: RuntimeController, listener: L, accel: Option<E>) -> Self {
        Self { controller, listener, accel }
    }

    pub fn tick(&mut self) {
        if let Some(evt) = self.controller.poll_events() {
            self.listener.on_event(evt);
        }

        if let Some(src) = self.accel.as_mut() {
            if let Some(aevt) = src.poll_event() {
                match aevt {
                    AccelEvent::Interrupt(_) => self.listener.on_event(RuntimeEvent::Error("interrupt".into())),
                    AccelEvent::Timer => self.listener.on_event(RuntimeEvent::Error("timer".into())),
                    AccelEvent::Io => self.listener.on_event(RuntimeEvent::Error("io".into())),
                }
            }
        }
    }

    pub fn controller(&self) -> &RuntimeController {
        &self.controller
    }
}
