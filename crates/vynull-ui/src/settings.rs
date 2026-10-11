//! Settings screen. Seven sections, as in the mockup:
//! this client's own look (stored locally), the library source tools, a link to
//! the overlay page, the player root menu, the editable player settings, and
//! the read-only mixer and waveform settings.
//!
//! Player settings are one JSON document on the daemon (`GET /api/settings`,
//! `full`), replaced as a whole on each edit: last write wins, as the banner says.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use serde_json::{json, Value};
use vynull_api::{MenuItem, MenuItems, MenuItemsPatch};
use vynull_core::settings::{self, End, Sub};
use vynull_core::{overlay, waveform};
use vynull_platform::StoreExt;

use crate::prefs::{Look, WaveColors};
use crate::shell::{ClientCtx, LookCtx, Platform, Tab};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Web,
    Source,
    Overlay,
    Menu,
    My,
    Djm,
    Dev,
}

impl Section {
    const ALL: [Section; 7] = [
        Section::Web,
        Section::Source,
        Section::Overlay,
        Section::Menu,
        Section::My,
        Section::Djm,
        Section::Dev,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::Web => "WEB UI",
            Section::Source => "LIBRARY SOURCE",
            Section::Overlay => "STREAM OVERLAY",
            Section::Menu => "PLAYER MENU",
            Section::My => "MYSETTING",
            Section::Djm => "DJMMYSETTING",
            Section::Dev => "DEVSETTING",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Section::Web => "Web UI",
            Section::Source => "Library source",
            Section::Overlay => "Stream overlay",
            Section::Menu => "Player menu",
            Section::My => "Player settings",
            Section::Djm => "Mixer settings",
            Section::Dev => "Waveform and key settings",
        }
    }
}

/// `(mode label, preset colors)` for the waveform color modes and presets.
const PRESETS: [(&str, [&str; 3]); 6] = [
    ("3 BAND", ["#B07CFF", "#3FD6C0", "#F4F1E8"]),
    ("RGB", ["#FF4D4D", "#4DFF88", "#4D8CFF"]),
    ("BLUE", ["#2F6BFF", "#5C9DFF", "#D6E8FF"]),
    ("WARM", ["#FF6B3C", "#FFC15A", "#FFF1D0"]),
    ("ICE", ["#3A7BFF", "#6FD0FF", "#EAF6FF"]),
    ("MONO", ["#6B7482", "#9AA4B2", "#F4F1E8"]),
];
const BAND_SWATCHES: [&str; 12] = [
    "#B07CFF", "#3FD6C0", "#F4F1E8", "#FF6B6B", "#FF9A3C", "#FFC15A", "#5FD38D", "#6FB1FF",
    "#FF8AD0", "#E8E05A", "#9AA4B2", "#2F6BFF",
];

fn preset(name: &str) -> WaveColors {
    let c = PRESETS
        .iter()
        .find(|p| p.0 == name)
        .map_or(PRESETS[0].1, |p| p.1);
    WaveColors {
        mode: name.to_string(),
        lo: c[0].into(),
        mi: c[1].into(),
        hi: c[2].into(),
    }
}

#[component]
pub fn Settings(base: String, goto: EventHandler<Tab>) -> Element {
    let mut section = use_signal(|| Section::My);
    rsx! {
        div { class: "pgs",
            div { class: "note", "EDIT TO PUSH LIVE.  PLAYER CHANGES FLOW BACK AUTOMATICALLY.  LAST WRITE WINS." }
            div { class: "lay",
                nav { class: "nav", "aria-label": "Settings sections",
                    for s in Section::ALL {
                        button { key: "{s.label()}", class: if s == section() { "ni on" } else { "ni" }, onclick: move |_| section.set(s), "{s.label()}" }
                    }
                }
                section { class: "cnt",
                    div { class: "hd", span { class: "kick", "SETTINGS" } h1 { "{section().title()}" } }
                    match section() {
                        Section::Web => rsx! { WebSection {} },
                        Section::Source => rsx! { SourceSection {} },
                        Section::Overlay => rsx! { OverlayLink { base: base.clone(), goto } },
                        Section::Menu => rsx! { MenuSection {} },
                        Section::My => rsx! { PlayerSettings { sub: PlayerPart::Editable } },
                        Section::Djm => rsx! { PlayerSettings { sub: PlayerPart::Readonly(Sub::Djm) } },
                        Section::Dev => rsx! { PlayerSettings { sub: PlayerPart::Readonly(Sub::Dev) } },
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- web ui

#[component]
fn WebSection() -> Element {
    let platform = use_context::<Platform>();
    let LookCtx { look, colors } = use_context::<LookCtx>();
    let mut band = use_signal(|| 0usize); // 0 low, 1 mid, 2 high

    let set_look = {
        let store = platform.store.clone();
        use_callback(move |l: Look| {
            let _ = store.set_json(Look::KEY, &l);
            let mut look = look;
            look.set(l);
        })
    };
    let set_colors = {
        let store = platform.store.clone();
        use_callback(move |c: WaveColors| {
            let _ = store.set_json(WaveColors::KEY, &c);
            let mut colors = colors;
            colors.set(c);
        })
    };
    let l = look();
    let c = colors();
    let cur = [c.lo.clone(), c.mi.clone(), c.hi.clone()];
    let band_names = ["KICK (LOW)", "BODY (MID)", "AIR (HIGH)"];
    let paths = waveform::overview_paths(&waveform::demo_entries(400));
    let mode = if c.mode.is_empty() {
        "3 BAND".to_string()
    } else {
        c.mode.clone()
    };
    let same = |a: &WaveColors, b: &WaveColors| a.lo == b.lo && a.mi == b.mi && a.hi == b.hi;

    rsx! {
        div { class: "card",
            div { class: "grp", "THIS BROWSER ONLY" }
            div { class: "sr",
                div { class: "sl", b { "Theme" } span { "Night is the default." } }
                div { class: "seg",
                    button { class: if !l.day { "on" } else { "" }, onclick: move |_| set_look.call(Look { day: false, ..look() }), "NIGHT" }
                    button { class: if l.day { "on" } else { "" }, onclick: move |_| set_look.call(Look { day: true, ..look() }), "DAY" }
                }
            }
            div { class: "sr",
                div { class: "sl", b { "Row density" } span { "Compact rows hide the artist line in the library." } }
                div { class: "seg",
                    button { class: if !l.compact { "on" } else { "" }, onclick: move |_| set_look.call(Look { compact: false, ..look() }), "COMFORT" }
                    button { class: if l.compact { "on" } else { "" }, onclick: move |_| set_look.call(Look { compact: true, ..look() }), "COMPACT" }
                }
            }
            div { class: "sr",
                div { class: "sl", b { "Waveform colors" } span { "Pick a color scheme." } }
                div { class: "seg",
                    for m in ["BLUE", "RGB", "3 BAND"] {
                        button { key: "{m}", class: if mode == m && same(&c, &preset(m)) { "on" } else { "" }, onclick: move |_| set_colors.call(preset(m)), "{m}" }
                    }
                }
            }
            div { class: "grp", "WAVEFORM COLORS" }
            div { class: "wc",
                div { class: "wcl",
                    for (i , name) in band_names.iter().enumerate() {
                        button { key: "{i}", class: if band() == i { "bnd on" } else { "bnd" }, onclick: move |_| band.set(i),
                            i { style: "background:{cur[i]}" }
                            span { class: "nm", "{name}" }
                            span { class: "mono mut", "{cur[i]}" }
                        }
                    }
                    div { class: "sw2",
                        for hex in BAND_SWATCHES {
                            button { key: "{hex}", class: if cur[band()].eq_ignore_ascii_case(hex) { "on" } else { "" }, style: "background:{hex}", "aria-label": "{hex}", title: "{hex}",
                                onclick: move |_| {
                                    let mut n = colors();
                                    match band() {
                                        0 => n.lo = hex.to_string(),
                                        1 => n.mi = hex.to_string(),
                                        _ => n.hi = hex.to_string(),
                                    }
                                    n.mode = "3 BAND".into();
                                    set_colors.call(n);
                                } }
                        }
                    }
                    div { class: "seg",
                        for name in ["3 BAND", "WARM", "ICE", "MONO"] {
                            button { key: "{name}", class: if same(&c, &preset(name)) { "on" } else { "" }, onclick: move |_| set_colors.call(WaveColors { mode: "3 BAND".into(), ..preset(name) }), "{name}" }
                        }
                    }
                }
                div { class: "wpv",
                    svg { view_box: "0 0 400 100", preserve_aspect_ratio: "none", "aria-hidden": "true",
                        path { d: "{paths.lo}", fill: "{c.lo}" }
                        path { d: "{paths.mi}", fill: "{c.mi}" }
                        path { d: "{paths.hi}", fill: "{c.hi}" }
                    }
                }
            }
            div { class: "rn", "The colors apply to the library, the track drawer and the player cards in this browser. The library's small waveforms are drawn by the daemon and keep its colors." }
        }
    }
}

// ---------------------------------------------------------------- library source

#[component]
fn SourceSection() -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut refresh = use_signal(|| 0u32);
    let mut from = use_signal(String::new);
    let mut to = use_signal(String::new);
    let mut msg = use_signal(String::new);
    let mut confirm_reimport = use_signal(|| false);
    let mut range = use_signal(|| None::<(i64, i64)>);

    let c = client.clone();
    let cfg = use_resource(move || {
        let _ = refresh();
        let c = c.clone();
        async move {
            c.settings()
                .await
                .map(|s| s.full)
                .map_err(|e| e.to_string())
        }
    });
    let loaded = cfg.read().clone();
    let full = match &loaded {
        Some(Ok(f)) => f.clone(),
        Some(Err(e)) => return rsx! { div { class: "errbox", "Could not read the settings: {e}" } },
        None => return rsx! { div { class: "card", span { class: "mut", "Loading…" } } },
    };
    let saved = settings::bpm_range(&full);
    let (lo, hi) = range().unwrap_or(saved);
    let dirty = (lo, hi) != saved;

    let c = client.clone();
    let save_range = move |_| {
        let c = c.clone();
        spawn(async move {
            // Start from the daemon's current config, not the one this screen loaded.
            let res = async {
                let current = c.settings().await.map_err(|e| e.to_string())?.full;
                let body = json!({ "full": settings::with_bpm_range(&current, lo, hi) });
                c.update_settings(&body).await.map_err(|e| e.to_string())
            }
            .await;
            match res {
                Ok(_) => {
                    range.set(None);
                    msg.set(format!("ANALYSIS RANGE SET TO {lo} - {hi} BPM."));
                    refresh.set(refresh() + 1);
                }
                Err(e) => msg.set(format!("COULD NOT SAVE THE RANGE: {e}")),
            }
        });
    };
    let c = client.clone();
    let replace = move |_| {
        let (f, t) = (from().trim().to_string(), to().trim().to_string());
        if f.is_empty() || t.is_empty() {
            return;
        }
        let c = c.clone();
        spawn(async move {
            match c.remap_paths(&f, &t).await {
                Ok(r) => msg.set(format!(
                    "{} TRACKS CHANGED, {} STILL MISSING.",
                    r.changed, r.missing
                )),
                Err(e) => msg.set(format!("REPLACE FAILED: {e}")),
            }
        });
    };
    let c = client.clone();
    let reimport = move |_| {
        if !confirm_reimport() {
            confirm_reimport.set(true);
            return;
        }
        confirm_reimport.set(false);
        let c = c.clone();
        spawn(async move {
            match c.reimport_tracks(&[]).await {
                Ok(n) => msg.set(format!("{n} TRACKS READ AGAIN.")),
                Err(e) => msg.set(format!("RE-IMPORT FAILED: {e}")),
            }
        });
    };

    rsx! {
        div { class: "card",
            div { class: "grp", "ANALYSIS" }
            div { class: "sr",
                div { class: "sl", b { "Analysis BPM range" } span { "Set 90 to 180 for drum and bass. Range is 60 to 200." } }
                div { class: "rng2",
                    input { class: "in", style: "width:90px;flex:none", r#type: "number", min: "60", max: "200", step: "5", value: "{lo}", "aria-label": "Lowest BPM",
                        onchange: move |e| if let Ok(v) = e.value().parse::<i64>() { range.set(Some(settings::move_range_end(lo, hi, End::Min, v))) } }
                    span { class: "mono", "-" }
                    input { class: "in", style: "width:90px;flex:none", r#type: "number", min: "60", max: "200", step: "5", value: "{hi}", "aria-label": "Highest BPM",
                        onchange: move |e| if let Ok(v) = e.value().parse::<i64>() { range.set(Some(settings::move_range_end(lo, hi, End::Max, v))) } }
                }
                button { class: "btn on", disabled: !dirty, onclick: save_range, "SAVE" }
            }
            div { class: "grp", "PATHS" }
            div { class: "sr",
                div { class: "sl", b { "Replace a path prefix" } span { "Fix tracks after a move. The change touches the library only." } }
                input { class: "in mono", r#type: "text", placeholder: "/old/music", "aria-label": "From", value: "{from()}", oninput: move |e| from.set(e.value()) }
                input { class: "in mono", r#type: "text", placeholder: "/music", "aria-label": "To", value: "{to()}", oninput: move |e| to.set(e.value()) }
                button { class: "btn", disabled: from().trim().is_empty() || to().trim().is_empty(), onclick: replace, "REPLACE" }
            }
            div { class: "sr",
                div { class: "sl", b { "Re-import tracks" } span { "Read the file tags again for all tracks. This overwrites metadata you edited here." } }
                if confirm_reimport() { button { class: "btn", onclick: move |_| confirm_reimport.set(false), "CANCEL" } }
                button { class: if confirm_reimport() { "btn dg" } else { "btn" }, onclick: reimport, if confirm_reimport() { "CONFIRM RE-IMPORT" } else { "RE-IMPORT" } }
            }
            if !msg().is_empty() { div { class: "rn mono", "{msg()}" } }
            div { class: "rn", "The link mode, the network interface, the music folder and lazy analysis are options the daemon is started with (--mode, --interface, --music-dir, --lazy-analysis). The API does not expose them, so they cannot be changed here." }
        }
    }
}

// ---------------------------------------------------------------- overlay link

#[component]
fn OverlayLink(base: String, goto: EventHandler<Tab>) -> Element {
    let url = overlay::url(&base);
    rsx! {
        div { class: "card",
            div { class: "grp", "NOW-PLAYING OVERLAY FOR STREAMING" }
            div { class: "sr",
                div { class: "sl", b { "Browser source address" } span { "Add it in OBS with a transparent background." } }
                input { class: "in mono", r#type: "text", readonly: true, value: "{url}", "aria-label": "Overlay address" }
            }
            div { class: "sr",
                div { class: "sl", b { "Appearance and sections" } span { "Position, style, accent color, waveform and lists. Changes apply live." } }
                button { class: "btn on", onclick: move |_| goto.call(Tab::Overlay), "OPEN THE OVERLAY PAGE" }
            }
        }
    }
}

// ---------------------------------------------------------------- player menu

#[component]
fn MenuSection() -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut refresh = use_signal(|| 0u32);
    let mut msg = use_signal(String::new);
    let mut confirm = use_signal(|| false);

    let c = client.clone();
    let data = use_resource(move || {
        let _ = refresh();
        let c = c.clone();
        async move { c.menu_items().await.map_err(|e| e.to_string()) }
    });
    let loaded = data.read().clone();
    let menu: MenuItems = match &loaded {
        Some(Ok(m)) => m.clone(),
        Some(Err(e)) => {
            return rsx! { div { class: "errbox", "The player menu is not available: {e}" } }
        }
        None => return rsx! { div { class: "card", span { class: "mut", "Loading…" } } },
    };

    let push = {
        let client = client.clone();
        use_callback(move |patch: MenuItemsPatch| {
            let c = client.clone();
            spawn(async move {
                match c.update_menu_items(&patch).await {
                    Ok(()) => {
                        msg.set(String::new());
                        refresh.set(refresh() + 1);
                    }
                    Err(e) => msg.set(format!("COULD NOT SAVE THE MENU: {e}")),
                }
            });
        })
    };
    let n = menu.items.len();
    let items = menu.items.clone();
    let reset = {
        let c = client.clone();
        move |_| {
            if !confirm() {
                confirm.set(true);
                return;
            }
            confirm.set(false);
            let c = c.clone();
            spawn(async move {
                match c.reset_menu_items().await {
                    Ok(()) => refresh.set(refresh() + 1),
                    Err(e) => msg.set(format!("COULD NOT RESET THE MENU: {e}")),
                }
            });
        }
    };

    rsx! {
        div { class: "card",
            div { class: "grp", "PLAYER ROOT MENU" }
            for (i , m) in menu.items.iter().enumerate() {
                {
                    let items_up = items.clone();
                    let items_dn = items.clone();
                    let items_tg = items.clone();
                    let visible = m.visible;
                    let locked = m.locked;
                    rsx! {
                        div { key: "{m.key}", class: "mi",
                            span { class: "o", "{i + 1}" }
                            span { class: "n", "{m.label}" }
                            button { class: "ib", "aria-label": "Move up", disabled: i == 0, onclick: move |_| push.call(MenuItemsPatch { items: Some(swap(&items_up, i, i.wrapping_sub(1))), track_detail: None }), "▲" }
                            button { class: "ib", "aria-label": "Move down", disabled: i + 1 == n, onclick: move |_| push.call(MenuItemsPatch { items: Some(swap(&items_dn, i, i + 1)), track_detail: None }), "▼" }
                            button { class: if visible { "tg on" } else { "tg" }, disabled: locked, title: if locked { "This entry is always shown" } else { "Show in menu" }, "aria-label": "Show in menu",
                                onclick: move |_| {
                                    let mut next = items_tg.clone();
                                    next[i].visible = !visible;
                                    push.call(MenuItemsPatch { items: Some(next), track_detail: None });
                                },
                                i {} }
                        }
                    }
                }
            }
            div { class: "grp", "TRACK DETAIL COLUMN" }
            div { class: "sr",
                div { class: "sl", b { "Field shown beside each track" } }
                select { class: "in", "aria-label": "Track detail field", value: "{menu.track_detail}",
                    onchange: move |e| push.call(MenuItemsPatch { items: None, track_detail: Some(e.value()) }),
                    for f in menu.track_detail_fields.iter() {
                        option { key: "{f.key}", value: "{f.key}", selected: f.key == menu.track_detail, "{f.label}" }
                    }
                }
            }
            div { class: "sr",
                div { class: "sl", b { "Restore the default menu" } }
                if confirm() { button { class: "btn", onclick: move |_| confirm.set(false), "CANCEL" } }
                button { class: "btn dg", onclick: reset, if confirm() { "CONFIRM RESET" } else { "RESET MENU" } }
            }
            if !msg().is_empty() { div { class: "rn mono", "{msg()}" } }
        }
    }
}

/// A copy of `items` with entries `a` and `b` swapped (unchanged if either is out of range).
fn swap(items: &[MenuItem], a: usize, b: usize) -> Vec<MenuItem> {
    let mut v = items.to_vec();
    if a < v.len() && b < v.len() {
        v.swap(a, b);
    }
    v
}

// ---------------------------------------------------------------- player settings

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayerPart {
    /// MYSETTING and MYSETTING2: editable.
    Editable,
    Readonly(Sub),
}

#[component]
fn PlayerSettings(sub: PlayerPart) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut refresh = use_signal(|| 0u32);
    let mut msg = use_signal(String::new);

    let c = client.clone();
    let data = use_resource(move || {
        let _ = refresh();
        let c = c.clone();
        async move {
            let s = c.settings().await.map_err(|e| e.to_string())?;
            let enums = c.settings_enums().await.map_err(|e| e.to_string())?;
            Ok::<_, String>((s.full, enums))
        }
    });
    let loaded = data.read().clone();
    let (full, enums): (Value, BTreeMap<String, Vec<String>>) = match &loaded {
        Some(Ok((f, e))) => (f.clone(), e.clone()),
        Some(Err(e)) => {
            return rsx! { div { class: "errbox", "Could not read the player settings: {e}" } }
        }
        None => return rsx! { div { class: "card", span { class: "mut", "Loading…" } } },
    };

    // Read the config again at the moment of the edit: the whole document is
    // replaced on each write, so a copy from an earlier render could undo a
    // change made since (by the players, or from another screen).
    let set = {
        let client = client.clone();
        use_callback(move |(key, value): (String, String)| {
            let c = client.clone();
            spawn(async move {
                let res = async {
                    let current = c.settings().await.map_err(|e| e.to_string())?.full;
                    let next = settings::with_value(&current, &key, &value)
                        .ok_or_else(|| format!("unknown setting {key}"))?;
                    c.update_settings(&json!({ "full": next }))
                        .await
                        .map_err(|e| e.to_string())
                }
                .await;
                match res {
                    Ok(_) => msg.set(String::new()),
                    Err(e) => msg.set(format!("COULD NOT SAVE: {e}")),
                }
                refresh.set(refresh() + 1);
            });
        })
    };

    match sub {
        PlayerPart::Editable => {
            let groups = settings::player_settings(&full);
            rsx! {
                div { class: "card",
                    for (g , rows) in groups.iter() {
                        div { key: "{g.label()}", class: "grp", "{g.label()}" }
                        for (key , value) in rows.iter() {
                            {
                                let opts = enums.get(key).cloned().unwrap_or_default();
                                rsx! {
                                    div { key: "{key}", class: "sr",
                                        div { class: "sl", b { "{settings::key_label(key)}" } }
                                        if opts.is_empty() {
                                            span { class: "mono mut", "{value}" }
                                        } else {
                                            div { class: "seg",
                                                for o in opts.iter() {
                                                    {
                                                        let (key, o2) = (key.clone(), o.clone());
                                                        rsx! { button { key: "{o}", class: if o == value { "on" } else { "" }, onclick: move |_| set.call((key.clone(), o2.clone())), "{settings::option_label(o)}" } }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if groups.is_empty() { span { class: "mut", "The daemon returned no player settings." } }
                    if !msg().is_empty() { div { class: "rn mono", "{msg()}" } }
                }
            }
        }
        PlayerPart::Readonly(part) => {
            let rows = settings::entries(&full, part);
            let (name, note) = match part {
                Sub::Djm => ("DJMMYSETTING.DAT", "Mixer settings. They come from the USB export or the rekordbox import. Import with the --import-settings option."),
                _ => ("DEVSETTING", "Waveform and key display, sent to the players in 6 bytes. Byte 0 and byte 3 are fixed protocol values and are not shown."),
            };
            rsx! {
                div { class: "card",
                    div { class: "grp", "{name}  ", span { class: "rof", "READ ONLY" } }
                    dl { class: "ro",
                        for (k , v) in rows.iter() { dt { key: "{k}", "{k}" } dd { "{v}" } }
                    }
                    div { class: "rn", "{note}" }
                }
            }
        }
    }
}
