use crate::{runtime::AgentEvent, wire::NativeStreamEvent};
use std::sync::{Arc, Mutex};

pub type AgentEventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct StreamState {
    has_streamed: Arc<Mutex<bool>>,
    sink: AgentEventSink,
}

impl StreamState {
    pub fn new(sink: AgentEventSink) -> Self {
        Self {
            has_streamed: Arc::new(Mutex::new(false)),
            sink,
        }
    }

    pub fn handler(&self) -> impl FnMut(NativeStreamEvent) + Send + 'static {
        let state = self.clone();
        move |event| state.handle(event)
    }

    pub fn has_streamed(&self) -> bool {
        self.has_streamed.lock().map(|value| *value).unwrap_or(false)
    }

    fn handle(&self, event: NativeStreamEvent) {
        match event {
            NativeStreamEvent::TextDelta(text) => {
                if text.is_empty() {
                    return;
                }
                if let Ok(mut has_streamed) = self.has_streamed.lock() {
                    *has_streamed = true;
                }
                (self.sink)(AgentEvent::MessageDelta { text });
            }
        }
    }
}
