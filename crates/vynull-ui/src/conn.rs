use dioxus::prelude::*;
use vynull_core::net::{is_private_host, split_host_port};

use crate::prefs::Saved;

/// The Vynull daemon's home, and its releases page.
pub const VYNULL_URL: &str = "https://github.com/vynulldev/vynull";
pub const VYNULL_RELEASES: &str = "https://github.com/vynulldev/vynull/releases";

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
        // This screen only shows when no daemon answered, so it is also where to learn what the
        // daemon is and where to get it. The client cannot tell "not installed" from "not running".
        section { class: "conn card inst", "aria-label": "Install the Vynull daemon",
            span { class: "kick", "NO DAEMON YET?" }
            p {
                "This app is a client. It needs the "
                b { "Vynull daemon" }
                " running on a computer on your network: a separate, free program that talks to Pioneer CDJs over Pro DJ Link and holds your library."
            }
            ul {
                li { "The daemon runs on " b { "Linux" } " (x86-64 and arm64). From Windows or macOS, connect to a Linux machine that runs it." }
                li { "Install it from its page, then start it with " code { "vynull --web" } ". It listens on port 9443." }
                li { "To reach it from another computer it must be started with " code { "--listen" } ". Read the security note in its README first: it has no authentication." }
            }
            div { class: "row",
                a { class: "btn on", href: "{VYNULL_URL}", target: "_blank", rel: "noopener noreferrer", "GET VYNULL" }
                a { class: "btn", href: "{VYNULL_RELEASES}", target: "_blank", rel: "noopener noreferrer", "RELEASES" }
            }
        }
    }
}
