//! Typed client and serde types for the Vynull HTTP API (0.6.0).
//!
//! Types are transcribed by hand from `docs/api/openapi.yaml`. Go encodes nil
//! slices as `null`, so list fields use [`null_default`].

pub mod client;
pub mod types;

pub use client::{Client, Error, Tracks};

pub use types::*;
