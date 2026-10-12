#![cfg(not(target_arch = "wasm32"))]

use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use vynull_platform::{state_stream, StreamItem};

const BODY: &str = ": ping\n\n\
event: state\ndata: {\"players\":null,\"nowplaying\":{\"playing\":true,\"title\":\"a\",\"artist\":\"b\"}}\n\n\
event: other\ndata: ignored\n\n\
event: state\ndata: not json\n\n";

/// Serves `BODY` to every connection, then closes it.
async fn fake_daemon() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf).await;
            let head =
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
            let _ = s.write_all(head.as_bytes()).await;
            let _ = s.write_all(BODY.as_bytes()).await;
            let _ = s.shutdown().await;
        }
    });
    format!("http://{addr}/api/events")
}

#[tokio::test(flavor = "current_thread")]
async fn parses_state_events_and_reconnects() {
    let mut s = Box::pin(state_stream(fake_daemon().await));
    assert_eq!(s.next().await, Some(StreamItem::Open));
    let Some(StreamItem::State(ev)) = s.next().await else {
        panic!("expected a state event");
    };
    assert!(ev.nowplaying.playing && ev.players.is_empty());
    assert!(matches!(s.next().await, Some(StreamItem::Error(m)) if m.starts_with("bad state")));
    // The server closed the body: an error, then a reconnect.
    assert!(matches!(s.next().await, Some(StreamItem::Error(_))));
    assert_eq!(s.next().await, Some(StreamItem::Open));
}

/// Sends one event and then goes silent without closing the connection, as a host that has
/// dropped off the network does.
async fn silent_daemon() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new(); // keep the sockets open so they never close
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf).await;
            let head =
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: keep-alive\r\n\r\n";
            let _ = s.write_all(head.as_bytes()).await;
            let ev =
                "event: state\ndata: {\"players\":null,\"nowplaying\":{\"playing\":false}}\n\n";
            let _ = s.write_all(ev.as_bytes()).await;
            held.push(s);
        }
    });
    format!("http://{addr}/api/events")
}

#[tokio::test(flavor = "current_thread")]
async fn a_link_that_goes_silent_is_dropped_and_reopened() {
    let url = silent_daemon().await;
    let mut s = Box::pin(vynull_platform::state_stream_with(
        url,
        std::time::Duration::from_millis(300),
        std::time::Duration::from_secs(2),
    ));
    assert_eq!(s.next().await, Some(StreamItem::Open));
    assert!(matches!(s.next().await, Some(StreamItem::State(_))));
    // Nothing more arrives and the socket stays open: without an idle timeout this waits forever.
    let next = tokio::time::timeout(std::time::Duration::from_secs(5), s.next())
        .await
        .expect("the stream must notice the silence");
    assert!(
        matches!(next, Some(StreamItem::Error(m)) if m.contains("no data")),
        "an error says why"
    );
    // And it connects again by itself.
    let again = tokio::time::timeout(std::time::Duration::from_secs(5), s.next())
        .await
        .expect("it must reconnect");
    assert_eq!(again, Some(StreamItem::Open));
}

#[tokio::test(flavor = "current_thread")]
async fn unreachable_daemon_reports_errors_and_keeps_trying() {
    // Port 9 (discard) on loopback refuses connections.
    let mut s = Box::pin(state_stream("http://127.0.0.1:9/api/events".into()));
    assert!(matches!(s.next().await, Some(StreamItem::Error(_))));
    assert!(matches!(s.next().await, Some(StreamItem::Error(_))));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs a live daemon"]
async fn live_daemon_opens() {
    let base = std::env::var("VYNULL_URL").expect("VYNULL_URL not set");
    let mut s = Box::pin(state_stream(format!("{base}/api/events")));
    assert_eq!(s.next().await, Some(StreamItem::Open));
}
