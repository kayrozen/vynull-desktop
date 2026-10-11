#!/usr/bin/env python3
"""A scripted stand-in for a Vynull daemon, for the browser tests.

It serves what the Players and Library screens read: a synthetic library of 1,200
tracks (with accents, tags, colors and a few missing files), four decks over SSE,
a mixer, history, playlists and cues. It accepts the writes the tests make and
records them at `GET /__log`. It talks to nothing else: no real daemon, no files.

Usage: fake-daemon.py PORT
"""
import base64
import json
import sys
import threading
import time
from datetime import datetime, timedelta
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 18444
T0 = time.time()
LOCK = threading.Lock()
LOG = []  # (method, path, body) of the writes
GETS = []  # paths of the reads, for tests that need to know what a client asked for

GENRES = ["Deep House", "Tech House", "Techno", "Melodic", "House", "Drum & Bass"]
KEYS = ["9A", "8A", "10A", "11A", "4A", "9B", "3B", "5A"]
TAG_NAMES = ["Drum & Bass", "Techno", "House", "Electronica", "Trance", "vocal-high", "vocal-low", "energy-high", "energy-med", "energy-low", "dark", "warm-up"]
TAGS = [{"id": i + 1, "name": n, "category_id": 1 + i // 4} for i, n in enumerate(TAG_NAMES)]
CATS = [{"id": 1, "name": "Genre"}, {"id": 2, "name": "Vocal"}, {"id": 3, "name": "Mood"}]


def make_tracks(n=1200):
    out = []
    for i in range(1, n + 1):
        accent = {0: "Thème", 1: "Café", 2: "Élan"}.get(i % 9)
        title = f"Sample Track {i:04d}" + (f" {accent}" if accent else "")
        tags = [TAG_NAMES[(i + k) % len(TAG_NAMES)] for k in range(1 + i % 3)]
        t = {
            "id": i, "title": title, "artist": f"Sample Artist {i % 37:02d}", "album": f"Sample Album {i % 11:02d}",
            "genre": GENRES[i % len(GENRES)], "bpm": 100 + (i * 7) % 80 + 0.5 * (i % 2), "key": KEYS[i % len(KEYS)],
            "duration": 120 + (i * 13) % 300, "file_path": f"/music/sample-{i:04d}.flac", "file_type": "flac",
            "comment": f"note {i % 5}: " + ("Female vocal at bar 33" if i % 10 == 0 else "plain"),
            "label": f"Sample Label {'ABC'[i % 3]}", "remixer": f"Remixer {i % 4}", "year": 2000 + i % 25,
            "rating": i % 6, "tags": tags, "art_id": (9000 + i) if i % 3 == 0 else 0, "art_checked": True, "color_id": (i % 9) if i % 4 == 0 else 0,
        }
        if i % 400 == 3:
            t["file_missing"] = True
        if t["color_id"] == 0:
            del t["color_id"]  # the daemon omits it when 0
        out.append(t)
    return out


TRACKS = make_tracks()
for _t in TAGS:
    _t["color"] = {1: 5, 2: 2}.get(_t["id"], 0)  # a few tags with a palette color
BY_ID = {t["id"]: t for t in TRACKS}
PLAYLISTS = [
    {"id": 1, "name": "Warm-up", "parent_id": 0, "is_folder": False, "sort_order": 0, "track_ids": list(range(1, 23))},
    {"id": 2, "name": "Archive", "parent_id": 0, "is_folder": True, "sort_order": 1, "track_ids": []},
    {"id": 3, "name": "Summer sets", "parent_id": 2, "is_folder": False, "sort_order": 0, "track_ids": [5, 9, 12]},
    {"id": 4, "name": "DNB", "parent_id": 0, "is_folder": False, "sort_order": 2, "is_smart": True, "track_ids": [],
     "rules": {"combinator": "all", "conditions": [{"field": "genre", "operator": "is", "value": "Drum & Bass"}]}},
]
NEXT_PLAYLIST = [10]
EXT_ASKS = {}
NEXT_ID = {"tag": 100, "cat": 100}

SETTINGS = {
    "my_setting": {"quantize": "on", "auto_cue": "on", "time_mode": "remain", "language": "english", "jog_mode": "vinyl", "tempo_range": "ten", "sync": "off"},
    "my_setting_2": {"beat_jump_beat_value": "sixteen", "vinyl_speed_adjust": "touch", "jog_display_mode": "auto"},
    "djm_my_setting": {"channel_fader_curve": "linear", "cross_fader_curve": "fast_cut", "mic_low_cut": "on"},
    "dev_setting": {"overview_type": "half", "waveform_color": "blue", "key_display": "classic", "wave_position": "center"},
    "track_detail": "artist", "bpm_range_min": 100, "bpm_range_max": 200,
}
ENUMS = {
    "quantize": ["off", "on"], "auto_cue": ["off", "on"], "time_mode": ["elapsed", "remain"], "language": ["english", "french", "german"],
    "jog_mode": ["cdj", "vinyl"], "tempo_range": ["six", "ten", "sixteen", "wide"], "sync": ["off", "on"],
    "beat_jump_beat_value": ["half", "one", "two", "four", "eight", "sixteen", "thirty_two", "sixty_four"],
    "vinyl_speed_adjust": ["touch_release", "touch", "release"], "jog_display_mode": ["auto", "info", "simple", "artwork"],
    "channel_fader_curve": ["steep_top", "linear", "steep_bottom"], "cross_fader_curve": ["constant", "slow_cut", "fast_cut"], "mic_low_cut": ["off", "on"],
    "overview_type": ["half", "full"], "waveform_color": ["blue", "rgb", "3band"], "key_display": ["classic", "alphanumeric"], "wave_position": ["center", "left"],
}
MENU = {"items": [
    {"key": "artist", "label": "ARTIST", "id": 2, "item_type": 129, "visible": True, "locked": False},
    {"key": "album", "label": "ALBUM", "id": 3, "item_type": 130, "visible": True, "locked": False},
    {"key": "track", "label": "TRACK", "id": 4, "item_type": 131, "visible": True, "locked": True},
    {"key": "bpm", "label": "BPM", "id": 6, "item_type": 133, "visible": True, "locked": False},
    {"key": "genre", "label": "GENRE", "id": 1, "item_type": 128, "visible": False, "locked": False},
], "track_detail": "artist", "track_detail_fields": [
    {"key": "bpm", "label": "BPM", "high_byte": 13}, {"key": "artist", "label": "Artist", "high_byte": 7}, {"key": "genre", "label": "Genre", "high_byte": 6}]}
MENU_DEFAULT = json.loads(json.dumps(MENU))
OVERLAY = {"position": "bottom-left", "style": "vinyl", "accent": "#ff7714", "show_meta": True, "show_nowplaying": True, "show_waveform": True,
           "show_decks": False, "show_history": False, "history_count": 5, "width": 440, "label": "Now Playing", "waveform_style": "full",
           "waveform_band": False, "waveform_height": 120}
LOGS = [{"seq": i + 1, "time": 1791724900000 + i * 1000, "line": f"2026/10/11 09:21:{40 + i:02d} {m}"} for i, m in enumerate([
    "started with args: [--interface eno1 --web]", "library: loaded 1200 tracks", "link: player 1 playing, bar 41", "nfs: read track 2 offset 0 length 65536",
    "dbserver: browse artist request from player 2", "analysis: track 14 queued", "nfs: file not found for track 31", "dbserver error: bind discovery port 12523",
    "analysis: track 14 beat grid done, 122.0 BPM", "link: mixer channel 3 on air"])]
HISTORY_DATE = [datetime.now().astimezone()]


def history():
    now = HISTORY_DATE[0]
    fmt = lambda d: d.isoformat(timespec="seconds")
    return [
        {"started_at": fmt(now - timedelta(days=1)), "device_number": 1, "device_name": "CDJ", "track_id": 8, "title": BY_ID[8]["title"], "bpm": 120.0},
        {"started_at": fmt(now - timedelta(minutes=30)), "device_number": 3, "device_name": "CDJ", "track_id": 12, "title": BY_ID[12]["title"], "bpm": 126.0},
        {"started_at": fmt(now - timedelta(minutes=5)), "device_number": 1, "device_name": "CDJ", "track_id": 5, "title": BY_ID[5]["title"], "bpm": 122.0},
    ]


def players():
    t = time.time() - T0
    out = []

    def deck(n, tid, state, playing, master, onair, bpm, pitch, ext, key, start):
        d = BY_ID.get(tid)
        beat_len = 60.0 / bpm
        elapsed = start + (t if playing else 0)
        beats = int(elapsed / beat_len) + 1
        age = int((elapsed % beat_len) * 1000) if playing else 0
        out.append({
            "device_number": n, "name": "CDJ-3000", "track_id": tid, "track_title": d["title"] if d else "", "artist": d["artist"] if d else "",
            "bpm": bpm, "pitch_pct": pitch, "key": key, "external": ext, "source": "own usb" if ext else "vynull",
            "analysis_url": "/api/analysis/ext/3/3/%d" % tid if ext else "", "is_playing": playing, "is_master": master, "is_sync": n in (1, 3), "on_air": onair, "beat_in_bar": ((beats - 1) % 4) + 1,
            "beat_in_track": beats, "beat_age_ms": age, "duration_ms": (d["duration"] * 1000 if d else 0),
            "play_state": 3 if playing else 5, "play_state_name": state,
        })

    deck(1, 1, "playing", True, True, True, 122.0, 0.0, False, "9A", 30)
    deck(2, 2, "cued", False, False, False, 124.0, 1.6, False, "8A", 4)
    deck(3, 3, "playing", True, False, True, 126.0, -0.4, True, "4A", 61)
    out.append({"device_number": 4, "name": "CDJ-3000", "track_id": 0, "track_title": "", "artist": "", "bpm": 0, "pitch_pct": 0, "key": "",
                "is_playing": False, "is_master": False, "on_air": False, "beat_in_bar": 0, "play_state_name": "empty"})
    return out


PEERS = [{"device_number": n, "name": "CDJ-3000", "device_type": "cdj", "ip": f"169.254.20.{n}", "mac": "aa:bb"} for n in (1, 2, 3)] + [
    {"device_number": 33, "name": "DJM-V10", "device_type": "djm", "ip": "169.254.20.9", "mac": "aa:cc",
     "mixer": {"master_device": 1, "master_bpm": 122.0, "beat_in_bar": 2, "channel_on_air": [True, False, True, False], "channel_state_known": True}}]


def smart_tracks(rules):
    conds = rules["conditions"]
    ok = lambda t: all(t.get(c["field"]) == c["value"] for c in conds if c["operator"] == "is")
    return [t for t in TRACKS if ok(t)]


def settings_reply():
    d = SETTINGS.get("dev_setting", {})
    return {"waveform_size": d.get("overview_type", "half"), "waveform_color": d.get("waveform_color", "blue"), "waveform_position": d.get("wave_position", "center"),
            "key_display": d.get("key_display", "classic"), "track_detail": SETTINGS.get("track_detail", ""), "full": SETTINGS}


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def send(self, code, body, ctype="application/json"):
        b = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        for k, v in (("Access-Control-Allow-Origin", "*"), ("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, HEAD, OPTIONS"),
                     ("Access-Control-Allow-Headers", "Content-Type")):
            self.send_header(k, v)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_OPTIONS(self):
        self.send(204, b"")

    def body(self):
        n = int(self.headers.get("Content-Length", 0))
        return json.loads(self.rfile.read(n) or b"{}") if n else {}

    def do_POST(self):
        path, body = urlparse(self.path).path, self.body()
        with LOCK:
            LOG.append(["POST", path, body])
            if path == "/api/load":
                return self.send(200, {"status": "ok"})
            if path == "/api/playlists":
                pid = NEXT_PLAYLIST[0]
                NEXT_PLAYLIST[0] += 1
                p = {"id": pid, "name": body.get("name", ""), "parent_id": body.get("parent_id", 0), "is_folder": bool(body.get("is_folder")),
                     "sort_order": 99, "track_ids": [], "is_smart": bool(body.get("is_smart")), "rules": body.get("rules")}
                PLAYLISTS.append(p)
                return self.send(200, p)
            if path.startswith("/api/playlists/") and path.endswith("/tracks"):
                pid = int(path.split("/")[3])
                for p in PLAYLISTS:
                    if p["id"] == pid:
                        p["track_ids"] = body.get("track_ids", [])
                return self.send(200, {"OK": True})
            if path == "/api/tags":
                NEXT_ID["tag"] += 1
                t = {"id": NEXT_ID["tag"], "name": body.get("name", ""), "category_id": body.get("category_id") or 1, "color": 0}
                TAGS.append(t)
                return self.send(200, t)
            if path == "/api/tag-categories":
                NEXT_ID["cat"] += 1
                k = {"id": NEXT_ID["cat"], "name": body.get("name", "")}
                CATS.append(k)
                return self.send(200, k)
            if path == "/api/settings":
                if "full" in body:
                    SETTINGS.clear()
                    SETTINGS.update(body["full"])
                return self.send(200, settings_reply())
            if path == "/api/menu-items/reset":
                MENU.clear()
                MENU.update(json.loads(json.dumps(MENU_DEFAULT)))
                return self.send(200, {"OK": True})
            if path == "/api/unlink":
                return self.send(200, {"sessions_closed": 2})
            if path == "/api/tracks/reimport":
                return self.send(200, {"status": "ok", "updated": len(TRACKS)})
            if path == "/api/library/remap-paths":
                return self.send(200, {"changed": 3, "missing": 1})
            if path == "/api/import/rekordbox":
                return self.send(200, {"TracksTotal": 3, "TracksAdded": 2, "TracksUpdated": 1, "TracksSkipped": 0, "Errors": None})
            if path == "/api/export":
                return self.send(200, {"ok": True, "source": body.get("source"), "destination": body.get("destination"), "track_count": 4})
        self.send(403, b"fake daemon: this write is not supported", "text/plain")

    def do_PUT(self):
        path, body = urlparse(self.path).path, self.body()
        with LOCK:
            LOG.append(["PUT", path, body])
            parts = path.strip("/").split("/")
            if path.startswith("/api/tags/"):
                t = next((x for x in TAGS if x["id"] == int(parts[2])), None)
                if not t:
                    return self.send(404, b"no such tag", "text/plain")
                old = t["name"]
                for k in ("name", "category_id", "color"):
                    if k in body:
                        t[k] = body[k]
                if old != t["name"]:
                    for tr in TRACKS:
                        tr["tags"] = [t["name"] if n == old else n for n in tr["tags"]]
                return self.send(200, {"OK": True})
            if path.startswith("/api/tag-categories/"):
                k = next((x for x in CATS if x["id"] == int(parts[2])), None)
                if not k:
                    return self.send(404, b"no such category", "text/plain")
                k["name"] = body.get("name", k["name"])
                return self.send(200, {"OK": True})
            if path == "/api/menu-items":
                if "items" in body:
                    MENU["items"] = body["items"]
                if "track_detail" in body:
                    MENU["track_detail"] = body["track_detail"]
                return self.send(200, {"OK": True})
            if path == "/api/overlay/config":
                kept = dict(body)
                kept["width"] = max(280, min(1000, int(kept.get("width", 440))))
                kept["history_count"] = max(1, min(10, int(kept.get("history_count", 5))))
                kept["waveform_height"] = max(40, min(300, int(kept.get("waveform_height", 120))))
                OVERLAY.clear()
                OVERLAY.update(kept)
                return self.send(200, OVERLAY)
        self.send(403, b"fake daemon: this write is not supported", "text/plain")

    def do_DELETE(self):
        path = urlparse(self.path).path
        with LOCK:
            LOG.append(["DELETE", path, None])
            parts = path.strip("/").split("/")
            if path.startswith("/api/tags/"):
                tid = int(parts[2])
                t = next((x for x in TAGS if x["id"] == tid), None)
                if t:
                    TAGS.remove(t)
                    for tr in TRACKS:
                        tr["tags"] = [n for n in tr["tags"] if n != t["name"]]
                return self.send(200, {"OK": True})
            if path.startswith("/api/tag-categories/"):
                cid = int(parts[2])
                CATS[:] = [x for x in CATS if x["id"] != cid]
                TAGS[:] = [x for x in TAGS if x["category_id"] != cid]
                return self.send(200, {"OK": True})
        self.send(403, b"fake daemon: this write is not supported", "text/plain")

    def do_GET(self):
        u = urlparse(self.path)
        p, q = u.path, parse_qs(u.query)
        if p == "/__gets":
            return self.send(200, GETS)
        if p != "/api/events":
            GETS.append(self.path)
        if p == "/api/events":
            self.send_response(200)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            try:
                while True:
                    pl = players()
                    a = pl[0]
                    np = {"playing": True, "device_number": 1, "track_id": a["track_id"], "title": a["track_title"], "artist": a["artist"],
                          "bpm": a["bpm"], "key": a["key"], "duration_ms": a["duration_ms"]}
                    self.wfile.write(f"event: state\ndata: {json.dumps({'players': pl, 'nowplaying': np})}\n\n".encode())
                    self.wfile.flush()
                    time.sleep(0.25)
            except Exception:
                return
        routes = {
            "/__log": lambda: LOG,
            "/api/peers": lambda: PEERS,
            "/api/history": history,
            "/api/link": lambda: {"linked": True},
            "/api/players": players,
            "/api/tracks": lambda: TRACKS,
            "/api/tracks/rev": lambda: {"rev": 1},
            "/api/tags": lambda: TAGS,
            "/api/tag-categories": lambda: CATS,
            "/api/playlists": lambda: PLAYLISTS,
            "/api/import/status": lambda: {"active": False, "phase": "", "done": 0, "total": 0},
            "/api/export/preview": lambda: {"total": 4 if q.get("source", ["all"])[0] != "all" else len(TRACKS)},
            "/api/fs/list": lambda: ({"path": "", "parent": "", "roots": True, "entries": [{"name": "/media", "path": "/media", "is_dir": True, "audio": False}]}
                                     if "path" not in q else {"path": q["path"][0], "parent": "", "roots": False, "entries": [
                                         {"name": "library.xml", "path": q["path"][0] + "/library.xml", "is_dir": False, "audio": False},
                                         {"name": "song.mp3", "path": q["path"][0] + "/song.mp3", "is_dir": False, "audio": True}]}),
            "/api/settings": lambda: settings_reply(),
            "/api/settings/enums": lambda: ENUMS,
            "/api/menu-items": lambda: MENU,
            "/api/overlay/config": lambda: OVERLAY,
            "/api/diag": lambda: {"started_unix_ms": 1791724900000, "uptime_sec": 11532, "linked": True, "go_version": "go1.25.7", "goroutines": 39,
                                  "heap_mb": 92, "sys_mb": 474, "gc_pause_us": 57, "gc_count": 103, "library_tracks": len(TRACKS),
                                  "analysis_pending": 12, "analysis_analyzed": 1188, "analysis_cached": 1190, "cache_dir_bytes": 1_503_238_553},
            "/api/diag/status": lambda: {"kind": "analysis", "text": "Analyzing (12 pending)"},
            "/api/diag/logs": lambda: [l for l in LOGS if l["seq"] > int(q.get("since", ["0"])[0])],
            "/api/status": lambda: {"device_name": "Vynull", "device_number": 17, "track_count": len(TRACKS), "peers": PEERS, "players": players()},
        }
        if p in routes:
            return self.send(200, routes[p]())
        if p.startswith("/api/playlists/") and p.endswith("/tracks"):
            pid = int(p.split("/")[3])
            pl = next((x for x in PLAYLISTS if x["id"] == pid), None)
            if not pl:
                return self.send(404, b"not found", "text/plain")
            tracks = smart_tracks(pl["rules"]) if pl.get("is_smart") and pl.get("rules") else [BY_ID[i] for i in pl["track_ids"] if i in BY_ID]
            return self.send(200, tracks)
        if p.startswith("/api/analysis/ext/"):
            # What a deck reads from its own USB: 404 for the first two asks, as the real daemon does until it has read it.
            EXT_ASKS[p] = EXT_ASKS.get(p, 0) + 1
            if EXT_ASKS[p] < 3:
                return self.send(404, b"not fetched yet", "text/plain")
            tid = int(p.rsplit("/", 1)[1])
            return self.send(200, {"track_id": tid, "duration_ms": 120000, "bpm": 126, "sample_rate": 150,
                                   "waveform": [{"r": (i // 40) % 8, "g": (i // 60 + 3) % 8, "b": (i // 80) % 8, "h": 4 + (i * 3) % 27} for i in range(150 * 120)],
                                   "cues": [{"number": 1, "type": "cue", "time_ms": 30000, "loop_ms": -1, "color_id": 0, "label": ""}], "beats": []})
        if p.startswith("/api/analysis/waveform/") and q.get("type", [""])[0] == "detail" and q.get("style", ["spectral"])[0] == "spectral":
            # PWV5-like: 150 entries a second, band weights 0-7 and a height 0-31.
            tid = int(p.rsplit("/", 1)[1])
            entries = [{"r": (i // 50 + tid) % 8, "g": (i // 70 + 2) % 8, "b": (i // 90) % 8, "h": 6 + (i * 5 + tid) % 25} for i in range(150 * 120)]
            return self.send(200, {"track_id": tid, "type": "detail", "style": "spectral", "sample_rate": 150, "entries": entries})
        if p.startswith("/api/analysis/waveform/") and q.get("type", [""])[0] == "color_preview":
            tid = int(p.rsplit("/", 1)[1])
            entries = [{"bass": int(120 + 100 * ((i * 7 + tid) % 10) / 10), "mid": 80, "treble": 50} for i in range(1500)]
            return self.send(200, {"track_id": tid, "type": "color_preview", "entries": entries})
        if p.startswith("/api/artwork/") and p.count("/") == 3:
            # Cover art is looked up by TRACK id; only tracks with art_id > 0 have some.
            tid = int(p.rsplit("/", 1)[1])
            t = BY_ID.get(tid)
            if not t or not t.get("art_id"):
                return self.send(404, b"no artwork", "text/plain")
            png = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")
            self.send_response(200)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(png)))
            self.end_headers()
            self.wfile.write(png)
            return
        if p.startswith("/api/tracks/") and p.endswith("/phrases"):
            # Five phrases; the second and fourth are sung (scores either side of the thresholds).
            return self.send(200, [
                {"kind": 1, "name": "Intro", "start_ms": 0, "end_ms": 20000, "energy": 0.2, "has_vocal": False, "vocal_score": 0.02},
                {"kind": 2, "name": "Up", "start_ms": 20000, "end_ms": 60000, "energy": 0.6, "has_vocal": True, "vocal_score": 0.2},
                {"kind": 3, "name": "Down", "start_ms": 60000, "end_ms": 90000, "energy": 0.4, "has_vocal": False, "vocal_score": 0.1},
                {"kind": 5, "name": "Chorus", "start_ms": 90000, "end_ms": 130000, "energy": 0.9, "has_vocal": True, "vocal_score": 0.13},
                {"kind": 6, "name": "Outro", "start_ms": 130000, "end_ms": 150000, "energy": 0.1, "has_vocal": False, "vocal_score": 0.0}])
        if p.startswith("/api/tracks/") and p.endswith("/cues"):
            return self.send(200, [{"number": 1, "type": "cue", "time_ms": 8200, "loop_ms": -1, "color_id": 0, "label": ""}])
        self.send(404, b"fake daemon: no such route", "text/plain")


class Server(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


Server(("127.0.0.1", PORT), H).serve_forever()
