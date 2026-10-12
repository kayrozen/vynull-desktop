//! Shared screens for the web and desktop clients. No platform code: the
//! binaries provide a [`Platform`] through Dioxus context, and everything else
//! goes through `vynull-platform` and `vynull-api`.

mod conn;
mod diag;
mod dom;
mod drawer;
mod fonts;
mod library;
mod library_data;
mod library_tools;
mod live;
mod nav;
mod overlay;
mod phrases;
mod players;
mod prefs;
mod settings;
mod shell;
mod stack;
mod tags;

pub use shell::{App, Platform};
