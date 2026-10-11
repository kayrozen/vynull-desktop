//! Diagnostics screen: runtime counters, library and analysis state, and the
//! daemon's recent log, refreshed every 2 seconds.

use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::{DiagResponse, DiagStatus};
use vynull_core::diag::{self, Entry, Source};
use vynull_platform::time;

use crate::shell::ClientCtx;

/// Log lines kept in memory.
const KEEP: usize = 1000;

#[component]
pub fn Diag(linked: Option<bool>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut stats = use_signal(|| None::<DiagResponse>);
    let mut status = use_signal(DiagStatus::default);
    let mut log = use_signal(Vec::<Entry>::new);
    let mut error = use_signal(String::new);
    let mut filter = use_signal(String::new);
    let mut errors_only = use_signal(|| false);
    let mut source = use_signal(|| Source::All);
    let mut confirm = use_signal(|| false);
    let mut note = use_signal(String::new);

    let c = client.clone();
    use_future(move || {
        let c = c.clone();
        async move {
            loop {
                match c.diag().await {
                    Ok(d) => {
                        stats.set(Some(d));
                        error.set(String::new());
                    }
                    Err(e) => error.set(e.to_string()),
                }
                if let Ok(s) = c.diag_status().await {
                    status.set(s);
                }
                let since = log.peek().last().map_or(0, |e| e.seq);
                if let Ok(lines) = c.diag_logs(since).await {
                    if !lines.is_empty() {
                        let mut l = log.write();
                        diag::append(&mut l, &lines, KEEP);
                    }
                }
                time::sleep(Duration::from_secs(2)).await;
            }
        }
    });

    // Newest at the bottom: follow the end unless the user scrolled up.
    use_effect(move || {
        let _ = log.read().len();
        document::eval("const e = document.getElementById('diag-log'); if (e && e.scrollHeight - e.scrollTop - e.clientHeight < 80) e.scrollTop = e.scrollHeight;");
    });

    let c = client.clone();
    let unlink = move |_| {
        if !confirm() {
            confirm.set(true);
            return;
        }
        confirm.set(false);
        let c = c.clone();
        spawn(async move {
            match c.unlink().await {
                Ok(()) => note
                    .set("UNLINKED: THE DAEMON CLOSED ITS SESSIONS AND DROPPED ITS PEERS.".into()),
                Err(e) => note.set(format!("UNLINK FAILED: {e}")),
            }
        });
    };

    let shown: Vec<Entry> = diag::filter(&log.read(), &filter(), errors_only(), source())
        .into_iter()
        .cloned()
        .collect();
    let d = stats();
    let link_txt = match linked {
        Some(true) => "ON",
        Some(false) => "OFF",
        None => "?",
    };

    rsx! {
        div { class: "pgd",
            main { class: "pg",
                div { class: "hd",
                    span { class: "kick", "DIAG" }
                    h1 { "Runtime, library and analysis" }
                    span { class: "mut", "Refreshes every 2 seconds" }
                    span { class: "sp" }
                    if confirm() { button { class: "btn", onclick: move |_| confirm.set(false), "CANCEL" } }
                    button { class: if confirm() { "btn dg" } else { "btn" }, title: "Close the players' sessions and drop the peers", onclick: unlink,
                        if confirm() { "CONFIRM UNLINK" } else { "UNLINK" } }
                }
                if confirm() { div { class: "mono warn", style: "font-size:13px", "Unlinking closes every player session. Players lose the library until the link is back." } }
                if !note().is_empty() { div { class: "mono", style: "font-size:13px", "{note()}" } }
                if !error().is_empty() { div { class: "errbox", "Could not read the diagnostics: {error()}" } }
                if !status().text.is_empty() {
                    div { class: "mono mut", style: "font-size:13px", "{status().kind.to_uppercase()}  {status().text}" }
                }
                section { class: "grid",
                    div { class: "card",
                        div { class: "sh2", "RUNTIME" }
                        dl { class: "dl",
                            dt { "LINK" } dd { "{link_txt}" }
                            if let Some(d) = &d {
                                dt { "UPTIME" } dd { "{diag::uptime_label(d.uptime_sec)}" }
                                dt { "GO" } dd { "{d.go_version}" }
                                dt { "GOROUTINES" } dd { "{d.goroutines}" }
                                dt { "HEAP" } dd { "{d.heap_mb} MB" }
                                dt { "SYS" } dd { "{d.sys_mb} MB" }
                                dt { "GC PAUSE" } dd { "{d.gc_pause_us} us" }
                                dt { "GC COUNT" } dd { "{d.gc_count}" }
                            }
                        }
                    }
                    div { class: "card",
                        div { class: "sh2", "LIBRARY" }
                        dl { class: "dl",
                            if let Some(d) = &d {
                                dt { "TRACKS" } dd { "{diag::thousands(d.library_tracks)}" }
                                dt { "CACHE ON DISK" } dd { "{diag::bytes_label(d.cache_dir_bytes)}" }
                            }
                        }
                    }
                    div { class: "card",
                        div { class: "sh2", "ANALYSIS" }
                        dl { class: "dl",
                            if let Some(d) = &d {
                                dt { "PENDING" } dd { "{diag::thousands(d.analysis_pending)}" }
                                dt { "ANALYZED" } dd { "{diag::thousands(d.analysis_analyzed)}" }
                                dt { "CACHED" } dd { "{diag::thousands(d.analysis_cached)}" }
                            }
                        }
                        if let Some(d) = &d {
                            div { class: "sh2", style: "margin-top:8px", "DONE" }
                            div { class: "pr", i { style: "width:{diag::analysis_percent(d.analysis_pending, d.analysis_analyzed):.1}%" } }
                        }
                    }
                }
                section { class: "card",
                    div { class: "flt",
                        span { class: "sh2", "RECENT LOG" }
                        span { class: "mut", style: "font-size:13px", "Newest at the bottom" }
                        span { class: "sp" }
                        span { class: "mono mut", style: "font-size:12px", "{shown.len()} OF {log.read().len()} LINES" }
                    }
                    div { class: "flt",
                        input { class: "in", r#type: "text", placeholder: "Filter, not case sensitive", "aria-label": "Filter log",
                            value: "{filter()}", oninput: move |e| filter.set(e.value()) }
                        label { class: "chk", input { r#type: "checkbox", checked: errors_only(), onchange: move |e| errors_only.set(e.checked()) } "Errors only" }
                        for s in Source::ALL {
                            button { key: "{s.label()}", class: if s == source() { "pill on" } else { "pill" }, onclick: move |_| source.set(s), "{s.label()}" }
                        }
                    }
                    pre { class: "log", id: "diag-log",
                        if shown.is_empty() { span { class: "ln", "Nothing matches." } }
                        for e in shown.iter() {
                            span { key: "{e.seq}", class: "{e.level.class()}", b { "{time::clock_label(e.time)}" } "  {e.text}" }
                        }
                    }
                }
            }
        }
    }
}
