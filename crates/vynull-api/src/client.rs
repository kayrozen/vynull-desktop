//! HTTP client. Runs on native and wasm32 (reqwest uses `fetch` there).
//!
//! Cross-origin note: the daemon allows only the `Content-Type` request header
//! and exposes no response headers. From another origin a browser cannot send
//! `If-None-Match` or read `ETag`, so use [`Client::library_rev`] to detect
//! library changes. [`Client::tracks_if_changed`] works same-origin and natively.

use reqwest::header::{ETAG, IF_NONE_MATCH};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::types::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// Non-2xx reply. The daemon sends `text/plain` bodies for errors.
    #[error("{status}: {body}")]
    Status { status: u16, body: String },
}

/// Result of a conditional library fetch.
#[derive(Debug, Clone, PartialEq)]
pub enum Tracks {
    NotModified,
    Fresh {
        /// `None` when the browser did not expose the header.
        etag: Option<String>,
        tracks: Vec<TrackInfo>,
    },
}

#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    /// `base` is like `http://192.168.1.10:9443` (a trailing slash is fine).
    /// An empty base gives same-origin relative URLs on the web.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// URL for an `<audio>` element or native player (supports Range).
    pub fn audio_url(&self, track_id: i64) -> String {
        self.url(&format!("/api/audio/{track_id}"))
    }

    /// Like [`Client::audio_url`] but forces the ffmpeg WAV path, whose length
    /// matches the analysis (the "WAV option" for the length difference).
    pub fn audio_url_transcoded(&self, track_id: i64) -> String {
        self.url(&format!("/api/audio/{track_id}?transcode=1"))
    }

    /// URL of the SSE stream.
    pub fn events_url(&self) -> String {
        self.url("/api/events")
    }

    async fn check(resp: reqwest::Response) -> Result<reqwest::Response, Error> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Status {
            status: status.as_u16(),
            body: body.trim().to_string(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = Self::check(self.http.get(self.url(path)).send().await?).await?;
        Ok(resp.json().await?)
    }

    async fn send<B: Serialize, T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let req = self.http.request(method, self.url(path)).json(body);
        Ok(Self::check(req.send().await?).await?.json().await?)
    }

    pub async fn status(&self) -> Result<StatusResponse, Error> {
        self.get("/api/status").await
    }

    pub async fn players(&self) -> Result<Vec<PlayerInfo>, Error> {
        self.get("/api/players").await
    }

    pub async fn now_playing(&self) -> Result<NowPlaying, Error> {
        self.get("/api/nowplaying").await
    }

    pub async fn peers(&self) -> Result<Vec<PeerInfo>, Error> {
        self.get("/api/peers").await
    }

    pub async fn history(&self) -> Result<Vec<HistoryItem>, Error> {
        self.get("/api/history").await
    }

    pub async fn library_rev(&self) -> Result<i64, Error> {
        Ok(self.get::<LibraryRev>("/api/tracks/rev").await?.rev)
    }

    /// Fetch the whole library, or `NotModified` if `etag` still matches.
    pub async fn tracks_if_changed(&self, etag: Option<&str>) -> Result<Tracks, Error> {
        let mut req = self.http.get(self.url("/api/tracks"));
        if let Some(tag) = etag {
            req = req.header(IF_NONE_MATCH, tag);
        }
        let resp = req.send().await?;
        if resp.status() == StatusCode::NOT_MODIFIED {
            return Ok(Tracks::NotModified);
        }
        let resp = Self::check(resp).await?;
        let etag = resp
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        Ok(Tracks::Fresh {
            etag,
            tracks: resp.json().await?,
        })
    }

    pub async fn tracks(&self) -> Result<Vec<TrackInfo>, Error> {
        self.get("/api/tracks").await
    }

    pub async fn track(&self, id: i64) -> Result<TrackInfo, Error> {
        self.get(&format!("/api/tracks/{id}")).await
    }

    pub async fn playlists(&self) -> Result<Vec<PlaylistInfo>, Error> {
        self.get("/api/playlists").await
    }

    pub async fn playlist_tracks(&self, id: i64) -> Result<Vec<TrackInfo>, Error> {
        self.get(&format!("/api/playlists/{id}/tracks")).await
    }

    pub async fn tags(&self) -> Result<Vec<TagInfo>, Error> {
        self.get("/api/tags").await
    }

    pub async fn tag_categories(&self) -> Result<Vec<TagCategoryInfo>, Error> {
        self.get("/api/tag-categories").await
    }

    pub async fn track_cues(&self, id: i64) -> Result<Vec<CueInfo>, Error> {
        self.get(&format!("/api/tracks/{id}/cues")).await
    }

    pub async fn track_beats(&self, id: i64) -> Result<BeatSummary, Error> {
        self.get(&format!("/api/tracks/{id}/beats")).await
    }

    /// Set the BPM, the bar phase, or clear the override. See [`BeatsPatch`].
    pub async fn set_beats(&self, id: i64, patch: &BeatsPatch) -> Result<BeatSummary, Error> {
        self.send(
            reqwest::Method::PUT,
            &format!("/api/tracks/{id}/beats"),
            patch,
        )
        .await
    }

    pub async fn analysis(&self, id: i64) -> Result<AnalysisResult, Error> {
        self.get(&format!("/api/analysis/{id}")).await
    }

    /// Edit metadata. Returns the updated track.
    pub async fn set_metadata(
        &self,
        id: i64,
        patch: &TrackMetadataPatch,
    ) -> Result<TrackInfo, Error> {
        self.send(
            reqwest::Method::PUT,
            &format!("/api/tracks/{id}/metadata"),
            patch,
        )
        .await
    }

    /// Shift, re-tempo, or set the downbeat of a beat grid.
    pub async fn adjust_beatgrid(&self, req: &BeatgridAdjust) -> Result<BeatgridAdjusted, Error> {
        self.send(reqwest::Method::POST, "/api/analysis/beatgrid/adjust", req)
            .await
    }

    async fn action<B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<(), Error> {
        let mut req = self.http.request(method, self.url(path));
        if let Some(b) = body {
            req = req.json(b);
        }
        Self::check(req.send().await?).await?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<(), Error> {
        self.action::<()>(reqwest::Method::DELETE, path, None).await
    }

    // ---- library and track edits

    pub async fn track_tags(&self, id: i64) -> Result<Vec<TagInfo>, Error> {
        self.get(&format!("/api/tracks/{id}/tags")).await
    }

    /// Replace the track's tag set.
    pub async fn set_track_tags(&self, id: i64, tag_ids: &[i64]) -> Result<(), Error> {
        let body = serde_json::json!({ "tag_ids": tag_ids });
        self.action(
            reqwest::Method::POST,
            &format!("/api/tracks/{id}/tags"),
            Some(&body),
        )
        .await
    }

    pub async fn set_track_color(&self, id: i64, color_id: i64) -> Result<(), Error> {
        let body = serde_json::json!({ "color_id": color_id });
        self.action(
            reqwest::Method::PUT,
            &format!("/api/tracks/{id}/color"),
            Some(&body),
        )
        .await
    }

    pub async fn set_track_rating(&self, id: i64, rating: i64) -> Result<(), Error> {
        let body = serde_json::json!({ "rating": rating });
        self.action(
            reqwest::Method::PUT,
            &format!("/api/tracks/{id}/rating"),
            Some(&body),
        )
        .await
    }

    /// Relink a moved file (library DB only). Returns the updated track.
    pub async fn relink_track(&self, id: i64, path: &str) -> Result<TrackInfo, Error> {
        let body = serde_json::json!({ "path": path });
        self.send(
            reqwest::Method::PUT,
            &format!("/api/tracks/{id}/path"),
            &body,
        )
        .await
    }

    pub async fn delete_track(&self, id: i64) -> Result<(), Error> {
        self.delete(&format!("/api/tracks/{id}")).await
    }

    pub async fn phrases(&self, id: i64) -> Result<Vec<PhraseInfo>, Error> {
        self.get(&format!("/api/tracks/{id}/phrases")).await
    }

    /// Create or overwrite one cue.
    pub async fn put_cue(&self, id: i64, cue: &CueInfo) -> Result<(), Error> {
        self.action(
            reqwest::Method::POST,
            &format!("/api/tracks/{id}/cues"),
            Some(cue),
        )
        .await
    }

    pub async fn delete_cue(&self, id: i64, number: i64) -> Result<(), Error> {
        self.delete(&format!("/api/tracks/{id}/cues/{number}"))
            .await
    }

    /// All cues, keyed by track ID.
    pub async fn all_cues(&self) -> Result<std::collections::HashMap<i64, Vec<CueInfo>>, Error> {
        let raw: std::collections::HashMap<String, Vec<CueInfo>> = self.get("/api/cues").await?;
        Ok(raw
            .into_iter()
            .filter_map(|(k, v)| k.parse().ok().map(|id| (id, v)))
            .collect())
    }

    /// Re-run analysis for a track.
    pub async fn reanalyze(&self, id: i64) -> Result<(), Error> {
        self.action::<()>(
            reqwest::Method::POST,
            &format!("/api/analysis/reanalyze/{id}"),
            None,
        )
        .await
    }

    /// Detail waveform. Pick the entry type that matches `style`: spectral gives
    /// [`WaveEntrySpectral`], `3band` gives [`WaveEntry3Band`].
    pub async fn detail_waveform<E: DeserializeOwned>(
        &self,
        id: i64,
        style: &str,
    ) -> Result<Waveform<E>, Error> {
        self.get(&format!(
            "/api/analysis/waveform/{id}?type=detail&style={style}"
        ))
        .await
    }

    pub async fn color_preview(&self, id: i64) -> Result<Waveform<WaveEntryColorPreview>, Error> {
        self.get(&format!("/api/analysis/waveform/{id}?type=color_preview"))
            .await
    }

    /// Raw PWAV preview bytes.
    pub async fn preview_waveform(&self, id: i64) -> Result<Vec<u8>, Error> {
        let url = self.url(&format!("/api/analysis/waveform/{id}?type=preview"));
        let resp = Self::check(self.http.get(url).send().await?).await?;
        Ok(resp.bytes().await?.to_vec())
    }

    /// The analysis of a track a deck plays from its own media. `path` is the player's
    /// `analysis_url`. A 404 means the daemon has not read it yet: ask again later.
    pub async fn ext_analysis(&self, path: &str) -> Result<ExtAnalysis, Error> {
        self.get(path).await
    }

    /// URL of the server-rendered waveform PNG.
    pub fn waveform_png_url(&self, id: i64, w: u32, h: u32) -> String {
        self.url(&format!("/api/analysis/waveform-png/{id}?w={w}&h={h}"))
    }

    pub fn artwork_url(&self, art_id: i64) -> String {
        self.url(&format!("/api/artwork/{art_id}"))
    }

    // ---- playlists

    pub async fn create_playlist(&self, new: &NewPlaylist) -> Result<PlaylistInfo, Error> {
        self.send(reqwest::Method::POST, "/api/playlists", new)
            .await
    }

    pub async fn update_playlist(&self, id: i64, patch: &PlaylistPatch) -> Result<(), Error> {
        self.action(
            reqwest::Method::PUT,
            &format!("/api/playlists/{id}"),
            Some(patch),
        )
        .await
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<(), Error> {
        self.delete(&format!("/api/playlists/{id}")).await
    }

    /// Set membership (the daemon rejects this for smart playlists).
    pub async fn set_playlist_tracks(&self, id: i64, track_ids: &[i64]) -> Result<(), Error> {
        let body = serde_json::json!({ "track_ids": track_ids });
        self.action(
            reqwest::Method::POST,
            &format!("/api/playlists/{id}/tracks"),
            Some(&body),
        )
        .await
    }

    /// `None` if the playlist is not smart.
    pub async fn playlist_rules(&self, id: i64) -> Result<Option<SmartRules>, Error> {
        self.get(&format!("/api/playlists/{id}/rules")).await
    }

    pub async fn set_playlist_rules(&self, id: i64, rules: &SmartRules) -> Result<(), Error> {
        self.action(
            reqwest::Method::PUT,
            &format!("/api/playlists/{id}/rules"),
            Some(rules),
        )
        .await
    }

    // ---- tags

    pub async fn create_tag(&self, name: &str, category_id: Option<i64>) -> Result<TagInfo, Error> {
        let body = serde_json::json!({ "name": name, "category_id": category_id });
        self.send(reqwest::Method::POST, "/api/tags", &body).await
    }

    pub async fn update_tag(&self, id: i64, patch: &TagPatch) -> Result<(), Error> {
        self.action(
            reqwest::Method::PUT,
            &format!("/api/tags/{id}"),
            Some(patch),
        )
        .await
    }

    pub async fn delete_tag(&self, id: i64) -> Result<(), Error> {
        self.delete(&format!("/api/tags/{id}")).await
    }

    pub async fn create_tag_category(&self, name: &str) -> Result<TagCategoryInfo, Error> {
        let body = serde_json::json!({ "name": name });
        self.send(reqwest::Method::POST, "/api/tag-categories", &body)
            .await
    }

    pub async fn rename_tag_category(&self, id: i64, name: &str) -> Result<(), Error> {
        let body = serde_json::json!({ "name": name });
        self.action(
            reqwest::Method::PUT,
            &format!("/api/tag-categories/{id}"),
            Some(&body),
        )
        .await
    }

    pub async fn delete_tag_category(&self, id: i64) -> Result<(), Error> {
        self.delete(&format!("/api/tag-categories/{id}")).await
    }

    // ---- playback

    /// Load a track onto a deck.
    pub async fn load(&self, req: &LoadRequest) -> Result<(), Error> {
        self.action(reqwest::Method::POST, "/api/load", Some(req))
            .await
    }

    // ---- import and export

    pub async fn import(&self, req: &ImportRequest) -> Result<ImportResult, Error> {
        self.send(reqwest::Method::POST, "/api/import/rekordbox", req)
            .await
    }

    pub async fn import_status(&self) -> Result<ImportStatus, Error> {
        self.get("/api/import/status").await
    }

    pub async fn export_preview(&self, source: &str) -> Result<ExportPreview, Error> {
        let resp = self
            .http
            .get(self.url("/api/export/preview"))
            .query(&[("source", source)])
            .send()
            .await?;
        Ok(Self::check(resp).await?.json().await?)
    }

    pub async fn export(&self, req: &ExportRequest) -> Result<ExportResult, Error> {
        self.send(reqwest::Method::POST, "/api/export", req).await
    }

    /// Read the file tags again. An empty list means every track. Returns how many were updated.
    pub async fn reimport_tracks(&self, ids: &[i64]) -> Result<i64, Error> {
        let body = if ids.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({ "track_ids": ids })
        };
        let reply: serde_json::Value = self
            .send(reqwest::Method::POST, "/api/tracks/reimport", &body)
            .await?;
        Ok(reply
            .get("updated")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0))
    }

    pub async fn remap_paths(&self, from: &str, to: &str) -> Result<RemapResult, Error> {
        let body = serde_json::json!({ "from": from, "to": to });
        self.send(reqwest::Method::POST, "/api/library/remap-paths", &body)
            .await
    }

    /// Browse directories. `None` lists the allowed roots.
    pub async fn fs_list(&self, path: Option<&str>) -> Result<FsListing, Error> {
        let mut req = self.http.get(self.url("/api/fs/list"));
        if let Some(p) = path {
            req = req.query(&[("path", p)]);
        }
        Ok(Self::check(req.send().await?).await?.json().await?)
    }

    // ---- settings, menu, overlay

    pub async fn settings(&self) -> Result<SettingsResponse, Error> {
        self.get("/api/settings").await
    }

    /// Send `{"full": <config>}` to replace the config, or the display fields to tweak them.
    pub async fn update_settings(
        &self,
        body: &serde_json::Value,
    ) -> Result<SettingsResponse, Error> {
        self.send(reqwest::Method::POST, "/api/settings", body)
            .await
    }

    pub async fn settings_enums(&self) -> Result<SettingsEnums, Error> {
        self.get("/api/settings/enums").await
    }

    pub async fn menu_items(&self) -> Result<MenuItems, Error> {
        self.get("/api/menu-items").await
    }

    pub async fn update_menu_items(&self, patch: &MenuItemsPatch) -> Result<(), Error> {
        self.action(reqwest::Method::PUT, "/api/menu-items", Some(patch))
            .await
    }

    pub async fn reset_menu_items(&self) -> Result<(), Error> {
        self.action::<()>(reqwest::Method::POST, "/api/menu-items/reset", None)
            .await
    }

    pub async fn overlay_config(&self) -> Result<OverlayConfig, Error> {
        self.get("/api/overlay/config").await
    }

    pub async fn set_overlay_config(&self, cfg: &OverlayConfig) -> Result<OverlayConfig, Error> {
        self.send(reqwest::Method::PUT, "/api/overlay/config", cfg)
            .await
    }

    // ---- link and diagnostics

    pub async fn linked(&self) -> Result<bool, Error> {
        Ok(self.get::<Linked>("/api/link").await?.linked)
    }

    pub async fn set_linked(&self, linked: bool) -> Result<bool, Error> {
        let body = Linked { linked };
        Ok(self
            .send::<_, Linked>(reqwest::Method::POST, "/api/link", &body)
            .await?
            .linked)
    }

    /// Tear down dbserver sessions and drop peers.
    pub async fn unlink(&self) -> Result<(), Error> {
        self.action::<()>(reqwest::Method::POST, "/api/unlink", None)
            .await
    }

    pub async fn diag(&self) -> Result<DiagResponse, Error> {
        self.get("/api/diag").await
    }

    /// Log lines after `since` (the last `seq` seen; 0 for all).
    pub async fn diag_logs(&self, since: i64) -> Result<Vec<LogLine>, Error> {
        self.get(&format!("/api/diag/logs?since={since}")).await
    }

    pub async fn diag_status(&self) -> Result<DiagStatus, Error> {
        self.get("/api/diag/status").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let c = Client::new("http://h:9443/");
        assert_eq!(c.audio_url(7), "http://h:9443/api/audio/7");
        assert_eq!(
            c.audio_url_transcoded(7),
            "http://h:9443/api/audio/7?transcode=1"
        );
        assert_eq!(c.events_url(), "http://h:9443/api/events");
        assert_eq!(Client::new("").url("/api/status"), "/api/status");
    }
}
