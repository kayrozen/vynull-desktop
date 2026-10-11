//! Parses real daemon responses. Set `VYNULL_URL` (e.g. `http://127.0.0.1:9443`)
//! and run `cargo test -p vynull-api -- --ignored`. Needs `curl` on PATH.

use vynull_api::*;

fn get(path: &str) -> String {
    let base = std::env::var("VYNULL_URL").expect("VYNULL_URL not set");
    let out = std::process::Command::new("curl")
        .args(["-sf", "--compressed", "-m", "20", &format!("{base}{path}")])
        .output()
        .expect("run curl");
    assert!(out.status.success(), "GET {path} failed");
    String::from_utf8(out.stdout).unwrap()
}

fn parse<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_json::from_str(&get(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
#[ignore = "needs a live daemon"]
fn live_read_endpoints_parse() {
    let _: StatusResponse = parse("/api/status");
    let _: Vec<PlayerInfo> = parse("/api/players");
    let _: NowPlaying = parse("/api/nowplaying");
    let _: Vec<PeerInfo> = parse("/api/peers");
    let _: Vec<HistoryItem> = parse("/api/history");
    let tracks: Vec<TrackInfo> = parse("/api/tracks");
    assert!(!tracks.is_empty());
    let _: Vec<PlaylistInfo> = parse("/api/playlists");
    let _: Vec<TagInfo> = parse("/api/tags");
    let _: Vec<TagCategoryInfo> = parse("/api/tag-categories");
    let _: Vec<CueInfo> = parse(&format!("/api/tracks/{}/cues", tracks[0].id));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs a live daemon"]
async fn live_client_reads_and_etag() {
    let c = Client::new(std::env::var("VYNULL_URL").expect("VYNULL_URL not set"));
    assert!(c.library_rev().await.unwrap() >= 0);
    let Tracks::Fresh { etag, tracks } = c.tracks_if_changed(None).await.unwrap() else {
        panic!("expected a fresh library");
    };
    assert!(!tracks.is_empty());
    let etag = etag.expect("native client sees ETag");
    assert_eq!(
        c.tracks_if_changed(Some(&etag)).await.unwrap(),
        Tracks::NotModified
    );
    let t = c.track(tracks[0].id).await.unwrap();
    assert_eq!(t.id, tracks[0].id);
    c.status().await.unwrap();
    c.playlists().await.unwrap();
    assert!(matches!(
        c.track(99_999_999).await,
        Err(Error::Status { status: 404, .. })
    ));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs a live daemon"]
async fn live_client_remaining_reads() {
    let c = Client::new(std::env::var("VYNULL_URL").expect("VYNULL_URL not set"));
    let id = c.tracks().await.unwrap()[0].id;
    let w3: Waveform<WaveEntry3Band> = c.detail_waveform(id, "3band").await.unwrap();
    assert!(!w3.entries.is_empty());
    let ws: Waveform<WaveEntrySpectral> = c.detail_waveform(id, "spectral").await.unwrap();
    assert_eq!(ws.sample_rate, 150);
    assert!(!c.color_preview(id).await.unwrap().entries.is_empty());
    assert!(!c.preview_waveform(id).await.unwrap().is_empty());
    c.track_tags(id).await.unwrap();
    c.phrases(id).await.unwrap();
    c.track_beats(id).await.unwrap();
    c.analysis(id).await.unwrap();
    c.all_cues().await.unwrap();
    let s = c.settings().await.unwrap();
    assert!(s.full.is_object());
    assert!(!c.settings_enums().await.unwrap().is_empty());
    assert!(!c.menu_items().await.unwrap().items.is_empty());
    c.overlay_config().await.unwrap();
    assert!(c.diag().await.unwrap().library_tracks > 0);
    assert!(!c.diag_logs(0).await.unwrap().is_empty());
    c.diag_status().await.unwrap();
    c.linked().await.unwrap();
    c.import_status().await.unwrap();
    assert!(c.export_preview("all").await.unwrap().total > 0);
    assert!(c.fs_list(None).await.unwrap().roots);
    if let Some(p) = c.playlists().await.unwrap().first() {
        c.playlist_rules(p.id).await.unwrap();
    }
}

// ---- fixture-library tests ------------------------------------------------
// These need the generated fixture library from `ci/run-contract-tests.sh`
// (`VYNULL_FIXTURE=1`) and change data, so they skip on any other daemon.

fn on_fixture() -> Option<Client> {
    std::env::var("VYNULL_FIXTURE").ok()?;
    Some(Client::new(
        std::env::var("VYNULL_URL").expect("VYNULL_URL not set"),
    ))
}

async fn fixture(c: &Client, file: &str) -> TrackInfo {
    c.tracks()
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.file_path.ends_with(file))
        .unwrap_or_else(|| panic!("fixture {file} not in the library"))
}

/// `(status, x-audio-source)` of a Range request for the first 100 bytes.
fn audio_range(c: &Client, id: i64) -> (String, String) {
    let out = std::process::Command::new("curl")
        .args([
            "-s",
            "-m",
            "30",
            "-D",
            "-",
            "-o",
            "/dev/null",
            "-r",
            "0-99",
            &c.audio_url(id),
        ])
        .output()
        .expect("run curl");
    let head = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let status = head.lines().next().unwrap_or_default().to_string();
    let source = head
        .lines()
        .find_map(|l| l.strip_prefix("x-audio-source:"))
        .unwrap_or_default()
        .trim()
        .to_string();
    (status, source)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs the fixture daemon"]
async fn fixture_library_and_analysis() {
    let Some(c) = on_fixture() else { return };
    let tracks = c.tracks().await.unwrap();
    assert_eq!(tracks.len(), 4);

    let flac = fixture(&c, "fixture-tone.flac").await;
    assert_eq!(
        (flac.title.as_str(), flac.artist.as_str()),
        ("Café Crème", "Élève")
    );
    let mp3 = fixture(&c, "fixture-128bpm.mp3").await;
    assert_eq!(
        (mp3.title.as_str(), mp3.album.as_str()),
        ("Fixture 128", "Fixtures")
    );

    // Analysis of the synthetic click track.
    let wav = fixture(&c, "fixture-120bpm.wav").await;
    let a = c.analysis(wav.id).await.unwrap();
    assert!((a.bpm - 120.0).abs() < 1.0, "bpm {}", a.bpm);
    assert!(
        (a.duration_sec - 24).abs() <= 1,
        "duration {}",
        a.duration_sec
    );
    assert!(a.beat_count >= 44, "beats {}", a.beat_count);

    // Detail waveform: 150 entries per second.
    let w: Waveform<WaveEntry3Band> = c.detail_waveform(wav.id, "3band").await.unwrap();
    assert_eq!(w.sample_rate, 150);
    assert!(
        (w.entries.len() as i64 - 24 * 150).abs() < 150,
        "{} entries",
        w.entries.len()
    );
    let s: Waveform<WaveEntrySpectral> = c.detail_waveform(wav.id, "spectral").await.unwrap();
    assert_eq!(s.entries.len(), w.entries.len());
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs the fixture daemon"]
async fn fixture_audio_serving() {
    let Some(c) = on_fixture() else { return };
    for (file, source) in [
        ("fixture-120bpm.wav", "direct"),
        ("fixture-128bpm.mp3", "direct"),
        ("fixture-tone.flac", "direct"),
        ("fixture-tone.aiff", "transcoded"),
    ] {
        let t = fixture(&c, file).await;
        let (status, got) = audio_range(&c, t.id);
        assert!(status.contains("206"), "{file}: {status}");
        assert_eq!(got, source, "{file}");
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs the fixture daemon"]
async fn fixture_track_edits() {
    let Some(c) = on_fixture() else { return };
    let mp3 = fixture(&c, "fixture-128bpm.mp3").await;
    let id = mp3.id;

    let patch = TrackMetadataPatch {
        comment: Some("edited by ci".into()),
        ..Default::default()
    };
    assert_eq!(
        c.set_metadata(id, &patch).await.unwrap().comment,
        "edited by ci"
    );
    assert_eq!(
        c.track(id).await.unwrap().title,
        "Fixture 128",
        "omitted fields stay"
    );

    c.set_track_color(id, 3).await.unwrap();
    assert_eq!(c.track(id).await.unwrap().color_id, 3);
    c.set_track_rating(id, 4).await.unwrap();
    assert_eq!(c.track(id).await.unwrap().rating, 4);

    let cue = CueInfo {
        number: 1,
        kind: CueKind::Cue,
        time_ms: 1500,
        loop_ms: -1,
        color_id: 2,
        label: "ci".into(),
    };
    c.put_cue(id, &cue).await.unwrap();
    let cues = c.track_cues(id).await.unwrap();
    assert!(cues.iter().any(|q| q.number == 1 && q.time_ms == 1500));
    c.delete_cue(id, 1).await.unwrap();
    assert!(c
        .track_cues(id)
        .await
        .unwrap()
        .iter()
        .all(|q| q.number != 1));

    // Keep the downbeat when the BPM changes (the client sends both).
    let adj = BeatgridAdjust {
        track_id: id,
        bpm: Some(118.0),
        set_downbeat_ms: Some(500.0),
        ..Default::default()
    };
    let r = c.adjust_beatgrid(&adj).await.unwrap();
    assert!(r.ok && (r.bpm - 118.0).abs() < 0.01, "{r:?}");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs the fixture daemon"]
async fn fixture_tags_and_playlists() {
    let Some(c) = on_fixture() else { return };
    let id = fixture(&c, "fixture-tone.aiff").await.id;

    let cat = c.create_tag_category("ci-category").await.unwrap();
    let tag = c.create_tag("ci-tag", Some(cat.id)).await.unwrap();
    assert_eq!(tag.category_id, cat.id);
    c.update_tag(
        tag.id,
        &TagPatch {
            color: Some(5),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    c.set_track_tags(id, &[tag.id]).await.unwrap();
    assert!(c
        .track_tags(id)
        .await
        .unwrap()
        .iter()
        .any(|t| t.id == tag.id));
    assert!(c
        .track(id)
        .await
        .unwrap()
        .tags
        .iter()
        .any(|n| n == "ci-tag"));
    c.set_track_tags(id, &[]).await.unwrap();
    c.delete_tag(tag.id).await.unwrap();
    c.delete_tag_category(cat.id).await.unwrap();
    assert!(c.tags().await.unwrap().iter().all(|t| t.id != tag.id));

    let pl = c
        .create_playlist(&NewPlaylist {
            name: "ci-playlist".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    c.set_playlist_tracks(pl.id, &[id]).await.unwrap();
    let in_pl = c.playlist_tracks(pl.id).await.unwrap();
    assert_eq!(in_pl.iter().map(|t| t.id).collect::<Vec<_>>(), vec![id]);
    c.delete_playlist(pl.id).await.unwrap();
    assert!(c.playlists().await.unwrap().iter().all(|p| p.id != pl.id));
}
