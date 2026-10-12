//! Live deck state from `GET /api/events` (Server-Sent Events).
//!
//! [`state_stream`] has the same signature on both targets. On the web it wraps
//! the browser's `EventSource`, which reconnects by itself after a network
//! error (such as a daemon restart). Natively it reads the response body with
//! `eventsource-stream` and reconnects with a growing delay. Either way the
//! stream reports [`StreamItem::Error`] when the link drops and
//! [`StreamItem::Open`] when it comes back, so a screen can show its state.

use vynull_api::StateEvent;

/// What the stream reports.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    /// The connection is open (first connect or a reconnect).
    Open,
    /// One `event: state` message.
    State(StateEvent),
    /// The link dropped or a message was malformed. A reconnect follows.
    Error(String),
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{state_stream, state_stream_with};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::state_stream;
