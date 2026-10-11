use futures_channel::mpsc;
use futures_util::stream::Stream;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, EventSource, MessageEvent};

use super::StreamItem;

/// Owns the `EventSource` and its callbacks. Dropping it closes the connection.
struct Guard {
    es: EventSource,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_state: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(Event)>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.es.close();
    }
}

struct Events {
    rx: mpsc::UnboundedReceiver<StreamItem>,
    /// `None` when the `EventSource` could not be created.
    _guard: Option<Guard>,
}

impl Stream for Events {
    type Item = StreamItem;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<StreamItem>> {
        std::pin::Pin::new(&mut self.rx).poll_next(cx)
    }
}

/// Stream of live state from `url`. The browser reconnects by itself after a
/// network error; it gives up only on an HTTP-level failure, in which case the
/// last item is an [`StreamItem::Error`] and the caller should call again.
/// Drop the stream to disconnect.
pub fn state_stream(url: String) -> impl Stream<Item = StreamItem> {
    let (tx, rx) = mpsc::unbounded();
    let guard = match EventSource::new(&url) {
        Ok(es) => Some(attach(tx, es)),
        Err(e) => {
            let _ = tx.unbounded_send(StreamItem::Error(format!("{e:?}")));
            None
        }
    };
    Events { rx, _guard: guard }
}

fn attach(tx: mpsc::UnboundedSender<StreamItem>, es: EventSource) -> Guard {
    let t = tx.clone();
    let on_open = Closure::<dyn FnMut(Event)>::new(move |_| {
        let _ = t.unbounded_send(StreamItem::Open);
    });
    let t = tx.clone();
    let on_state = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
        let item = match m.data().as_string() {
            Some(s) => match serde_json::from_str(&s) {
                Ok(ev) => StreamItem::State(ev),
                Err(e) => StreamItem::Error(format!("bad state event: {e}")),
            },
            None => StreamItem::Error("state event without text data".into()),
        };
        let _ = t.unbounded_send(item);
    });
    let t = tx;
    let on_error = Closure::<dyn FnMut(Event)>::new(move |_| {
        let _ = t.unbounded_send(StreamItem::Error("connection lost".into()));
    });
    es.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    es.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    let _ = es.add_event_listener_with_callback("state", on_state.as_ref().unchecked_ref());
    Guard {
        es,
        _on_open: on_open,
        _on_state: on_state,
        _on_error: on_error,
    }
}
