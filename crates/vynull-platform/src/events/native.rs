use std::pin::Pin;
use std::time::Duration;

use eventsource_stream::Eventsource;
use futures_util::stream::{self, Stream, StreamExt};

use super::StreamItem;

type Inner = Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>;

const MIN_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(5);
/// The daemon sends state about four times a second, so this long without a single byte
/// means the link is dead even though the socket never closed (sleep, a Wi-Fi switch).
const IDLE: Duration = Duration::from_secs(10);
/// Longest wait for a connection to a host that does not answer.
const CONNECT: Duration = Duration::from_secs(8);

struct State {
    url: String,
    http: reqwest::Client,
    /// `Some` while connected: yields the `data` of each `state` event.
    inner: Option<Inner>,
    backoff: Duration,
    /// Wait before the next connect attempt.
    wait: bool,
    idle: Duration,
}

/// Stream of live state from `url` (for example `Client::events_url()`).
/// It never ends on its own: drop it to disconnect.
pub fn state_stream(url: String) -> impl Stream<Item = StreamItem> + Send {
    state_stream_with(url, IDLE, CONNECT)
}

/// [`state_stream`] with its two timeouts given: how long the link may stay silent before it is
/// dropped and reopened, and how long to wait for a connection.
pub fn state_stream_with(
    url: String,
    idle: Duration,
    connect_timeout: Duration,
) -> impl Stream<Item = StreamItem> + Send {
    let st = State {
        url,
        http: reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .build()
            .unwrap_or_default(),
        inner: None,
        backoff: MIN_BACKOFF,
        wait: false,
        idle,
    };
    stream::unfold(st, |mut st| async move {
        if let Some(inner) = st.inner.as_mut() {
            let item = match tokio::time::timeout(st.idle, inner.next()).await {
                Ok(Some(Ok(data))) => parse(&data),
                Ok(Some(Err(e))) => drop_link(&mut st, e),
                Ok(None) => drop_link(&mut st, "stream closed".into()),
                Err(_) => drop_link(&mut st, "no data from the daemon, reconnecting".into()),
            };
            return Some((item, st));
        }
        if st.wait {
            tokio::time::sleep(st.backoff).await;
            st.backoff = (st.backoff * 2).min(MAX_BACKOFF);
        }
        let item = match connect(&st.http, &st.url).await {
            Ok(inner) => {
                st.inner = Some(inner);
                st.backoff = MIN_BACKOFF;
                st.wait = false;
                StreamItem::Open
            }
            Err(e) => {
                st.wait = true;
                StreamItem::Error(e)
            }
        };
        Some((item, st))
    })
}

fn drop_link(st: &mut State, why: String) -> StreamItem {
    st.inner = None;
    st.wait = true;
    StreamItem::Error(why)
}

fn parse(data: &str) -> StreamItem {
    match serde_json::from_str(data) {
        Ok(ev) => StreamItem::State(ev),
        Err(e) => StreamItem::Error(format!("bad state event: {e}")),
    }
}

async fn connect(http: &reqwest::Client, url: &str) -> Result<Inner, String> {
    let resp = http
        .get(url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("events: HTTP {}", resp.status()));
    }
    let events = resp
        .bytes_stream()
        .eventsource()
        .filter_map(|ev| async move {
            match ev {
                Ok(ev) if ev.event == "state" => Some(Ok(ev.data)),
                Ok(_) => None,
                Err(e) => Some(Err(e.to_string())),
            }
        });
    Ok(Box::pin(events))
}
