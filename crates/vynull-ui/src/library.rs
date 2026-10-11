//! Library screen: sources, search with suggestions, tag filter, a virtual
//! track list with configurable columns, and loading a track onto a deck.
//! The track drawer (waveform, grid, cues, edits) and the queue come later.

use std::collections::HashSet;
use std::rc::Rc;

use dioxus::prelude::*;
use vynull_api::{LoadRequest, NewPlaylist, PlaylistInfo, TrackInfo};
use vynull_core::columns::{Col, Columns};
use vynull_core::key;
use vynull_core::palette;
use vynull_core::queue::{total_label, Queue};
use vynull_core::route::{Page, Route, Source};
use vynull_core::search::{fold, Recent};
use vynull_core::sort::{self, Sort};
use vynull_platform::{time, StoreExt};

use crate::drawer::{DeckBtn, Drawer};
use crate::library_data::{LibState, Load};
use crate::library_tools::{NewKind, NewPlaylistPanel, ToolsPanel};
use crate::live::Feed;
use crate::shell::{ClientCtx, LookCtx, Platform};

/// Rows drawn: enough for a tall window. The list is not measured.
const VIEW_H: f64 = 1600.0;
const OVERSCAN: usize = 6;

/// What a queue button on a row does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QAction {
    Toggle,
    Up,
    Down,
    Remove,
}

fn header_label(c: Col, queue_view: bool) -> &'static str {
    if c == Col::Load && queue_view {
        "ACTIONS"
    } else {
        c.header()
    }
}

#[derive(Clone, PartialEq)]
struct Toast {
    msg: String,
    /// `(track id, deck)` waiting for the user to confirm loading onto a playing deck.
    ask: Option<(i64, i64)>,
}

#[derive(Clone, PartialEq)]
struct SugView {
    field: &'static str,
    before: String,
    mid: String,
    after: String,
    count: String,
    value: String,
}

fn art_style(n: i64) -> String {
    let h = (n * 47 + 7) % 360;
    format!(
        "background:linear-gradient(135deg,hsl({h},42%,34%),hsl({},42%,16%))",
        (h + 50) % 360
    )
}

fn mmss(secs: i64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Playlists in tree order with their depth. Orphans (a missing parent) show at the top level.
fn flatten(pls: &[PlaylistInfo]) -> Vec<(PlaylistInfo, usize)> {
    let ids: HashSet<i64> = pls.iter().map(|p| p.id).collect();
    let mut out = Vec::new();
    fn walk(
        pls: &[PlaylistInfo],
        ids: &HashSet<i64>,
        parent: i64,
        depth: usize,
        out: &mut Vec<(PlaylistInfo, usize)>,
    ) {
        let mut kids: Vec<&PlaylistInfo> = pls
            .iter()
            .filter(|p| {
                if parent == 0 {
                    p.parent_id == 0 || !ids.contains(&p.parent_id)
                } else {
                    p.parent_id == parent
                }
            })
            .collect();
        kids.sort_by_key(|p| (p.sort_order, p.id));
        for p in kids {
            out.push((p.clone(), depth));
            if p.is_folder && depth < 8 {
                walk(pls, ids, p.id, depth + 1, out);
            }
        }
    }
    walk(pls, &ids, 0, 0, &mut out);
    out
}

#[component]
pub fn Library(feed: Feed, lib: Signal<LibState>, route: Signal<Route>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let platform = use_context::<Platform>();
    let row_h = use_context::<LookCtx>().look.read().row_px();

    let source = use_memo(move || route().source);
    let mut q = use_signal(String::new);
    let mut sug_open = use_signal(|| false);
    let mut hi = use_signal(|| 0usize);
    let sel = use_memo(move || route().track);
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut tag_sel = use_signal(Vec::<i64>::new);
    let mut tag_all = use_signal(|| false);
    let mut cols_open = use_signal(|| false);
    let mut load_open = use_signal(|| None::<i64>);
    let mut toast = use_signal(|| None::<Toast>);
    let mut toast_gen = use_signal(|| 0u32);
    let mut tools_open = use_signal(|| false);
    let mut new_kind = use_signal(|| None::<NewKind>);
    let mut drag = use_signal(|| None::<Col>);
    // (column, pointer x at the start, width at the start) while resizing.
    let mut resizing = use_signal(|| None::<(Col, f64, u32)>);

    let mut cols = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<Columns>("columns")
                .ok()
                .flatten()
                .unwrap_or_default()
                .normalized()
        }
    });
    let mut sort = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<Sort>("sort")
                .ok()
                .flatten()
                .unwrap_or_default()
        }
    });
    let mut recent = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<Recent>("recent")
                .ok()
                .flatten()
                .unwrap_or_default()
        }
    });
    {
        let store = platform.store.clone();
        use_effect(move || {
            let _ = store.set_json("columns", &cols());
        });
        let store = platform.store.clone();
        use_effect(move || {
            let _ = store.set_json("sort", &sort());
        });
        let store = platform.store.clone();
        use_effect(move || {
            let _ = store.set_json("recent", &recent());
        });
    }

    let mut queue = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<Queue>("queue")
                .ok()
                .flatten()
                .unwrap_or_default()
        }
    });
    {
        let store = platform.store.clone();
        use_effect(move || {
            let _ = store.set_json("queue", &queue());
        });
    }
    let go_source = use_callback(move |s: Source| {
        route.set(Route {
            page: Page::Library,
            source: s,
            track: None,
        });
        scroll_top.set(0.0);
    });
    let select = use_callback(move |t: Option<i64>| {
        let mut r = *route.peek();
        r.track = t;
        route.set(r);
    });

    let mut flash = move |msg: String, ask: Option<(i64, i64)>| {
        let g = toast_gen() + 1;
        toast_gen.set(g);
        toast.set(Some(Toast { msg, ask }));
        if ask.is_none() {
            spawn(async move {
                time::sleep(std::time::Duration::from_millis(2600)).await;
                if toast_gen() == g {
                    toast.set(None);
                }
            });
        }
    };

    let c = client.clone();
    let do_load = use_callback(move |(track_id, deck, title): (i64, i64, String)| {
        let c = c.clone();
        spawn(async move {
            let req = LoadRequest {
                track_id: Some(track_id),
                file_path: None,
                device_number: deck,
            };
            match c.load(&req).await {
                Ok(()) => {
                    queue.write().mark_played(track_id);
                    flash(
                        format!("{} LOADED ON DECK {deck}", title.to_uppercase()),
                        None,
                    );
                }
                Err(e) => flash(format!("LOAD FAILED: {e}"), None),
            }
        });
    });

    // Rows and the drawer both load a track on a deck; a playing deck asks first.
    let request_load = use_callback(move |(id, deck): (i64, i64)| {
        let title = lib
            .read()
            .by_id
            .get(&id)
            .map(|&i| lib.read().tracks[i].title.clone())
            .unwrap_or_default();
        let playing = (feed.live)()
            .players
            .iter()
            .any(|p| p.device_number == deck && p.is_playing);
        if playing {
            flash(
                format!("DECK {deck} IS PLAYING. LOAD ANYWAY?"),
                Some((id, deck)),
            );
        } else {
            do_load.call((id, deck, title));
        }
    });

    let c = client.clone();
    let refresh_playlists = use_callback(move |_: ()| {
        let c = c.clone();
        spawn(async move {
            if let Ok(v) = c.playlists().await {
                lib.write().playlists = Rc::new(v);
            }
        });
    });

    // The next queued track goes on the first deck that is not playing, an empty one first.
    let queue_next = use_callback(move |_: ()| {
        let Some(id) = queue.peek().next_unplayed() else {
            flash("NO TRACK LEFT IN THE QUEUE".into(), None);
            return;
        };
        let players = (feed.live)().players;
        let deck = players
            .iter()
            .filter(|p| !p.is_playing)
            .min_by_key(|p| (p.track_id > 0 || !p.track_title.is_empty(), p.device_number))
            .map(|p| p.device_number);
        match deck {
            Some(d) => request_load.call((id, d)),
            None => flash("EVERY DECK IS PLAYING".into(), None),
        }
    });
    let c = client.clone();
    let queue_save = use_callback(move |_: ()| {
        let ids = queue.peek().ids.clone();
        if ids.is_empty() {
            flash("THE QUEUE IS EMPTY".into(), None);
            return;
        }
        let c = c.clone();
        spawn(async move {
            let name = format!("Queue {} tracks", ids.len());
            let made = c
                .create_playlist(&NewPlaylist {
                    name: name.clone(),
                    ..Default::default()
                })
                .await;
            let res = match made {
                Ok(p) => c.set_playlist_tracks(p.id, &ids).await,
                Err(e) => Err(e),
            };
            match res {
                Ok(()) => {
                    refresh_playlists.call(());
                    flash(
                        format!("QUEUE SAVED AS PLAYLIST {}", name.to_uppercase()),
                        None,
                    );
                }
                Err(e) => flash(format!("SAVE FAILED: {e}"), None),
            }
        });
    });

    // The order the user just gave a playlist, shown until the daemon's own answer arrives.
    let mut pl_order = use_signal(|| None::<(i64, Vec<i64>)>);

    // Tracks of the selected playlist, evaluated by the daemon (smart playlists too).
    let c = client.clone();
    let mut playlist_ids = use_resource(move || {
        let src = source();
        let c = c.clone();
        async move {
            match src {
                Source::Playlist(id) => c
                    .playlist_tracks(id)
                    .await
                    .ok()
                    .map(|v| v.into_iter().map(|t| t.id).collect::<Vec<_>>()),
                Source::Collection | Source::Queue | Source::History => None,
            }
        }
    });

    use_effect(move || {
        let _ = playlist_ids.read();
        pl_order.set(None);
    });

    // Today's plays, newest first, as library tracks.
    let c = client.clone();
    let history = use_resource(move || {
        let _ = source();
        let c = c.clone();
        async move { c.history().await.unwrap_or_default() }
    });
    let history_today = use_memo(move || -> Vec<i64> {
        let st = lib.read();
        let today = time::local_date();
        let mut items: Vec<_> = history
            .read()
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|h| h.started_at.starts_with(&today))
            .collect();
        items.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        let mut seen = HashSet::new();
        items
            .into_iter()
            .map(|h| h.track_id)
            .filter(|id| st.by_id.contains_key(id) && seen.insert(*id))
            .collect()
    });

    let visible = use_memo(move || {
        let st = lib.read();
        let by_ids = |ids: &[i64]| -> Vec<usize> {
            ids.iter()
                .filter_map(|id| st.by_id.get(id).copied())
                .collect()
        };
        let base: Vec<usize> = match (source(), &*playlist_ids.read()) {
            (Source::Playlist(pid), Some(Some(ids))) => match &*pl_order.read() {
                Some((oid, order)) if *oid == pid => by_ids(order),
                _ => by_ids(ids),
            },
            (Source::Playlist(_), _) => Vec::new(),
            (Source::Queue, _) => by_ids(&queue().ids),
            (Source::History, _) => by_ids(&history_today()),
            (Source::Collection, _) => (0..st.tracks.len()).collect(),
        };
        let hits = st.index.filter(&q());
        let mut mask = vec![false; st.tracks.len()];
        for i in hits {
            mask[i] = true;
        }
        let wanted: Vec<&str> = st
            .tags
            .iter()
            .filter(|t| tag_sel().contains(&t.id))
            .map(|t| t.name.as_str())
            .collect();
        let mut out: Vec<usize> = base
            .into_iter()
            .filter(|&i| mask[i])
            .filter(|&i| {
                let have = &st.tracks[i].tags;
                wanted.is_empty()
                    || if tag_all() {
                        wanted.iter().all(|w| have.iter().any(|h| h == w))
                    } else {
                        wanted.iter().any(|w| have.iter().any(|h| h == w))
                    }
            })
            .collect();
        // The queue keeps the order the user gave it.
        if source() != Source::Queue {
            sort.read().apply(&st.tracks, &mut out);
        }
        out
    });

    // Playlist order: only for a plain playlist, and only while the list shows it as it is
    // stored (no sort, search or tag filter), since the order edited is the stored one.
    let reorder_ok = use_memo(move || {
        let Source::Playlist(pid) = source() else {
            return false;
        };
        let editable = lib
            .read()
            .playlists
            .iter()
            .any(|p| p.id == pid && !p.is_smart && !p.is_folder && !p.read_only);
        editable && sort.read().is_empty() && q().trim().is_empty() && tag_sel().is_empty()
    });
    let c_order = client.clone();
    let move_in_playlist = use_callback(move |(id, delta): (i64, isize)| {
        let Source::Playlist(pid) = source() else {
            return;
        };
        let mut ids: Vec<i64> = {
            let st = lib.read();
            visible().iter().map(|&i| st.tracks[i].id).collect()
        };
        let Some(at) = ids.iter().position(|x| *x == id) else {
            return;
        };
        let to = at as isize + delta;
        if to < 0 || to >= ids.len() as isize {
            return;
        }
        ids.swap(at, to as usize);
        pl_order.set(Some((pid, ids.clone())));
        let c = c_order.clone();
        spawn(async move {
            match c.set_playlist_tracks(pid, &ids).await {
                // Read it back: the daemon's answer replaces the order shown meanwhile.
                Ok(()) => playlist_ids.restart(),
                Err(e) => {
                    pl_order.set(None);
                    flash(format!("COULD NOT SAVE THE ORDER: {e}"), None);
                }
            }
        });
    });

    let sugs = use_memo(move || -> Vec<SugView> {
        if !sug_open() {
            return Vec::new();
        }
        let st = lib.read();
        let query = q();
        if fold(&query).trim().is_empty() {
            return recent()
                .top(3)
                .into_iter()
                .map(|e| SugView {
                    field: "RECENT",
                    before: e.value.clone(),
                    mid: String::new(),
                    after: String::new(),
                    count: String::new(),
                    value: e.value.clone(),
                })
                .collect();
        }
        st.index
            .suggest(&st.tracks, &query)
            .into_iter()
            .map(|s| {
                let (before, mid, after) = s.highlight(&query);
                SugView {
                    field: s.field.label(),
                    before,
                    mid,
                    after,
                    count: s.count.to_string(),
                    value: s.value,
                }
            })
            .collect()
    });

    let pick_sug = use_callback(move |value: String| {
        recent.write().add(&value);
        q.set(value);
        sug_open.set(false);
        hi.set(0);
        scroll_top.set(0.0);
    });

    let state = lib.read().clone();
    let total = state.tracks.len();
    let flat = flatten(&state.playlists);
    let rows_idx = visible();
    let shown = rows_idx.len();
    let missing = rows_idx
        .iter()
        .filter(|&&i| state.tracks[i].file_missing)
        .count();
    let query = q();
    let has_q = !query.is_empty();
    let hi_i = hi().min(sugs.read().len().saturating_sub(1));

    let live = (feed.live)();
    let master_key = live
        .players
        .iter()
        .find(|p| p.is_master)
        .map(|p| p.key.clone())
        .unwrap_or_default();
    let decks_all: Vec<DeckBtn> = live
        .players
        .iter()
        .map(|p| DeckBtn {
            n: p.device_number,
            cls: if p.is_playing {
                "dkb pl"
            } else if p.track_id <= 0 && p.track_title.is_empty() {
                "dkb em"
            } else {
                "dkb"
            },
            title: format!(
                "Load on deck {}, {}",
                p.device_number,
                if p.play_state_name.is_empty() {
                    "idle"
                } else {
                    &p.play_state_name
                }
            ),
        })
        .collect();

    let first = ((scroll_top() / row_h) as usize)
        .saturating_sub(OVERSCAN)
        .min(shown);
    let last = (first + (VIEW_H / row_h) as usize + 2 * OVERSCAN).min(shown);

    let tag_dots = |t: &TrackInfo| -> Vec<String> {
        t.tags
            .iter()
            .filter_map(|n| state.tags.iter().find(|g| &g.name == n))
            .map(|g| palette::tag_color(g.color, g.id))
            .collect()
    };

    let cols_now = cols();
    // Twice the on-screen size for sharp thumbnails, in steps so that dragging a column edge does not refetch every pixel.
    let wave_w = (cols_now.width(Col::Overview).unwrap_or(130) * 2).div_ceil(80) * 80;
    let wave_h = (row_h as u32).saturating_sub(8).max(14) * 2;
    let source_now = source();
    let queue_view = source_now == Source::Queue;
    let col_style = cols_now.css_vars(if queue_view || reorder_ok() { 200 } else { 104 });
    let queue_now = queue();
    let queued_secs: i64 = queue_now
        .ids
        .iter()
        .filter_map(|id| state.by_id.get(id))
        .map(|&i| state.tracks[i].duration)
        .sum();
    let today_count = history_today().len();

    rsx! {
        div { class: "lib",
            div { class: "lay",
                aside { class: "side",
                    button {
                        class: if queue_view { "src on" } else { "src" },
                        onclick: move |_| go_source.call(Source::Queue),
                        svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M4 6h11M4 12h7M4 18h7M17 10v8M13 14h8" } }
                        span { "QUEUE" }
                        span { class: "n", "{queue_now.ids.len()}" }
                    }
                    button {
                        class: if source_now == Source::Collection { "src on" } else { "src" },
                        onclick: move |_| go_source.call(Source::Collection),
                        span { "COLLECTION" }
                        span { class: "n", "{total}" }
                    }
                    div { class: "sec", "PLAYLISTS" }
                    for (p , depth) in flat.iter() {
                        {
                            let id = p.id;
                            let folder = p.is_folder;
                            let on = source_now == Source::Playlist(id);
                            let label = format!(
                                "{}{}",
                                if p.is_smart { "SMART  " } else if folder { "FOLDER  " } else { "" },
                                p.name.to_uppercase()
                            );
                            let count = if folder || p.is_smart { String::new() } else { p.track_ids.len().to_string() };
                            rsx! {
                                button {
                                    key: "{id}",
                                    class: if on { "src on" } else { "src" },
                                    style: "padding-left:{16 + 20 * depth}px",
                                    onclick: move |_| {
                                        if !folder {
                                            go_source.call(Source::Playlist(id));
                                        }
                                    },
                                    span { "{label}" }
                                    span { class: "n", "{count}" }
                                }
                            }
                        }
                    }
                    div { class: "sec", "HISTORY" }
                    button {
                        class: if source_now == Source::History { "src on" } else { "src" },
                        onclick: move |_| go_source.call(Source::History),
                        span { "TODAY" }
                        span { class: "n", "{today_count}" }
                    }
                    div { class: "sfoot",
                        button { class: "btn ib", title: "New playlist", "aria-label": "New playlist",
                            onclick: move |_| { tools_open.set(false); new_kind.set(Some(NewKind::Playlist)); },
                            svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M4 6h11M4 11h11M4 16h6M18 13v6M15 16h6" } } }
                        button { class: "btn ib", title: "New smart playlist", "aria-label": "New smart playlist",
                            onclick: move |_| { tools_open.set(false); new_kind.set(Some(NewKind::Smart)); },
                            svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M10 4l1.6 4.4L16 10l-4.4 1.6L10 16l-1.6-4.4L4 10l4.4-1.6zM18 14v6M15 17h6" } } }
                        button { class: "btn ib", title: "New folder", "aria-label": "New folder",
                            onclick: move |_| { tools_open.set(false); new_kind.set(Some(NewKind::Folder)); },
                            svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2zM12 10.5v5M9.5 13h5" } } }
                    }
                }
                section { class: "main",
                    div { class: "bar",
                        div { class: "search",
                            svg {
                                width: "18",
                                height: "18",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "#9AA4B2",
                                stroke_width: "2.2",
                                stroke_linecap: "round",
                                circle { cx: "11", cy: "11", r: "7" }
                                path { d: "M20 20l-4-4" }
                            }
                            input {
                                r#type: "text",
                                value: "{query}",
                                placeholder: "Search title, artist, album, tag, comment",
                                autocomplete: "off",
                                oninput: move |e| {
                                    q.set(e.value());
                                    sug_open.set(true);
                                    hi.set(0);
                                    scroll_top.set(0.0);
                                },
                                onfocus: move |_| sug_open.set(true),
                                onclick: move |_| sug_open.set(true),
                                onblur: move |_| sug_open.set(false),
                                onkeydown: move |e| {
                                    let n = sugs.read().len();
                                    match e.key() {
                                        Key::ArrowDown if n > 0 => {
                                            e.prevent_default();
                                            hi.set((hi_i + 1) % n);
                                        }
                                        Key::ArrowUp if n > 0 => {
                                            e.prevent_default();
                                            hi.set((hi_i + n - 1) % n);
                                        }
                                        Key::Enter if n > 0 => {
                                            e.prevent_default();
                                            let v = sugs.read()[hi_i].value.clone();
                                            pick_sug.call(v);
                                        }
                                        Key::Enter if !q().is_empty() => {
                                            e.prevent_default();
                                            recent.write().add(&q());
                                            sug_open.set(false);
                                        }
                                        Key::Escape => sug_open.set(false),
                                        _ => {}
                                    }
                                },
                            }
                            if has_q {
                                button {
                                    class: "clr",
                                    "aria-label": "Clear search",
                                    onclick: move |_| {
                                        q.set(String::new());
                                        sug_open.set(false);
                                        hi.set(0);
                                    },
                                    svg {
                                        width: "14",
                                        height: "14",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2.6",
                                        stroke_linecap: "round",
                                        path { d: "M6 6l12 12M18 6L6 18" }
                                    }
                                }
                            }
                            if !sugs.read().is_empty() {
                                div { class: "sug", role: "listbox",
                                    for (i , s) in sugs.read().iter().enumerate() {
                                        button {
                                            key: "{s.field}{s.value}",
                                            class: if i == hi_i { "si hi" } else { "si" },
                                            role: "option",
                                            // mousedown, not click: the input would blur first and close the list.
                                            onmousedown: {
                                                let v = s.value.clone();
                                                move |e| {
                                                    e.prevent_default();
                                                    pick_sug.call(v.clone());
                                                }
                                            },
                                            span { class: "f", "{s.field}" }
                                            span { class: "v", "{s.before}", b { "{s.mid}" }, "{s.after}" }
                                            span { class: "c", "{s.count}" }
                                        }
                                    }
                                    div { class: "sugf", "UP AND DOWN TO MOVE  ENTER TO PICK  ESC TO CLOSE" }
                                }
                            }
                        }
                        div { style: "position:relative",
                            button {
                                class: if cols_open() { "btn ib on" } else { "btn ib" },
                                title: "Columns",
                                "aria-label": "Columns",
                                onclick: move |_| cols_open.toggle(),
                                svg {
                                    width: "22",
                                    height: "22",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M4 5h16v14H4zM9.3 5v14M14.7 5v14" }
                                }
                            }
                            if cols_open() {
                                div { class: "colp",
                                    div { class: "h", "SHOW, HIDE AND ORDER" }
                                    for c in cols_now.order.iter().copied() {
                                        div { key: "{c.css()}", class: "cr",
                                            span { class: "n", "{c.panel_name()}" }
                                            button {
                                                class: "btn ib",
                                                "aria-label": "Move the column left",
                                                onclick: move |_| cols.write().step(c, -1),
                                                "▲"
                                            }
                                            button {
                                                class: "btn ib",
                                                "aria-label": "Move the column right",
                                                onclick: move |_| cols.write().step(c, 1),
                                                "▼"
                                            }
                                            button {
                                                class: if cols_now.is_hidden(c) { "tg" } else { "tg on" },
                                                disabled: c == Col::Track,
                                                "aria-label": "Show the column",
                                                onclick: move |_| cols.write().toggle(c),
                                                i {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        span { class: "sp" }
                        button {
                            class: if tools_open() { "btn ib on" } else { "btn ib" },
                            title: "Import and export",
                            "aria-label": "Import and export",
                            onclick: move |_| { new_kind.set(None); tools_open.toggle(); },
                            svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "M8 4v12M4 12l4 4 4-4M16 20V8M12 12l4-4 4 4" } }
                        }
                    }
                    if tools_open() {
                        ToolsPanel {
                            playlists: state.playlists.to_vec(),
                            on_flash: move |m: String| flash(m, None),
                        }
                    }
                    if let Some(kind) = new_kind() {
                        NewPlaylistPanel {
                            key: "{kind == NewKind::Smart}{kind == NewKind::Folder}",
                            kind,
                            tags: state.tags.to_vec(),
                            on_cancel: move |_| new_kind.set(None),
                            on_flash: move |m: String| flash(m, None),
                            on_done: move |p: PlaylistInfo| {
                                new_kind.set(None);
                                refresh_playlists.call(());
                                if !p.is_folder {
                                    go_source.call(Source::Playlist(p.id));
                                }
                                flash(format!("CREATED {}", p.name.to_uppercase()), None);
                            },
                        }
                    }
                    div { class: "tgs",
                        button {
                            class: "tag md",
                            title: "Match any selected tag, or all of them",
                            onclick: move |_| tag_all.toggle(),
                            if tag_all() { "ALL" } else { "ANY" }
                        }
                        for t in state.tags.iter() {
                            {
                                let id = t.id;
                                let on = tag_sel().contains(&id);
                                let color = palette::tag_color(t.color, id);
                                // Both states set the same properties: when a style string changes,
                                // the renderer does not remove properties that the new one omits.
                                let style = if on {
                                    format!("background:{color};border-color:{color};color:#05070A")
                                } else {
                                    format!("background:var(--surface);border-color:{};color:var(--text)", palette::tag_color_dim(t.color, id))
                                };
                                let dot = if on { "background:#05070A".to_string() } else { format!("background:{color}") };
                                rsx! {
                                    button {
                                        key: "{id}",
                                        class: "tag",
                                        style: "{style}",
                                        onclick: move |_| {
                                            let mut v = tag_sel.write();
                                            if let Some(p) = v.iter().position(|x| *x == id) {
                                                v.remove(p);
                                            } else {
                                                v.push(id);
                                            }
                                            drop(v);
                                            scroll_top.set(0.0);
                                        },
                                        i { style: "{dot}" }
                                        "{t.name}"
                                    }
                                }
                            }
                        }
                        span { class: "sp" }
                        span { class: "mono mut", style: "font-size:12px",
                            if queue_view { "{shown} OF {queue_now.ids.len()} QUEUED" } else { "{shown} OF {total} TRACKS" }
                            if missing > 0 { "  {missing} MISSING" }
                        }
                    }
                    if queue_view {
                        div { class: "qbar",
                            span { class: "qt",
                                svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                    path { d: "M4 6h11M4 12h7M4 18h7M17 10v8M13 14h8" } }
                                "QUEUE"
                            }
                            span { class: "mut", "{queue_now.ids.len()} TRACKS  {total_label(queued_secs)}" }
                            span { class: "sp" }
                            button { class: "btn ib", title: "Load the next track on a free deck", "aria-label": "Load the next track",
                                onclick: move |_| queue_next.call(()),
                                svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M5 4v16M9 4l11 8-11 8z" } } }
                            button { class: "btn ib", title: "Save the queue as a playlist", "aria-label": "Save the queue as a playlist",
                                onclick: move |_| queue_save.call(()),
                                svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M5 4h11l3 3v13H5zM8 4v5h7M8 20v-6h8v6" } } }
                            button { class: "btn ib", title: "Empty the queue", "aria-label": "Empty the queue",
                                onclick: move |_| queue.write().clear(),
                                svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" } } }
                        }
                    }
                    div {
                        class: "rows",
                        style: "{col_style}",
                        onscroll: move |e| scroll_top.set(e.data().scroll_top()),
                        div { class: "lh",
                            for c in cols_now.visible() {
                                span {
                                    key: "{c.css()}",
                                    class: "{c.class()} hc",
                                    draggable: "true",
                                    title: if sort::sortable(c) && !queue_view { "Click to sort, click again to reverse. Shift-click sets the second sort. Drag to reorder." } else { "Drag to reorder the column" },
                                    onclick: move |e| {
                                        if queue_view {
                                            return;
                                        }
                                        if e.modifiers().shift() {
                                            sort.write().click_secondary(c);
                                        } else {
                                            sort.write().click(c);
                                        }
                                        scroll_top.set(0.0);
                                    },
                                    ondragstart: move |_| drag.set(Some(c)),
                                    ondragover: move |e| e.prevent_default(),
                                    ondrop: move |e| {
                                        e.prevent_default();
                                        if let Some(from) = drag() {
                                            cols.write().move_to(from, c);
                                        }
                                        drag.set(None);
                                    },
                                    "{header_label(c, queue_view)}"
                                    if !queue_view {
                                        if let Some((rank, desc)) = sort.read().rank(c) {
                                            span { class: "so", "aria-label": if desc { "sorted descending" } else { "sorted ascending" },
                                                if desc { "▼" } else { "▲" }
                                                if rank == 2 { sub { "2" } }
                                            }
                                        }
                                    }
                                    if c.resizable() {
                                        span {
                                            class: "rz",
                                            title: "Drag to resize the column. Double click to reset.",
                                            draggable: "false",
                                            onclick: move |e| e.stop_propagation(),
                                            onpointerdown: move |e| {
                                                e.prevent_default();
                                                e.stop_propagation();
                                                let w = cols.read().width(c).unwrap_or(100);
                                                resizing.set(Some((c, e.client_coordinates().x, w)));
                                            },
                                            ondoubleclick: move |e| {
                                                e.stop_propagation();
                                                cols.write().reset_width(c);
                                            },
                                        }
                                    }
                                }
                            }
                        }
                        match &state.load {
                            Load::Loading => rsx! {
                                div { class: "none", span { "LOADING THE LIBRARY…" } }
                            },
                            Load::Failed(e) => rsx! {
                                div { class: "none", span { "COULD NOT LOAD THE LIBRARY" } span { class: "mut", "{e}" } }
                            },
                            Load::Ready if shown == 0 => rsx! {
                                div { class: "none",
                                    span {
                                        if has_q { "NO TRACK MATCHES" }
                                        else if queue_view { "THE QUEUE IS EMPTY. ADD TRACKS FROM THE COLLECTION." }
                                        else if source_now == Source::History { "NOTHING PLAYED TODAY" }
                                        else if matches!(source_now, Source::Playlist(_)) { "THIS PLAYLIST HAS NO TRACKS" }
                                        else { "NO TRACK MATCHES" }
                                    }
                                    if has_q {
                                        button {
                                            class: "btn",
                                            onclick: move |_| {
                                                q.set(String::new());
                                                sug_open.set(false);
                                            },
                                            "CLEAR SEARCH"
                                        }
                                    }
                                }
                            },
                            Load::Ready => rsx! {},
                        }
                        div { class: "spacer", style: "height:{shown as f64 * row_h}px",
                            div { class: "win", style: "top:{first as f64 * row_h}px",
                                for &i in &rows_idx[first..last] {
                                    {
                                        let t = &state.tracks[i];
                                        let id = t.id;
                                        let opened = load_open() == Some(id);
                                        let mt = state.index.match_in(&state.tracks, i, &query).unwrap_or_default();
                                        rsx! {
                                            Row {
                                                key: "{id}",
                                                track: t.clone(),
                                                selected: sel() == Some(id),
                                                mt,
                                                key_class: key::relation(&t.key, &master_key).class(),
                                                tint: palette::tint_vars(t.color_id),
                                                dots: tag_dots(t),
                                                decks: if opened { decks_all.clone() } else { Vec::new() },
                                                load_open: opened,
                                                in_queue: queue_now.contains(id),
                                                queue_view,
                                                reorder: reorder_ok(),
                                                wave_w: wave_w,
                                                wave_h: wave_h,
                                                played: queue_view && queue_now.is_played(id),
                                                on_q: move |a: QAction| {
                                                    if !queue_view {
                                                        match a {
                                                            QAction::Up => return move_in_playlist.call((id, -1)),
                                                            QAction::Down => return move_in_playlist.call((id, 1)),
                                                            _ => {}
                                                        }
                                                    }
                                                    let mut q = queue.write();
                                                    match a {
                                                        QAction::Toggle => q.toggle(id),
                                                        QAction::Up => q.step(id, -1),
                                                        QAction::Down => q.step(id, 1),
                                                        QAction::Remove => q.remove(id),
                                                    }
                                                },
                                                on_pick: move |_| select.call(Some(id)),
                                                on_ld: move |_| {
                                                    load_open.set(if load_open() == Some(id) { None } else { Some(id) });
                                                },
                                                on_load: move |deck: i64| {
                                                    load_open.set(None);
                                                    request_load.call((id, deck));
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(id) = sel() {
                if let Some(&i) = state.by_id.get(&id) {
                    Drawer {
                        key: "{id}",
                        track: state.tracks[i].clone(),
                        lib,
                        decks: decks_all.clone(),
                        on_close: move |_| select.call(None),
                        on_load: move |deck: i64| request_load.call((id, deck)),
                        on_flash: move |m: String| flash(m, None),
                        in_queue: queue_now.contains(id),
                        on_queue: move |_| queue.write().toggle(id),
                    }
                }
            }
            if let Some(t) = toast() {
                div { class: "toast",
                    span { "{t.msg}" }
                    if let Some((track, deck)) = t.ask {
                        button {
                            class: "btn on sm",
                            onclick: move |_| {
                                let title = lib.read().by_id.get(&track).map(|&i| lib.read().tracks[i].title.clone()).unwrap_or_default();
                                toast.set(None);
                                do_load.call((track, deck, title));
                            },
                            "LOAD"
                        }
                        button { class: "btn sm", onclick: move |_| toast.set(None), "CANCEL" }
                    }
                }
            }
            // While a column is being resized, a full-screen layer owns the pointer.
            if let Some((c, x0, w0)) = resizing() {
                div {
                    class: "rsz-shield",
                    onpointermove: move |e| {
                        let dx = e.client_coordinates().x - x0;
                        cols.write().set_width(c, (w0 as f64 + dx).round() as i64);
                    },
                    onpointerup: move |_| resizing.set(None),
                    onpointerleave: move |_| resizing.set(None),
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct RowProps {
    track: TrackInfo,
    selected: bool,
    mt: String,
    key_class: &'static str,
    tint: String,
    dots: Vec<String>,
    decks: Vec<DeckBtn>,
    load_open: bool,
    in_queue: bool,
    queue_view: bool,
    played: bool,
    /// Show the playlist's move up and down buttons.
    reorder: bool,
    /// Pixel size to ask the daemon for the row thumbnail at.
    wave_w: u32,
    wave_h: u32,
    on_q: EventHandler<QAction>,
    on_pick: EventHandler<()>,
    on_ld: EventHandler<()>,
    on_load: EventHandler<i64>,
}

#[component]
fn Row(props: RowProps) -> Element {
    let client = use_context::<ClientCtx>().0;
    let t = &props.track;
    let color = palette::color(t.color_id);
    let cdot = color.map_or(
        "background:none;border:1px dashed #6B7482".to_string(),
        |(_, hex)| format!("background:{hex}"),
    );
    // `/api/artwork/{id}` takes the track id. Art is extracted on first request, so
    // a track not yet checked may still have some; one known to have none is skipped.
    let mut art_failed = use_signal(|| false);
    let art = (!art_failed() && (t.art_id > 0 || !t.art_checked))
        .then(|| client.url(&format!("/api/artwork/{}", t.id)));
    // The daemon draws the row thumbnail from the CDJ's detail data in the
    // color mode and overview type it uses itself, as its own web UI asks for it.
    // `a=1` makes the URL change once the track is analysed, so a placeholder is not kept.
    let LookCtx {
        colors,
        daemon_wave,
        daemon_overview,
        locks,
        ..
    } = use_context::<LookCtx>();
    let locked = locks().has(t.id);
    let overview = if daemon_overview() == "half" {
        "half"
    } else {
        "full"
    };
    let wave = client.url(&format!(
        "/api/analysis/waveform-png/{}?type=detail&color={}&overview={overview}&w={}&h={}{}",
        t.id,
        colors().png_color(&daemon_wave()),
        props.wave_w,
        props.wave_h,
        if t.bpm > 0.0 { "&a=1" } else { "" }
    ));
    let border = if props.selected {
        "#FFC15A"
    } else {
        "transparent"
    };
    let style = format!("border-left-color:{border};{}", props.tint);
    let stars = t.rating.clamp(0, 5);
    let row_class = format!(
        "rw{}{}",
        if props.selected { " on" } else { "" },
        if props.played { " pd" } else { "" }
    );
    rsx! {
        div {
            class: "{row_class}",
            style: "{style}",
            role: "button",
            tabindex: "0",
            onclick: move |_| props.on_pick.call(()),
            span { class: "c-cl", i { class: "cdot", style: "{cdot}" } }
            div { class: "c-a", style: "{art_style(t.id)}",
                if let Some(url) = art { img { src: "{url}", alt: "", loading: "lazy", onerror: move |_| art_failed.set(true) } }
            }
            div { class: "c-t",
                div { class: "ttl",
                    "{t.title}"
                    if t.file_missing { span { class: "fl rd", "MISSING" } }
                    if locked { span { class: "fl am", title: "Grid and analysis locked in this client", "LOCKED" } }
                }
                div { class: "sub",
                    span { "{t.artist}" }
                    for (i , d) in props.dots.iter().enumerate() {
                        i { key: "{i}", style: "background:{d}" }
                    }
                }
                if !props.mt.is_empty() { div { class: "mt", "MATCH IN {props.mt}" } }
            }
            span { class: "c-cm", title: "{t.comment}", "{t.comment}" }
            div { class: "c-w", img { src: "{wave}", alt: "", loading: "lazy" } }
            span { class: "c-g", "{t.genre}" }
            span { class: "c-b mono", "{vynull_core::search::bpm_label(t.bpm)}" }
            span { class: "c-k", span { class: "{props.key_class}", "{t.key}" } }
            span { class: "c-m mono", "{mmss(t.duration)}" }
            span { class: "c-r",
                for s in 0..5 {
                    i { key: "{s}", class: if s < stars { "f" } else { "" } }
                }
            }
            span { class: "c-ld",
                if props.queue_view {
                    span { class: "ldb", role: "button", tabindex: "0", title: "Move up", "aria-label": "Move up",
                        onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Up); }, "▲" }
                    span { class: "ldb", role: "button", tabindex: "0", title: "Move down", "aria-label": "Move down",
                        onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Down); }, "▼" }
                    span { class: "ldb", role: "button", tabindex: "0", title: "Remove from the queue", "aria-label": "Remove from the queue",
                        onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Remove); }, "✕" }
                } else {
                    if props.reorder {
                        span { class: "ldb", role: "button", tabindex: "0", title: "Move up in the playlist", "aria-label": "Move up in the playlist",
                            onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Up); }, "▲" }
                        span { class: "ldb", role: "button", tabindex: "0", title: "Move down in the playlist", "aria-label": "Move down in the playlist",
                            onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Down); }, "▼" }
                    }
                    span {
                        class: if props.in_queue { "ldb on" } else { "ldb" },
                        role: "button",
                        tabindex: "0",
                        title: if props.in_queue { "Remove from the queue" } else { "Add to the queue" },
                        "aria-label": if props.in_queue { "Remove from the queue" } else { "Add to the queue" },
                        onclick: move |e| { e.stop_propagation(); props.on_q.call(QAction::Toggle); },
                        svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M4 6h11M4 12h7M4 18h7M17 10v8M13 14h8" } }
                    }
                }
                span {
                    class: "ldb",
                    role: "button",
                    tabindex: "0",
                    title: "Load on a deck",
                    "aria-label": "Load on a deck",
                    onclick: move |e| {
                        e.stop_propagation();
                        props.on_ld.call(());
                    },
                    svg {
                        width: "22",
                        height: "22",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M12 3v12M7 10l5 5 5-5M4 20h16" }
                    }
                }
                if props.load_open {
                    span { class: "ldm",
                        if props.decks.is_empty() {
                            span { class: "mut", style: "padding:0 8px", "NO PLAYERS" }
                        }
                        for d in props.decks.iter() {
                            {
                                let n = d.n;
                                rsx! {
                                    span {
                                        key: "{n}",
                                        class: "{d.cls}",
                                        role: "button",
                                        tabindex: "0",
                                        title: "{d.title}",
                                        onclick: move |e| {
                                            e.stop_propagation();
                                            props.on_load.call(n);
                                        },
                                        "{n}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
