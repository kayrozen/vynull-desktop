//! The URL hash as the current screen. A page that is reloaded, bookmarked or
//! sent to someone opens on the same screen, and the browser's back button
//! steps through screens. It uses Dioxus's `eval` for the little JavaScript it
//! needs, so the same code runs in a browser and in a desktop webview.

use dioxus::prelude::*;
use vynull_core::route::Route;

/// The hash the page was opened with.
pub async fn initial_route() -> Route {
    let mut ev = document::eval("dioxus.send(location.hash)");
    Route::parse(&ev.recv::<String>().await.unwrap_or_default())
}

/// The current route, kept in step with the URL hash in both directions.
pub fn use_route(initial: Route) -> Signal<Route> {
    let mut route = use_signal(|| initial);

    // The hash changed (back, forward, a pasted link): follow it.
    use_future(move || async move {
        let mut ev = document::eval(
            "window.addEventListener('hashchange', () => dioxus.send(location.hash));",
        );
        while let Ok(h) = ev.recv::<String>().await {
            let r = Route::parse(&h);
            if r != *route.peek() {
                route.set(r);
            }
        }
    });

    // The route changed in the app: write it to the URL.
    use_effect(move || {
        let hash = serde_json::to_string(&route().hash()).unwrap_or_default();
        document::eval(&format!(
            "if (location.hash !== {hash}) location.hash = {hash};"
        ));
    });

    route
}
