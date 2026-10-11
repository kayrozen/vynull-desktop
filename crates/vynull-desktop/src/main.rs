//! The desktop client: the same screens as the web client in a system webview,
//! with preferences in a file and playback through the sound card.

use std::rc::Rc;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use vynull_platform::{AudioPlayer, FakeAudio, FileStore, MemoryStore, NativeAudio, Store};
use vynull_ui::{App, Platform};

fn main() {
    let window = WindowBuilder::new()
        .with_title("Vynull")
        .with_inner_size(LogicalSize::new(1440.0, 900.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(Root);
}

#[component]
fn Root() -> Element {
    use_context_provider(|| Platform {
        // No config directory or an unreadable file: preferences last for this run only.
        store: match FileStore::open_default() {
            Ok(s) => Rc::new(s) as Rc<dyn Store>,
            Err(e) => {
                eprintln!("vynull: preferences will not be saved: {e}");
                Rc::new(MemoryStore::new())
            }
        },
        // No sound card: the player never plays, and the drawer shows no error.
        audio: match NativeAudio::new() {
            Ok(a) => Rc::new(a) as Rc<dyn AudioPlayer>,
            Err(e) => {
                eprintln!("vynull: no sound output: {e}");
                Rc::new(FakeAudio::new())
            }
        },
    });
    rsx! { App {} }
}
