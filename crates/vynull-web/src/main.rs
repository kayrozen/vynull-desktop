#[cfg(target_arch = "wasm32")]
mod web {
    use std::rc::Rc;

    use dioxus::prelude::*;
    use vynull_platform::{AudioPlayer, FakeAudio, MemoryStore, Store, WebAudio, WebStore};
    use vynull_ui::{App, Platform};

    pub fn main() {
        // Without this a panic in the browser is only "unreachable executed".
        console_error_panic_hook::set_once();
        dioxus::launch(Root);
    }

    #[component]
    fn Root() -> Element {
        use_context_provider(|| Platform {
            // Blocked storage (some private modes) falls back to memory: settings
            // then last only for the session.
            // No audio element (very unusual) gives a player that never plays.
            audio: match WebAudio::new() {
                Ok(a) => Rc::new(a) as Rc<dyn AudioPlayer>,
                Err(_) => Rc::new(FakeAudio::new()),
            },
            store: match WebStore::new() {
                Ok(s) => Rc::new(s) as Rc<dyn Store>,
                Err(_) => Rc::new(MemoryStore::new()),
            },
        });
        rsx! { App {} }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::main();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("vynull-web is the browser build: use `dx build --platform web`.");
}
