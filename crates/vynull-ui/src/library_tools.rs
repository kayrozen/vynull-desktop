//! The library's tool panels: import, export to USB, and creating playlists,
//! folders and smart playlists. They change data on the daemon, so every
//! action says what happened (or why it failed) through `on_flash`.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::{
    ExportRequest, FsListing, ImportInclude, ImportRequest, NewPlaylist, PlaylistInfo, TagInfo,
};
use vynull_core::smart::{self, Kind, Row};
use vynull_platform::time;

use crate::shell::ClientCtx;

/// Browse the daemon's filesystem (`GET /api/fs/list`) and pick a file or a folder.
#[component]
pub fn FilePicker(
    /// Extensions that can be picked, lowercase, no dot. Empty means a folder is wanted.
    extensions: Vec<String>,
    on_pick: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut path = use_signal(|| None::<String>);
    let c = client.clone();
    let listing = use_resource(move || {
        let p = path();
        let c = c.clone();
        async move { c.fs_list(p.as_deref()).await.map_err(|e| e.to_string()) }
    });
    let want_folder = extensions.is_empty();
    rsx! {
        div { class: "fp",
            div { class: "wt",
                span { class: "sh2", if want_folder { "CHOOSE A FOLDER" } else { "CHOOSE A FILE" } }
                span { class: "sp" }
                button { class: "btn sm", onclick: move |_| on_cancel.call(()), "CANCEL" }
            }
            match &*listing.read() {
                None => rsx! { span { class: "mut", "Loading…" } },
                Some(Err(e)) => rsx! { span { class: "mut", "{e}" } },
                Some(Ok(l)) => {
                    let l: FsListing = l.clone();
                    rsx! {
                        div { class: "mono mut", style: "font-size:12px;word-break:break-all", if l.roots { "ALLOWED FOLDERS" } else { "{l.path}" } }
                        div { class: "fpl",
                            if !l.roots {
                                button {
                                    class: "fpe",
                                    onclick: {
                                        let parent = l.parent.clone();
                                        move |_| path.set(if parent.is_empty() { None } else { Some(parent.clone()) })
                                    },
                                    "↑ UP"
                                }
                            }
                            for e in l.entries.iter() {
                                {
                                    let entry_path = e.path.clone();
                                    let is_dir = e.is_dir;
                                    let ext = e.name.rsplit('.').next().unwrap_or("").to_lowercase();
                                    let pickable = !is_dir && extensions.contains(&ext);
                                    let dim = !is_dir && !pickable;
                                    rsx! {
                                        button {
                                            key: "{e.path}",
                                            class: if dim { "fpe dim" } else { "fpe" },
                                            disabled: dim,
                                            onclick: move |_| {
                                                if is_dir {
                                                    path.set(Some(entry_path.clone()));
                                                } else {
                                                    on_pick.call(entry_path.clone());
                                                }
                                            },
                                            span { if is_dir { "📁 " } else { "" } "{e.name}" }
                                        }
                                    }
                                }
                            }
                        }
                        if want_folder && !l.roots {
                            button { class: "btn on sm", onclick: { let p = l.path.clone(); move |_| on_pick.call(p.clone()) }, "USE THIS FOLDER" }
                        }
                    }
                }
            }
        }
    }
}

/// Import a rekordbox library or a Traktor collection.
#[component]
fn ImportCard(on_flash: EventHandler<String>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut traktor = use_signal(|| false);
    let mut file = use_signal(String::new);
    let mut key = use_signal(String::new);
    let mut picking = use_signal(|| false);
    let mut running = use_signal(|| false);
    let mut status = use_signal(String::new);
    let mut progress = use_signal(|| 0.0_f64);

    let exts: Vec<String> = if traktor() {
        vec!["nml".into()]
    } else {
        vec!["xml".into(), "db".into(), "zip".into()]
    };
    let needs_key = {
        let f = file().to_lowercase();
        f.ends_with(".db") || f.ends_with(".zip")
    };
    let c = client.clone();
    let start = move |_| {
        if running() || file().is_empty() {
            return;
        }
        running.set(true);
        progress.set(0.0);
        status.set("STARTING…".into());
        let c = c.clone();
        let req = ImportRequest {
            path: file(),
            key: if needs_key && !key().trim().is_empty() {
                Some(key().trim().to_string())
            } else {
                None
            },
            include: Some(ImportInclude::default()),
            ..Default::default()
        };
        // Poll the status while the import request is open.
        let poll_c = c.clone();
        let poller = spawn(async move {
            loop {
                if let Ok(s) = poll_c.import_status().await {
                    if s.active {
                        status.set(format!(
                            "{}  {} OF {}",
                            s.phase.to_uppercase(),
                            s.done,
                            s.total
                        ));
                        if s.total > 0 {
                            progress.set(s.done as f64 / s.total as f64 * 100.0);
                        }
                    }
                }
                time::sleep(Duration::from_millis(500)).await;
            }
        });
        spawn(async move {
            let res = c.import(&req).await;
            poller.cancel();
            running.set(false);
            match res {
                Ok(r) => {
                    progress.set(100.0);
                    status.set(format!(
                        "DONE: {} TRACKS ADDED, {} UPDATED, {} SKIPPED{}",
                        r.tracks_added,
                        r.tracks_updated,
                        r.tracks_skipped,
                        if r.errors.is_empty() {
                            String::new()
                        } else {
                            format!(", {} ERRORS", r.errors.len())
                        }
                    ));
                    on_flash.call("IMPORT FINISHED".into());
                }
                Err(e) => {
                    status.set(format!("FAILED: {e}"));
                    on_flash.call("IMPORT FAILED".into());
                }
            }
        });
    };

    rsx! {
        div { class: "tc",
            span { class: "sh2", "IMPORT" }
            div { class: "opt",
                button { class: if traktor() { "btn sm" } else { "btn sm on" }, onclick: move |_| { traktor.set(false); file.set(String::new()); }, "REKORDBOX" }
                button { class: if traktor() { "btn sm on" } else { "btn sm" }, onclick: move |_| { traktor.set(true); file.set(String::new()); }, "TRAKTOR" }
            }
            span { class: "mut", if traktor() { "Choose a Traktor collection.nml." } else { "Choose a rekordbox XML, a master.db or a backup zip." } }
            if !file().is_empty() { div { class: "pth", "{file()}" } }
            if needs_key {
                div { class: "fld", style: "grid-template-columns:1fr",
                    label { class: "w", "KEY, MASTER.DB AND BACKUP ZIP ONLY"
                        input { class: "in mono", r#type: "text", placeholder: "64 hex digits", value: "{key()}", oninput: move |e| key.set(e.value()) }
                    }
                }
            }
            if picking() {
                FilePicker { extensions: exts, on_pick: move |p| { file.set(p); picking.set(false); }, on_cancel: move |_| picking.set(false) }
            } else {
                div { class: "opt",
                    button { class: "btn", disabled: running(), onclick: move |_| picking.set(true), "CHOOSE FILE" }
                    button { class: "btn on", disabled: running() || file().is_empty(), onclick: start, if running() { "IMPORTING…" } else { "IMPORT" } }
                }
            }
            if !status().is_empty() {
                div { class: "pr", i { style: "width:{progress()}%" } }
                div { class: "mono mut", style: "font-size:12px", "{status()}" }
            }
        }
    }
}

/// Write a rekordbox-compatible USB.
#[component]
fn ExportCard(playlists: Vec<PlaylistInfo>, on_flash: EventHandler<String>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut dest = use_signal(String::new);
    let mut source = use_signal(|| "all".to_string());
    let mut copy = use_signal(|| true);
    let mut merge = use_signal(|| false);
    let mut picking = use_signal(|| false);
    let mut busy = use_signal(|| false);

    let c = client.clone();
    let preview = use_resource(move || {
        let s = source();
        let c = c.clone();
        async move { c.export_preview(&s).await.map(|p| p.total).ok() }
    });
    let c = client.clone();
    let write = move |_| {
        if busy() || dest().trim().is_empty() {
            return;
        }
        busy.set(true);
        let c = c.clone();
        let req = ExportRequest {
            destination: dest().trim().to_string(),
            source: Some(source()),
            copy_files: Some(copy()),
            merge: Some(merge()),
        };
        spawn(async move {
            match c.export(&req).await {
                Ok(r) => on_flash.call(format!(
                    "WROTE {} TRACKS TO {}",
                    r.track_count, r.destination
                )),
                Err(e) => on_flash.call(format!("EXPORT FAILED: {e}")),
            }
            busy.set(false);
        });
    };
    let choices: Vec<(String, String)> = playlists
        .iter()
        .filter(|p| !p.is_folder)
        .map(|p| {
            (
                format!("{}:{}", if p.is_smart { "smart" } else { "playlist" }, p.id),
                format!(
                    "{}: {}",
                    if p.is_smart { "Smart" } else { "Playlist" },
                    p.name
                ),
            )
        })
        .collect();
    rsx! {
        div { class: "tc",
            span { class: "sh2", "EXPORT TO USB" }
            div { class: "fld", style: "grid-template-columns:1fr",
                label { class: "w", "DESTINATION"
                    div { class: "opt",
                        input { class: "in mono", style: "flex:1;min-width:0", r#type: "text", placeholder: "/media/usb", value: "{dest()}", oninput: move |e| dest.set(e.value()) }
                        button { class: "btn sm", onclick: move |_| picking.set(true), "BROWSE" }
                    }
                }
                label { class: "w", "SOURCE"
                    select { class: "in", value: "{source()}", onchange: move |e| source.set(e.value()),
                        option { value: "all", "All tracks" }
                        for (v , l) in choices.iter() { option { key: "{v}", value: "{v}", "{l}" } }
                    }
                }
            }
            if picking() {
                FilePicker { extensions: Vec::new(), on_pick: move |p| { dest.set(p); picking.set(false); }, on_cancel: move |_| picking.set(false) }
            }
            div { class: "opt",
                label { class: "chk", input { r#type: "checkbox", checked: copy(), onchange: move |e| copy.set(e.checked()) } "Copy files" }
                label { class: "chk", input { r#type: "checkbox", checked: merge(), onchange: move |e| merge.set(e.checked()) } "Merge with the USB" }
            }
            div { class: "mono mut", style: "font-size:12px",
                match &*preview.read() {
                    Some(Some(n)) => format!("{n} TRACKS WILL EXPORT"),
                    Some(None) => "COULD NOT COUNT THE TRACKS".to_string(),
                    None => "COUNTING…".to_string(),
                }
            }
            button { class: "btn on", disabled: busy() || dest().trim().is_empty(), onclick: write, if busy() { "WRITING…" } else { "WRITE USB" } }
        }
    }
}

#[component]
pub fn ToolsPanel(playlists: Vec<PlaylistInfo>, on_flash: EventHandler<String>) -> Element {
    rsx! {
        div { class: "tool",
            ImportCard { on_flash }
            ExportCard { playlists, on_flash }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Playlist,
    Folder,
    Smart,
}

/// Name a new playlist, folder or smart playlist (with its rules).
#[component]
pub fn NewPlaylistPanel(
    kind: NewKind,
    tags: Vec<TagInfo>,
    on_done: EventHandler<PlaylistInfo>,
    on_cancel: EventHandler<()>,
    on_flash: EventHandler<String>,
) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut name = use_signal(String::new);
    let mut all = use_signal(|| true);
    let mut rows = use_signal(|| vec![Row::default()]);
    let busy = use_signal(|| false);

    let title = match kind {
        NewKind::Playlist => "NEW PLAYLIST",
        NewKind::Folder => "NEW FOLDER",
        NewKind::Smart => "NEW SMART PLAYLIST",
    };
    let rules = if kind == NewKind::Smart {
        smart::build_rules(all(), &rows.read())
    } else {
        None
    };
    let rules_ok = rules.is_some();
    let can_create = !name().trim().is_empty() && (kind != NewKind::Smart || rules_ok) && !busy();
    // Shared by the Enter key and the button; both need the values of this render.
    let c = client.clone();
    let create: Rc<dyn Fn()> = Rc::new(move || {
        if !can_create {
            return;
        }
        let mut busy = busy;
        busy.set(true);
        let c = c.clone();
        let req = NewPlaylist {
            name: name().trim().to_string(),
            parent_id: None,
            is_folder: (kind == NewKind::Folder).then_some(true),
            is_smart: (kind == NewKind::Smart).then_some(true),
            rules: rules.clone(),
        };
        spawn(async move {
            match c.create_playlist(&req).await {
                Ok(p) => on_done.call(p),
                Err(e) => on_flash.call(format!("CREATE FAILED: {e}")),
            }
            busy.set(false);
        });
    });
    let on_enter = create.clone();
    rsx! {
        div { class: "tool",
            div { class: "tc", style: "grid-column:1/-1",
                div { class: "wt",
                    span { class: "sh2", "{title}" }
                    span { class: "sp" }
                    button { class: "btn sm", onclick: move |_| on_cancel.call(()), "CANCEL" }
                }
                div { class: "fld", style: "grid-template-columns:1fr",
                    label { class: "w", "NAME"
                        input { class: "in", r#type: "text", autofocus: true, value: "{name()}", oninput: move |e| name.set(e.value()),
                            onkeydown: move |e| { e.stop_propagation(); if e.key() == Key::Enter { on_enter() } } }
                    }
                }
                if kind == NewKind::Smart {
                    div { class: "opt",
                        span { class: "sh2", style: "align-self:center", "MATCH" }
                        div { class: "seg",
                            button { class: if all() { "on" } else { "" }, onclick: move |_| all.set(true), "ALL" }
                            button { class: if all() { "" } else { "on" }, onclick: move |_| all.set(false), "ANY" }
                        }
                        span { class: "mut", "of these rules" }
                    }
                    for (i , r) in rows.read().iter().cloned().enumerate() {
                        RuleRow { key: "{i}", row: r, tags: tags.clone(),
                            on_change: move |nr| rows.write()[i] = nr,
                            on_remove: move |_| { if rows.read().len() > 1 { rows.write().remove(i); } } }
                    }
                    div { class: "opt",
                        button { class: "btn sm", onclick: move |_| rows.write().push(Row::default()), "+ RULE" }
                        if !rules_ok { span { class: "mut", "Complete at least one rule." } }
                    }
                }
                button { class: "btn on", disabled: !can_create, onclick: move |_| create(), "CREATE" }
            }
        }
    }
}

#[component]
fn RuleRow(
    row: Row,
    tags: Vec<TagInfo>,
    on_change: EventHandler<Row>,
    on_remove: EventHandler<()>,
) -> Element {
    let kind = smart::kind_of(&row.field);
    let ops = smart::operators(kind);
    let r0 = row.clone();
    rsx! {
        div { class: "opt", style: "align-items:center",
            select { class: "in", style: "width:auto", value: "{row.field}",
                onchange: { let r = row.clone(); move |e| { let mut n = r.clone(); n.field = e.value(); n.a.clear(); n.b.clear(); n.fix_operator(); on_change.call(n); } },
                for (f , l , _) in smart::FIELDS.iter() { option { key: "{f}", value: "{f}", "{l}" } }
            }
            select { class: "in", style: "width:auto", value: "{row.op}",
                onchange: { let r = row.clone(); move |e| { let mut n = r.clone(); n.op = e.value(); on_change.call(n); } },
                for (o , l) in ops.iter() { option { key: "{o}", value: "{o}", "{l}" } }
            }
            if smart::takes_value(&row.op) {
                if kind == Kind::Tag {
                    select { class: "in", style: "width:auto", value: "{row.a}",
                        onchange: { let r = row.clone(); move |e| { let mut n = r.clone(); n.a = e.value(); on_change.call(n); } },
                        option { value: "", "Choose a tag" }
                        for t in tags.iter() { option { key: "{t.id}", value: "{t.name}", "{t.name}" } }
                    }
                } else {
                    input { class: "in", style: "width:140px", r#type: if kind == Kind::Number { "number" } else { "text" }, value: "{row.a}",
                        oninput: { let r = row.clone(); move |e| { let mut n = r.clone(); n.a = e.value(); on_change.call(n); } },
                        onkeydown: move |e| e.stop_propagation() }
                    if kind == Kind::Number && row.op == "between" {
                        span { class: "mut", "AND" }
                        input { class: "in", style: "width:140px", r#type: "number", value: "{row.b}",
                            oninput: { let r = r0.clone(); move |e| { let mut n = r.clone(); n.b = e.value(); on_change.call(n); } },
                            onkeydown: move |e| e.stop_propagation() }
                    }
                }
            }
            button { class: "btn ib", style: "width:36px;height:36px", title: "Remove this rule", onclick: move |_| on_remove.call(()), "✕" }
        }
    }
}
