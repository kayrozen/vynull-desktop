use std::rc::Rc;

use dioxus::prelude::*;
use vynull_api::Client;
use vynull_core::net::is_private_host;
use vynull_core::route::{Page, Route};
use vynull_platform::{time, AudioPlayer, Store, StoreExt};

use crate::conn::ConnectScreen;
use crate::library::Library;
use crate::library_data::use_library;
use crate::live::{use_feed, Feed, Link};
use crate::nav;
use crate::players::Players;
use crate::prefs::{CueColors, Look, Saved, WaveColors, CONNECTION, VOCAL_KEY};
use vynull_core::phrase::Vocal;

const THEME: &str = include_str!("theme.css");
const LIBRARY_CSS: &str = include_str!("library.css");
const SCREENS_CSS: &str = include_str!("screens.css");

/// What the binary supplies: storage and audio playback.
#[derive(Clone)]
pub struct Platform {
    pub store: Rc<dyn Store>,
    pub audio: Rc<dyn AudioPlayer>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Players,
    Library,
    Tags,
    Settings,
    Diag,
    Overlay,
}

impl From<Page> for Tab {
    fn from(p: Page) -> Tab {
        match p {
            Page::Players => Tab::Players,
            Page::Library => Tab::Library,
            Page::Tags => Tab::Tags,
            Page::Settings => Tab::Settings,
            Page::Diag => Tab::Diag,
            Page::Overlay => Tab::Overlay,
        }
    }
}

impl From<Tab> for Page {
    fn from(t: Tab) -> Page {
        match t {
            Tab::Players => Page::Players,
            Tab::Library => Page::Library,
            Tab::Tags => Page::Tags,
            Tab::Settings => Page::Settings,
            Tab::Diag => Page::Diag,
            Tab::Overlay => Page::Overlay,
        }
    }
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::Players,
        Tab::Library,
        Tab::Tags,
        Tab::Settings,
        Tab::Diag,
        Tab::Overlay,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Players => "PLAYERS",
            Tab::Library => "LIBRARY",
            Tab::Tags => "TAGS",
            Tab::Settings => "SETTINGS",
            Tab::Diag => "DIAG",
            Tab::Overlay => "OVERLAY",
        }
    }

    /// SVG path from the mockup's tab icons.
    fn icon(self) -> &'static str {
        match self {
            Tab::Players => "M3 12h4l3-8 4 16 3-8h4",
            Tab::Library => "M4 6h16M4 12h16M4 18h10",
            Tab::Tags => "M20 12l-8 8-9-9V3h8zM7.5 7.5h.01",
            Tab::Settings => "M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4",
            Tab::Diag => "M4 19V5M4 19h16M8 15l3-4 3 2 4-6",
            Tab::Overlay => "M3 5h18v12H3zM8 21h8M12 17v4",
        }
    }
}

#[derive(Clone, PartialEq)]
enum Phase {
    Starting,
    /// Ask for host and port. Carries the last error, if any.
    Prompt(Option<String>),
    Ready(String),
}

/// Context for screens: the connected client.
#[derive(Clone)]
pub struct ClientCtx(pub Client);

/// Appearance settings that apply at once: the settings screen writes them, the app container reads them.
#[derive(Clone, Copy)]
pub struct LookCtx {
    pub look: Signal<Look>,
    pub colors: Signal<WaveColors>,
    /// How strictly phrases are marked as sung.
    pub vocal: Signal<Vocal>,
    /// Default colors of new hot cues A to H.
    pub cue_colors: Signal<CueColors>,
    /// The daemon's `waveform_color` (`blue`, `rgb`, `3band`), for the AUTO color mode.
    pub daemon_wave: Signal<String>,
}

#[component]
pub fn App() -> Element {
    let platform = use_context::<Platform>();
    let mut phase = use_signal(|| Phase::Starting);
    let mut start_route = use_signal(Route::default);

    // Try the saved daemon, then the origin the page came from.
    let p = platform.clone();
    use_future(move || {
        let p = p.clone();
        async move {
            start_route.set(nav::initial_route().await);
            let saved = p.store.get_json::<Saved>(CONNECTION).ok().flatten();
            let mut tries: Vec<String> = saved.iter().map(Saved::base).collect();
            tries.extend(time::same_origin_base());
            for base in tries {
                if Client::new(&base).status().await.is_ok() {
                    phase.set(Phase::Ready(base));
                    return;
                }
            }
            phase.set(Phase::Prompt(None));
        }
    });

    let look = use_signal(|| Look::load(platform.store.as_ref()));
    let colors = use_signal(|| WaveColors::load(platform.store.as_ref()));
    let vocal = use_signal(|| {
        Vocal::from_index(
            platform
                .store
                .get_json::<u8>(VOCAL_KEY)
                .ok()
                .flatten()
                .unwrap_or(Vocal::default().index()),
        )
    });
    let cue_colors = use_signal(|| CueColors::load(platform.store.as_ref()));
    let daemon_wave = use_signal(String::new);
    use_context_provider(|| LookCtx {
        look,
        colors,
        vocal,
        cue_colors,
        daemon_wave,
    });
    rsx! {
        document::Style { {crate::fonts::css()} }
        document::Style { {THEME} }
        document::Style { {LIBRARY_CSS} }
        document::Style { {SCREENS_CSS} }
        div { class: "{look().classes()}", style: "{colors().effective(&daemon_wave()).css_vars(look().day)}",
            match phase() {
                Phase::Starting => rsx! {
                    div { class: "conn", span { class: "mono mut", "CONNECTING…" } }
                },
                Phase::Prompt(err) => rsx! {
                    ConnectScreen {
                        error: err,
                        on_connect: move |s: Saved| {
                            let store = platform.store.clone();
                            spawn(async move {
                                match Client::new(s.base()).status().await {
                                    Ok(_) => {
                                        let _ = store.set_json(CONNECTION, &s);
                                        phase.set(Phase::Ready(s.base()));
                                    }
                                    Err(e) => phase.set(Phase::Prompt(Some(format!("{}: {e}", s.base())))),
                                }
                            });
                        },
                    }
                },
                Phase::Ready(base) => rsx! {
                    Connected { key: "{base}", base: base.clone(), initial: start_route(), on_change: move |_| phase.set(Phase::Prompt(None)) }
                },
            }
        }
    }
}

#[component]
fn Connected(base: String, initial: Route, on_change: EventHandler<()>) -> Element {
    let client = use_hook(|| Client::new(&base));
    use_context_provider(|| ClientCtx(client.clone()));
    let lib = use_library(client.clone());
    // The AUTO waveform color follows the daemon's setting, which the players can change.
    let LookCtx { daemon_wave, .. } = use_context::<LookCtx>();
    let c = client.clone();
    use_future(move || {
        let c = c.clone();
        let mut daemon_wave = daemon_wave;
        async move {
            loop {
                if let Ok(s) = c.settings().await {
                    let w = s
                        .full
                        .pointer("/dev_setting/waveform_color")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if w != *daemon_wave.peek() {
                        daemon_wave.set(w);
                    }
                }
                time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });
    let feed = use_feed(client);
    let mut route = nav::use_route(initial);
    // Switching away from the library and back returns to the same list and track.
    let mut last_library = use_signal(|| {
        if initial.page == Page::Library {
            initial
        } else {
            Route::page(Page::Library)
        }
    });
    use_effect(move || {
        let r = route();
        if r.page == Page::Library {
            last_library.set(r);
        }
    });
    let tab: Tab = route().page.into();
    let go = move |t: Tab| {
        let page: Page = t.into();
        route.set(if page == Page::Library {
            *last_library.peek()
        } else {
            Route::page(page)
        });
    };

    let host = base
        .trim_start_matches("http://")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();
    rsx! {
        if !is_private_host(&host) {
            div { class: "banner",
                "{host} is not on a private network. The Vynull API has no authentication: anyone who can reach it can edit the library."
            }
        }
        TopBar { feed, base: base.clone(), tab, on_tab: go, on_change }
        match tab {
            Tab::Library => rsx! { Library { feed, lib, route } },
            Tab::Players => rsx! {
                main { class: "pg", Players { feed, base: base.clone(), goto: go } }
            },
            Tab::Tags => rsx! { crate::tags::Tags { lib } },
            Tab::Settings => rsx! { crate::settings::Settings { base: base.clone(), goto: go } },
            Tab::Diag => rsx! { crate::diag::Diag { linked: (feed.polled)().linked } },
            Tab::Overlay => rsx! { crate::overlay::OverlayScreen { feed, base: base.clone() } },
        }
    }
}

#[component]
fn TopBar(
    feed: Feed,
    base: String,
    tab: Tab,
    on_tab: EventHandler<Tab>,
    on_change: EventHandler<()>,
) -> Element {
    let polled = (feed.polled)();
    let (link_cls, link_txt) = match polled.linked {
        Some(true) => ("dot", "LINK ON"),
        Some(false) => ("dot off", "LINK OFF"),
        None => ("dot off", "LINK ?"),
    };
    let (stream_cls, stream_txt) = match (feed.link)() {
        Link::Open => ("chip mono", "LIVE"),
        Link::Connecting => ("chip mono", "CONNECTING"),
        Link::Lost => ("chip mono bad", "RECONNECTING"),
    };
    let device = polled
        .status
        .as_ref()
        .map(|s| format!("{} #{}", s.device_name, s.device_number));
    let addr = base.trim_start_matches("http://").to_string();
    rsx! {
        header { class: "top",
            span { class: "brand", "VYNULL" }
            nav { class: "tabs",
                for t in Tab::ALL {
                    button {
                        key: "{t.label()}",
                        class: if t == tab { "tab on" } else { "tab" },
                        onclick: move |_| on_tab.call(t),
                        svg {
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "{t.icon()}" }
                        }
                        "{t.label()}"
                    }
                }
            }
            span { class: "sp" }
            span { class: "chip mono", i { class: "{link_cls}" } "{link_txt}" }
            span { class: "{stream_cls}", "{stream_txt}" }
            if let Some(d) = device {
                span { class: "chip mono", "{d}" }
            }
            button { class: "chip mono", title: "Change daemon", onclick: move |_| on_change.call(()), "{addr}" }
        }
    }
}
