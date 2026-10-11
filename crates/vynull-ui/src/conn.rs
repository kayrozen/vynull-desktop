use dioxus::prelude::*;
use vynull_core::net::{is_private_host, split_host_port};

use crate::prefs::Saved;

/// Host and port of any Vynull daemon on the network.
#[component]
pub fn ConnectScreen(error: Option<String>, on_connect: EventHandler<Saved>) -> Element {
    let mut addr = use_signal(|| "127.0.0.1:9443".to_string());
    let parsed = split_host_port(&addr());
    let public = parsed.as_ref().is_some_and(|(h, _)| !is_private_host(h));
    rsx! {
        form {
            class: "conn card",
            onsubmit: move |e| {
                e.prevent_default();
                if let Some((host, port)) = split_host_port(&addr()) {
                    on_connect.call(Saved { host, port });
                }
            },
            span { class: "kick", "CONNECT" }
            h1 { style: "margin:0;font-size:28px;font-weight:600", "Vynull daemon" }
            label {
                "HOST AND PORT"
                input {
                    r#type: "text",
                    value: "{addr}",
                    placeholder: "192.168.1.10:9443",
                    autofocus: true,
                    oninput: move |e| addr.set(e.value()),
                }
            }
            if public {
                span { class: "mono warn", "NOT A PRIVATE ADDRESS: the API has no authentication." }
            }
            if let Some(e) = error {
                span { class: "err", "{e}" }
            }
            button { class: "btn on", r#type: "submit", disabled: parsed.is_none(), "CONNECT" }
        }
    }
}
