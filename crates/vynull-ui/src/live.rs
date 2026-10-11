//! Live deck state from the SSE stream, plus a slow poll for the data the
//! stream does not carry (peers, link, status).

use std::time::Duration;

use dioxus::prelude::*;
use futures_util::StreamExt;
use vynull_api::{Client, NowPlaying, PeerInfo, PlayerInfo, StatusResponse};
use vynull_platform::{state_stream, time, StreamItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Connecting,
    Open,
    /// The stream dropped; a reconnect is under way.
    Lost,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Live {
    pub players: Vec<PlayerInfo>,
    pub nowplaying: NowPlaying,
    /// `time::now_ms()` when the last event arrived, for playhead interpolation.
    pub at_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Polled {
    pub peers: Vec<PeerInfo>,
    pub linked: Option<bool>,
    pub status: Option<StatusResponse>,
}

/// Everything the screens read. All fields are signals, so copies are cheap.
#[derive(Clone, Copy, PartialEq)]
pub struct Feed {
    pub live: Signal<Live>,
    pub link: Signal<Link>,
    pub polled: Signal<Polled>,
    /// Ticks about 15 times a second; read it to animate a playhead.
    pub now: Signal<f64>,
}

pub fn use_feed(client: Client) -> Feed {
    let mut live = use_signal(Live::default);
    let mut link = use_signal(|| Link::Connecting);
    let mut polled = use_signal(Polled::default);
    let mut now = use_signal(time::now_ms);

    let url = client.events_url();
    use_future(move || {
        let url = url.clone();
        async move {
            let mut stream = Box::pin(state_stream(url));
            while let Some(item) = stream.next().await {
                match item {
                    StreamItem::Open => link.set(Link::Open),
                    StreamItem::Error(_) => link.set(Link::Lost),
                    StreamItem::State(ev) => {
                        link.set(Link::Open);
                        live.set(Live {
                            players: ev.players,
                            nowplaying: ev.nowplaying,
                            at_ms: time::now_ms(),
                        });
                    }
                }
            }
        }
    });

    let c = client.clone();
    use_future(move || {
        let c = c.clone();
        async move {
            loop {
                let (peers, linked, status) = (c.peers().await, c.linked().await, c.status().await);
                let mut p = polled.write();
                if let Ok(v) = peers {
                    p.peers = v;
                }
                p.linked = linked.ok();
                if let Ok(v) = status {
                    p.status = Some(v);
                }
                drop(p);
                time::sleep(Duration::from_secs(2)).await;
            }
        }
    });

    use_future(move || async move {
        loop {
            time::sleep(Duration::from_millis(66)).await;
            now.set(time::now_ms());
        }
    });

    Feed {
        live,
        link,
        polled,
        now,
    }
}
