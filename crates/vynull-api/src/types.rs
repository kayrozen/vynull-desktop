//! Response and request shapes. Optional fields default so that an older or
//! newer daemon that omits them still parses.

use serde::{Deserialize, Deserializer, Serialize};

/// Treat JSON `null` like a missing field (Go nil slices encode as `null`).
pub fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Accept `213`, `213.0` or `213.456` for an integer field. `PUT .../metadata`
/// answers with `"duration":0.000` while the library list sends an integer.
pub fn int_or_float<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Num {
        Int(i64),
        Float(f64),
    }
    Ok(match Option::<Num>::deserialize(d)? {
        None => 0,
        Some(Num::Int(n)) => n,
        Some(Num::Float(f)) => f.round() as i64,
    })
}

/// `{"ok": true}` acks. The API also has `{"OK": true}`; see [`AckOk`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Ack {
    #[serde(default)]
    pub ok: bool,
}

/// Capitalised `{"OK": true}` ack (historical quirk).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AckOk {
    #[serde(default, rename = "OK")]
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StatusResponse {
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub device_number: i64,
    #[serde(default)]
    pub track_count: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub peers: Vec<PeerInfo>,
    #[serde(default, deserialize_with = "null_default")]
    pub players: Vec<PlayerInfo>,
    #[serde(default)]
    pub analysis: Option<AnalysisStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AnalysisStatus {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub pending: i64,
    #[serde(default)]
    pub analyzed: i64,
    #[serde(default)]
    pub cached: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlayerInfo {
    #[serde(default)]
    pub device_number: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub track_title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub bpm: f64,
    /// -100..+100, 0 = nominal.
    #[serde(default)]
    pub pitch_pct: f64,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub artwork_url: String,
    #[serde(default)]
    pub analysis_url: String,
    #[serde(default)]
    pub waveform_url: String,
    #[serde(default)]
    pub is_playing: bool,
    #[serde(default)]
    pub is_master: bool,
    #[serde(default)]
    pub is_sync: bool,
    #[serde(default)]
    pub on_air: bool,
    /// 1..4, 0 = unknown.
    #[serde(default)]
    pub beat_in_bar: i64,
    #[serde(default)]
    pub beat_in_track: i64,
    /// Milliseconds since that beat fired: the sub-beat phase for playhead interpolation.
    #[serde(default)]
    pub beat_age_ms: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub play_state: i64,
    #[serde(default)]
    pub play_state_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NowPlaying {
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub device_number: i64,
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub beat_in_track: i64,
    #[serde(default)]
    pub beat_age_ms: i64,
    #[serde(default)]
    pub pitch_pct: f64,
    #[serde(default)]
    pub artwork_url: String,
    #[serde(default)]
    pub waveform_url: String,
}

/// Payload of each `event: state` message on `GET /api/events`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StateEvent {
    #[serde(default, deserialize_with = "null_default")]
    pub players: Vec<PlayerInfo>,
    #[serde(default)]
    pub nowplaying: NowPlaying,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    #[serde(default)]
    pub device_number: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub device_type: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default)]
    pub mixer: Option<MixerInfo>,
}

/// Only on-air state is available; the API has no fader or crossfader data.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MixerInfo {
    #[serde(default)]
    pub master_device: i64,
    #[serde(default)]
    pub master_bpm: f64,
    #[serde(default)]
    pub beat_in_bar: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub channel_on_air: Vec<bool>,
    #[serde(default)]
    pub channel_state_known: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HistoryItem {
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: String,
    #[serde(default)]
    pub device_number: i64,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub key: String,
}

/// One library row from `GET /api/tracks` (trimmed payload).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackInfo {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub key: String,
    /// Seconds (a float in some replies; see [`int_or_float`]).
    #[serde(default, deserialize_with = "int_or_float")]
    pub duration: i64,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub file_type: String,
    #[serde(default)]
    pub file_size: i64,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub original_artist: String,
    #[serde(default)]
    pub remixer: String,
    #[serde(default)]
    pub mix_name: String,
    #[serde(default)]
    pub bitrate: i64,
    #[serde(default)]
    pub year: i64,
    #[serde(default)]
    pub track_num: i64,
    #[serde(default)]
    pub rating: i64,
    #[serde(default)]
    pub color_id: i64,
    #[serde(default)]
    pub color_name: String,
    #[serde(default)]
    pub date_added: String,
    #[serde(default)]
    pub play_count: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub art_id: i64,
    #[serde(default)]
    pub art_checked: bool,
    #[serde(default)]
    pub file_missing: bool,
}

/// Body of `PUT /api/tracks/{id}/metadata`. Omitted fields stay unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackMetadataPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// A string in the patch, though `TrackInfo.year` is an integer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remixer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CueKind {
    #[default]
    Cue,
    Loop,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CueInfo {
    /// 1 = A, 2 = B, ...; memory cues number upward from 9.
    #[serde(default)]
    pub number: i64,
    #[serde(default, rename = "type")]
    pub kind: CueKind,
    #[serde(default)]
    pub time_ms: i64,
    /// Loop end in ms, -1 if not a loop.
    #[serde(default)]
    pub loop_ms: i64,
    #[serde(default)]
    pub color_id: i64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BeatInfo {
    #[serde(default)]
    pub time_ms: f64,
    /// 1-4.
    #[serde(default)]
    pub beat_in_bar: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AnalysisResult {
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub key_standard: String,
    #[serde(default)]
    pub duration_sec: i64,
    #[serde(default)]
    pub beat_count: i64,
    #[serde(default)]
    pub downbeat_index: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub beats: Vec<BeatInfo>,
    #[serde(default, deserialize_with = "null_default")]
    pub cues: Vec<CueInfo>,
}

/// One PWV5 waveform column (spectral entries).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveEntrySpectral {
    /// 0-7.
    #[serde(default)]
    pub r: u8,
    /// 0-7.
    #[serde(default)]
    pub g: u8,
    /// 0-7.
    #[serde(default)]
    pub b: u8,
    /// Height, 0-31.
    #[serde(default)]
    pub h: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TagInfo {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category_id: i64,
    #[serde(default)]
    pub color: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TagCategoryInfo {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlaylistInfo {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub parent_id: i64,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub track_ids: Vec<i64>,
    #[serde(default)]
    pub is_smart: bool,
    #[serde(default)]
    pub rules: Option<SmartRules>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SmartRules {
    /// `all` or `any`.
    #[serde(default)]
    pub combinator: String,
    #[serde(default, deserialize_with = "null_default")]
    pub conditions: Vec<SmartCond>,
}

/// A leaf condition (`field`/`operator`/`value`) or a nested `group`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SmartCond {
    #[serde(default)]
    pub group: Option<SmartRules>,
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub operator: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

/// `GET /api/analysis/waveform/{id}` JSON envelope. `E` is the entry type for
/// the requested `type`/`style`: [`WaveEntrySpectral`], [`WaveEntry3Band`] or
/// [`WaveEntryColorPreview`]. The `preview` type returns raw PWAV bytes instead.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(bound(deserialize = "E: Deserialize<'de>"))]
pub struct Waveform<E> {
    #[serde(default)]
    pub track_id: i64,
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Absent for `color_preview`.
    #[serde(default)]
    pub style: String,
    /// Entries per second (150 for detail).
    #[serde(default)]
    pub sample_rate: i64,
    #[serde(default = "Vec::new", deserialize_with = "null_default")]
    pub entries: Vec<E>,
}

/// `GET /api/analysis/ext/{player}/{slot}/{trackID}`: the analysis a deck reads from its own USB or SD.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExtAnalysis {
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub bpm: f64,
    /// Always 150.
    #[serde(default)]
    pub sample_rate: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub waveform: Vec<WaveEntrySpectral>,
    #[serde(default, deserialize_with = "null_default")]
    pub cues: Vec<CueInfo>,
    /// Beat times in ms.
    #[serde(default, deserialize_with = "null_default")]
    pub beats: Vec<f64>,
}

/// `detail&style=3band` column, 0-255 per band.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveEntry3Band {
    #[serde(default)]
    pub bass: u8,
    #[serde(default)]
    pub mid: u8,
    #[serde(default)]
    pub high: u8,
}

/// `color_preview` column, 0-255 per band.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveEntryColorPreview {
    #[serde(default)]
    pub bass: u8,
    #[serde(default)]
    pub mid: u8,
    #[serde(default)]
    pub treble: u8,
}

/// Detected song-structure phrase.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PhraseInfo {
    /// 1 = intro, 2 = up, 3 = down, 5 = chorus, 6 = outro.
    #[serde(default)]
    pub kind: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub start_ms: i64,
    #[serde(default)]
    pub end_ms: i64,
    #[serde(default)]
    pub energy: f64,
    #[serde(default)]
    pub has_vocal: bool,
    #[serde(default)]
    pub vocal_score: f64,
}

/// `GET /api/settings`: five display fields plus the whole config under `full`.
/// `full` stays untyped: its sections (`my_setting`, `dev_setting`, ...) are
/// large enum-valued maps that the settings screen edits generically.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SettingsResponse {
    #[serde(default)]
    pub waveform_size: String,
    #[serde(default)]
    pub waveform_color: String,
    #[serde(default)]
    pub waveform_position: String,
    #[serde(default)]
    pub key_display: String,
    #[serde(default)]
    pub track_detail: String,
    #[serde(default)]
    pub full: serde_json::Value,
}

/// `GET /api/settings/enums`: setting name to its allowed values.
pub type SettingsEnums = std::collections::BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MenuItem {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub item_type: i64,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackDetailField {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub high_byte: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MenuItems {
    #[serde(default, deserialize_with = "null_default")]
    pub items: Vec<MenuItem>,
    #[serde(default)]
    pub track_detail: String,
    #[serde(default, deserialize_with = "null_default")]
    pub track_detail_fields: Vec<TrackDetailField>,
}

/// Body of `PUT /api/menu-items`; both parts optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MenuItemsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<MenuItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OverlayConfig {
    #[serde(default)]
    pub position: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub accent: String,
    #[serde(default)]
    pub show_meta: bool,
    #[serde(default)]
    pub show_nowplaying: bool,
    #[serde(default)]
    pub show_waveform: bool,
    #[serde(default)]
    pub show_decks: bool,
    #[serde(default)]
    pub show_history: bool,
    #[serde(default)]
    pub history_count: i64,
    #[serde(default)]
    pub width: i64,
    #[serde(default)]
    pub label: String,
    /// `half` or `full`.
    #[serde(default)]
    pub waveform_style: String,
    #[serde(default)]
    pub waveform_band: bool,
    #[serde(default)]
    pub waveform_height: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiagResponse {
    #[serde(default)]
    pub started_unix_ms: i64,
    #[serde(default)]
    pub uptime_sec: i64,
    #[serde(default)]
    pub linked: bool,
    #[serde(default)]
    pub go_version: String,
    #[serde(default)]
    pub goroutines: i64,
    #[serde(default)]
    pub heap_mb: i64,
    #[serde(default)]
    pub sys_mb: i64,
    #[serde(default)]
    pub gc_pause_us: i64,
    #[serde(default)]
    pub gc_count: i64,
    #[serde(default)]
    pub library_tracks: i64,
    #[serde(default)]
    pub analysis_pending: i64,
    #[serde(default)]
    pub analysis_analyzed: i64,
    #[serde(default)]
    pub analysis_cached: i64,
    #[serde(default)]
    pub cache_dir_bytes: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LogLine {
    #[serde(default)]
    pub seq: i64,
    /// Unix milliseconds.
    #[serde(default)]
    pub time: i64,
    #[serde(default)]
    pub line: String,
}

/// `GET /api/diag/status`: current long-running activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiagStatus {
    /// `export`, `analysis` or empty.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Linked {
    #[serde(default)]
    pub linked: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FsEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub audio: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FsListing {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub parent: String,
    /// True for the list of allowed roots.
    #[serde(default)]
    pub roots: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub entries: Vec<FsEntry>,
}

/// Which parts of an import to run. `None` means include (the daemon treats nil as true).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImportInclude {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artwork: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cues: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<bool>,
}

/// Body of `POST /api/import/rekordbox`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImportRequest {
    /// `.xml`, `.nml`, `.db` or `.zip`.
    pub path: String,
    /// SQLCipher 64-hex key, for an encrypted `master.db` or a backup zip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remap_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remap_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<ImportInclude>,
}

/// Import summary. The daemon sends PascalCase keys (no json tags on the Go struct).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ImportResult {
    #[serde(default)]
    pub tracks_total: i64,
    #[serde(default)]
    pub tracks_added: i64,
    #[serde(default)]
    pub tracks_updated: i64,
    #[serde(default)]
    pub tracks_skipped: i64,
    #[serde(default)]
    pub playlists_total: i64,
    #[serde(default)]
    pub tags_total: i64,
    #[serde(default)]
    pub artwork_imported: i64,
    #[serde(default)]
    pub analysis_imported: i64,
    #[serde(default)]
    pub cues_imported: i64,
    #[serde(default)]
    pub settings_imported: i64,
    #[serde(default)]
    pub files_missing: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImportStatus {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub done: i64,
    #[serde(default)]
    pub total: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportPreview {
    #[serde(default)]
    pub total: i64,
}

/// Body of `POST /api/export`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportRequest {
    pub destination: String,
    /// `all`, `playlist:N`, `smart:N` or `selection:<csv ids>`. Default `all`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub destination: String,
    #[serde(default)]
    pub track_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemapResult {
    #[serde(default)]
    pub changed: i64,
    #[serde(default)]
    pub missing: i64,
}

/// Body of `POST /api/load`: give `track_id` or `file_path`, plus the deck.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LoadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Target CDJ, 1-4.
    pub device_number: i64,
}

/// Body of `POST /api/playlists`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NewPlaylist {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_folder: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_smart: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<SmartRules>,
}

/// Body of `PUT /api/playlists/{id}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlaylistPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i64>,
}

/// Body of `PUT /api/tags/{id}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TagPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_event_with_null_and_missing_fields() {
        let e: StateEvent = serde_json::from_str(
            r#"{"players":null,"nowplaying":{"playing":true,"title":"t","artist":"a","beat_age_ms":120}}"#,
        )
        .unwrap();
        assert!(e.players.is_empty());
        assert!(e.nowplaying.playing);
        assert_eq!(e.nowplaying.beat_age_ms, 120);
    }

    #[test]
    fn metadata_reply_has_float_duration_and_extra_fields() {
        let t: TrackInfo = serde_json::from_str(
            r#"{"id":2,"title":"t","duration":0.000,"bpm":0,"disc_num":0,"sample_rate":0,"comment":"x"}"#,
        )
        .unwrap();
        assert_eq!((t.duration, t.comment.as_str()), (0, "x"));
        let t: TrackInfo = serde_json::from_str(r#"{"id":2,"duration":213.6}"#).unwrap();
        assert_eq!(t.duration, 214);
    }

    #[test]
    fn both_ack_spellings() {
        assert!(serde_json::from_str::<Ack>(r#"{"ok":true}"#).unwrap().ok);
        assert!(serde_json::from_str::<AckOk>(r#"{"OK":true}"#).unwrap().ok);
    }

    #[test]
    fn track_row_without_optionals() {
        let t: TrackInfo = serde_json::from_str(r#"{"id":1,"title":"x","tags":null}"#).unwrap();
        assert_eq!(t.id, 1);
        assert!(t.tags.is_empty());
    }

    #[test]
    fn import_result_pascal_case_and_waveform_envelope() {
        let r: ImportResult =
            serde_json::from_str(r#"{"TracksAdded":3,"Errors":null,"FilesMissing":1}"#).unwrap();
        assert_eq!((r.tracks_added, r.files_missing), (3, 1));
        assert!(r.errors.is_empty());
        let w: Waveform<WaveEntry3Band> = serde_json::from_str(
            r#"{"track_id":1,"type":"detail","style":"3band","sample_rate":150,"entries":[{"bass":1,"mid":2,"high":3}]}"#,
        )
        .unwrap();
        assert_eq!(w.entries[0].high, 3);
    }

    #[test]
    fn cue_type_and_patch_skips_unset() {
        let c: CueInfo =
            serde_json::from_str(r#"{"number":9,"type":"loop","time_ms":1,"loop_ms":2}"#).unwrap();
        assert_eq!(c.kind, CueKind::Loop);
        let p = TrackMetadataPatch {
            title: Some("n".into()),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&p).unwrap(), r#"{"title":"n"}"#);
    }
}

/// `GET /api/tracks/rev`: bumps whenever the library changes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryRev {
    #[serde(default)]
    pub rev: i64,
}

/// `GET /api/tracks/{id}/beats`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BeatSummary {
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub detected_bpm: f64,
    #[serde(default)]
    pub beat_phase_shift: i64,
}

/// Body of `PUT /api/tracks/{id}/beats`. `bpm` rebuilds the grid from the first
/// beat (it does not keep the downbeat: use [`BeatgridAdjust`] for that),
/// `beat_phase_shift` renumbers which beat starts a bar, `clear` drops the override.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BeatsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat_phase_shift: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear: Option<bool>,
}

/// Body of `POST /api/analysis/beatgrid/adjust`. To change the BPM and keep
/// the downbeat in place, send `bpm` together with `set_downbeat_ms`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BeatgridAdjust {
    pub track_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_downbeat_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BeatgridAdjusted {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub bpm: f64,
    #[serde(default)]
    pub beats: i64,
    #[serde(default)]
    pub phase_ms: f64,
}
