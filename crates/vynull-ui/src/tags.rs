//! Tags screen: categories with their tags, an editor for the selected tag,
//! and the track colors with how many tracks use each.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use vynull_api::{TagCategoryInfo, TagInfo, TagPatch};
use vynull_core::palette;

use crate::library_data::LibState;
use crate::shell::ClientCtx;

#[component]
pub fn Tags(lib: Signal<LibState>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut refresh = use_signal(|| 0u32);
    let mut sel = use_signal(|| None::<i64>);
    let mut msg = use_signal(String::new);
    let mut new_tag = use_signal(|| None::<String>);
    let mut new_cat = use_signal(|| None::<String>);
    let mut rename = use_signal(|| None::<(i64, String)>);
    let mut confirm = use_signal(|| None::<String>);

    let c = client.clone();
    let data = use_resource(move || {
        let _ = refresh();
        let c = c.clone();
        async move {
            let (t, k) = (c.tags().await, c.tag_categories().await);
            match (t, k) {
                (Ok(t), Ok(k)) => Ok((t, k)),
                (Err(e), _) | (_, Err(e)) => Err(e.to_string()),
            }
        }
    });

    // The library list and the drawer show tags from `LibState`: keep it current.
    let sync = use_callback(move |tags: Vec<TagInfo>| {
        lib.write().tags = Rc::new(tags);
    });
    let c = client.clone();
    let reload = use_callback(move |_: ()| {
        let c = c.clone();
        spawn(async move {
            if let Ok(t) = c.tags().await {
                sync.call(t);
            }
            // Track records carry tag names: a rename or delete changes them.
            if let Ok(tr) = c.tracks().await {
                let cur = lib.peek().clone();
                let index = Rc::new(vynull_core::search::SearchIndex::new(&tr));
                let by_id = tr.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
                lib.set(LibState {
                    tracks: Rc::new(tr),
                    index,
                    by_id: Rc::new(by_id),
                    ..cur
                });
            }
            refresh.set(refresh() + 1);
        });
    });

    let counts: HashMap<String, usize> = {
        let st = lib.read();
        let mut m = HashMap::new();
        for t in st.tracks.iter() {
            for n in &t.tags {
                *m.entry(n.clone()).or_insert(0) += 1;
            }
        }
        m
    };
    let color_counts: HashMap<i64, usize> = {
        let st = lib.read();
        let mut m = HashMap::new();
        for t in st.tracks.iter() {
            *m.entry(t.color_id).or_insert(0) += 1;
        }
        m
    };

    // Keep the last good lists while a reload is under way: showing "no categories" for
    // a moment would also send a tag created at that instant out without a category.
    let mut last = use_signal(|| None::<(Vec<TagInfo>, Vec<TagCategoryInfo>)>);
    if let Some(Ok(v)) = &*data.read() {
        if last.peek().as_ref() != Some(v) {
            last.set(Some(v.clone()));
        }
    }
    let (tags, cats) = match (last(), &*data.read()) {
        (Some(v), _) => v,
        (None, Some(Err(e))) => {
            return rsx! { div { class: "pgt", main { class: "pg", div { class: "errbox", "Could not load tags: {e}" } } } };
        }
        (None, _) => (Vec::new(), Vec::new()),
    };
    let selected: Option<TagInfo> = sel().and_then(|id| tags.iter().find(|t| t.id == id).cloned());

    let c = client.clone();
    let create_cat: Rc<dyn Fn()> = {
        let c = c.clone();
        Rc::new(move || {
            let Some(name) = new_cat()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
            else {
                return;
            };
            let c = c.clone();
            spawn(async move {
                match c.create_tag_category(&name).await {
                    Ok(_) => {
                        new_cat.set(None);
                        reload.call(());
                    }
                    Err(e) => msg.set(format!("COULD NOT CREATE THE CATEGORY: {e}")),
                }
            });
        })
    };
    let create_tag: Rc<dyn Fn()> = {
        let c = c.clone();
        Rc::new(move || {
            let Some(name) = new_tag()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
            else {
                return;
            };
            let c = c.clone();
            // A new tag starts in the selected tag's category, or the first one. Read when the
            // tag is created, from the signals: a value taken at an earlier render can be stale.
            let lists = last();
            let cat = lists.as_ref().and_then(|(t, k)| {
                sel()
                    .and_then(|id| t.iter().find(|x| x.id == id).map(|x| x.category_id))
                    .or_else(|| k.first().map(|c| c.id))
            });
            // The Enter key reaches here even where the button is disabled: a tag always
            // belongs to a category, so wait until there is one to put it in.
            let Some(cat) = cat else {
                let mut msg = msg;
                msg.set("ADD A CATEGORY FIRST, THEN CREATE THE TAG.".into());
                return;
            };
            spawn(async move {
                match c.create_tag(&name, Some(cat)).await {
                    Ok(t) => {
                        new_tag.set(None);
                        sel.set(Some(t.id));
                        reload.call(());
                    }
                    Err(e) => msg.set(format!("COULD NOT CREATE THE TAG: {e}")),
                }
            });
        })
    };

    rsx! {
        div { class: "pgt",
            main { class: "pg",
                div { class: "hd",
                    span { class: "kick", "TAGS" }
                    h1 { "Tags and track colors" }
                    span { class: "mut", "{tags.len()} tags in {cats.len()} categories" }
                    span { class: "sp" }
                    button { class: "btn", onclick: move |_| new_cat.set(Some(String::new())), "+ CATEGORY" }
                    button { class: "btn on", onclick: move |_| new_tag.set(Some(String::new())), "+ TAG" }
                }
                if !msg().is_empty() {
                    div { class: "mono warn", style: "font-size:13px", "{msg()}" }
                }
                if let Some(v) = new_cat() {
                    div { class: "card",
                        span { class: "sh2", "NEW CATEGORY" }
                        div { style: "display:flex;gap:10px;flex-wrap:wrap",
                            input { class: "in", style: "flex:1 1 240px", r#type: "text", placeholder: "Name", autofocus: true, value: "{v}",
                                oninput: move |e| new_cat.set(Some(e.value())),
                                onkeydown: {
                                    let go = create_cat.clone();
                                    move |e| if e.key() == Key::Enter { go() }
                                } }
                            button { class: "btn on", disabled: v.trim().is_empty(), onclick: { let go = create_cat.clone(); move |_| go() }, "CREATE" }
                            button { class: "btn", onclick: move |_| new_cat.set(None), "CANCEL" }
                        }
                    }
                }
                if let Some(v) = new_tag() {
                    div { class: "card",
                        span { class: "sh2", "NEW TAG" }
                        div { style: "display:flex;gap:10px;flex-wrap:wrap",
                            input { class: "in", style: "flex:1 1 240px", r#type: "text", placeholder: "Name", autofocus: true, value: "{v}",
                                oninput: move |e| new_tag.set(Some(e.value())),
                                onkeydown: {
                                    let go = create_tag.clone();
                                    move |e| if e.key() == Key::Enter { go() }
                                } }
                            button { class: "btn on", disabled: v.trim().is_empty() || cats.is_empty(), onclick: { let go = create_tag.clone(); move |_| go() }, "CREATE" }
                            button { class: "btn", onclick: move |_| new_tag.set(None), "CANCEL" }
                        }
                        span { class: "mut", style: "font-size:13px", "It starts in the category of the selected tag, or the first one. Move it in the editor." }
                    }
                }
                div { class: "lay",
                    section { class: "cats",
                        if cats.is_empty() && last().is_some() {
                            div { class: "card", span { class: "mut", "No tag categories yet. Add one first." } }
                        }
                        for cat in cats.iter() {
                            CategoryCard {
                                key: "{cat.id}",
                                cat: cat.clone(),
                                tags: tags.iter().filter(|t| t.category_id == cat.id).cloned().collect::<Vec<_>>(),
                                counts: counts.clone(),
                                selected: sel(),
                                renaming: rename().filter(|(id, _)| *id == cat.id).map(|(_, v)| v),
                                confirm: confirm().as_deref() == Some(&format!("cat{}", cat.id)),
                                on_pick: move |id| sel.set(Some(id)),
                                on_rename_start: move |(id, name)| rename.set(Some((id, name))),
                                on_rename_edit: move |v: String| { let id = rename().map(|r| r.0).unwrap_or(0); rename.set(Some((id, v))) },
                                on_rename_cancel: move |_| rename.set(None),
                                on_rename_save: {
                                    let c = client.clone();
                                    move |(id, name): (i64, String)| {
                                        let c = c.clone();
                                        spawn(async move {
                                            match c.rename_tag_category(id, name.trim()).await {
                                                Ok(()) => { rename.set(None); reload.call(()); }
                                                Err(e) => msg.set(format!("COULD NOT RENAME: {e}")),
                                            }
                                        });
                                    }
                                },
                                on_delete: {
                                    let c = client.clone();
                                    move |id: i64| {
                                        let key = format!("cat{id}");
                                        if confirm().as_deref() != Some(&key) {
                                            confirm.set(Some(key));
                                            return;
                                        }
                                        confirm.set(None);
                                        let c = c.clone();
                                        spawn(async move {
                                            match c.delete_tag_category(id).await {
                                                Ok(()) => reload.call(()),
                                                Err(e) => msg.set(format!("COULD NOT DELETE THE CATEGORY: {e}")),
                                            }
                                        });
                                    }
                                },
                            }
                        }
                    }
                    aside { class: "ed",
                        if let Some(t) = selected {
                            TagEditor {
                                key: "{t.id}",
                                tag: t.clone(),
                                cats: cats.clone(),
                                uses: counts.get(&t.name).copied().unwrap_or(0),
                                confirm: confirm().as_deref() == Some(&format!("tag{}", t.id)),
                                on_save: {
                                    let c = client.clone();
                                    move |(id, patch): (i64, TagPatch)| {
                                        let c = c.clone();
                                        spawn(async move {
                                            match c.update_tag(id, &patch).await {
                                                Ok(()) => { msg.set(String::new()); reload.call(()); }
                                                Err(e) => msg.set(format!("COULD NOT SAVE THE TAG: {e}")),
                                            }
                                        });
                                    }
                                },
                                on_delete: {
                                    let c = client.clone();
                                    move |id: i64| {
                                        let key = format!("tag{id}");
                                        if confirm().as_deref() != Some(&key) {
                                            confirm.set(Some(key));
                                            return;
                                        }
                                        confirm.set(None);
                                        let c = c.clone();
                                        spawn(async move {
                                            match c.delete_tag(id).await {
                                                Ok(()) => { sel.set(None); reload.call(()); }
                                                Err(e) => msg.set(format!("COULD NOT DELETE THE TAG: {e}")),
                                            }
                                        });
                                    }
                                },
                                on_cancel_confirm: move |_| confirm.set(None),
                            }
                        } else {
                            div { class: "card",
                                span { class: "sh2", "EDIT TAG" }
                                span { class: "mut", "Pick a tag to rename it, move it to another category, or change its color." }
                            }
                        }
                        div { class: "card",
                            div { class: "sh2", "TRACK COLORS" }
                            div { class: "tc",
                                for (cid , name , hex) in palette::PALETTE.iter().filter(|p| p.0 != 0) {
                                    div { key: "{cid}", class: "tcc",
                                        i { style: "background:{hex}" }
                                        "{name}"
                                        b { "{color_counts.get(cid).copied().unwrap_or(0)}" }
                                    }
                                }
                            }
                        }
                        div { class: "card",
                            span { class: "fl", style: "align-self:flex-start", "REKORDBOX MYTAGS" }
                            span { class: "mut", "A rekordbox import brings the MyTags with their categories and the track colors." }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CategoryCard(
    cat: TagCategoryInfo,
    tags: Vec<TagInfo>,
    counts: HashMap<String, usize>,
    selected: Option<i64>,
    renaming: Option<String>,
    confirm: bool,
    on_pick: EventHandler<i64>,
    on_rename_start: EventHandler<(i64, String)>,
    on_rename_edit: EventHandler<String>,
    on_rename_cancel: EventHandler<()>,
    on_rename_save: EventHandler<(i64, String)>,
    on_delete: EventHandler<i64>,
) -> Element {
    let id = cat.id;
    let n = tags.len();
    rsx! {
        div { class: "card",
            div { class: "ch",
                if let Some(v) = renaming.clone() {
                    input { class: "in", style: "flex:1", r#type: "text", value: "{v}", autofocus: true,
                        oninput: move |e| on_rename_edit.call(e.value()),
                        onkeydown: { let v = v.clone(); move |e| match e.key() {
                            Key::Enter if !v.trim().is_empty() => on_rename_save.call((id, v.clone())),
                            Key::Escape => on_rename_cancel.call(()),
                            _ => {}
                        } } }
                    button { class: "btn on", style: "height:36px", disabled: v.trim().is_empty(), onclick: { let v = v.clone(); move |_| on_rename_save.call((id, v.clone())) }, "SAVE" }
                    button { class: "btn", style: "height:36px", onclick: move |_| on_rename_cancel.call(()), "CANCEL" }
                } else {
                    b { "{cat.name}" }
                    span { class: "mono mut", "{n} TAGS" }
                    button { class: "btn", style: "height:36px", "aria-label": "Edit category", onclick: { let name = cat.name.clone(); move |_| on_rename_start.call((id, name.clone())) }, "RENAME" }
                    button { class: if confirm { "btn dg" } else { "btn" }, style: "height:36px", title: "Delete this category",
                        onclick: move |_| on_delete.call(id),
                        if confirm { "CONFIRM DELETE" } else { "DELETE" } }
                }
            }
            if tags.is_empty() { span { class: "mut", "No tags in this category." } }
            for t in tags.iter() {
                {
                    let tid = t.id;
                    let uses = counts.get(&t.name).copied().unwrap_or(0);
                    rsx! {
                        button { key: "{tid}", class: if selected == Some(tid) { "tr on" } else { "tr" }, onclick: move |_| on_pick.call(tid),
                            i { style: "background:{palette::tag_color(t.color, t.id)}" }
                            span { class: "n", "{t.name}" }
                            span { class: "c", "{uses}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TagEditor(
    tag: TagInfo,
    cats: Vec<TagCategoryInfo>,
    uses: usize,
    confirm: bool,
    on_save: EventHandler<(i64, TagPatch)>,
    on_delete: EventHandler<i64>,
    on_cancel_confirm: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| tag.name.clone());
    let mut cat = use_signal(|| tag.category_id);
    let mut color = use_signal(|| tag.color);
    let id = tag.id;
    let changed = name().trim() != tag.name || cat() != tag.category_id || color() != tag.color;
    let ok = !name().trim().is_empty() && changed;
    let save = {
        let t = tag.clone();
        move |_| {
            let patch = TagPatch {
                name: (name().trim() != t.name).then(|| name().trim().to_string()),
                category_id: (cat() != t.category_id).then_some(cat()),
                color: (color() != t.color).then_some(color()),
            };
            on_save.call((id, patch));
        }
    };
    rsx! {
        div { class: "card",
            div { class: "sh2", "EDIT TAG" }
            label { class: "fld", "NAME"
                input { class: "in", r#type: "text", value: "{name()}", oninput: move |e| name.set(e.value()) } }
            label { class: "fld", "CATEGORY"
                select { class: "in", value: "{cat()}", onchange: move |e| cat.set(e.value().parse().unwrap_or(0)),
                    for c in cats.iter() { option { key: "{c.id}", value: "{c.id}", selected: c.id == cat(), "{c.name}" } }
                } }
            div { class: "fld", "COLOR"
                div { class: "sw",
                    for (cid , cname , hex) in palette::PALETTE {
                        button {
                            key: "{cid}",
                            class: if color() == cid { "on" } else { "" },
                            style: if cid == 0 { "background:var(--surface2);border:1px dashed var(--faint)".to_string() } else { format!("background:{hex}") },
                            "aria-label": "{cname}",
                            title: "{cname}",
                            onclick: move |_| color.set(cid),
                        }
                    }
                }
            }
            div { class: "mono mut", style: "font-size:13px", "{uses} TRACKS USE THIS TAG" }
            div { style: "display:flex;gap:10px;flex-wrap:wrap",
                button { class: "btn on", disabled: !ok, onclick: save, "SAVE" }
                button { class: if confirm { "btn dg" } else { "btn" }, onclick: move |_| on_delete.call(id), if confirm { "CONFIRM DELETE" } else { "DELETE TAG" } }
                if confirm { button { class: "btn", onclick: move |_| on_cancel_confirm.call(()), "KEEP IT" } }
            }
            if confirm && uses > 0 { span { class: "mono warn", style: "font-size:12px", "{uses} tracks will lose this tag." } }
        }
    }
}
