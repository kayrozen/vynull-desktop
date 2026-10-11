// Track drawer, against the fixture daemon (ci/make-fixtures.sh): grid editing,
// playback, metadata, cues, color and tags, each checked against the daemon's own data.
import { launch } from './browser.mjs';

const APP = process.env.APP, API = process.env.API;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const ok = (name, cond, extra = '') => {
  if (!cond) failed++;
  console.log((cond ? 'PASS ' : 'FAIL ') + name + (extra ? '  ' + extra : ''));
};
const get = async (p) => (await fetch(API + p)).json();
const until = async (fn, ms = 6000) => {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = await fn();
    if (v) return v;
    await sleep(100);
  }
  return false;
};

const browser = await launch();
const page = await browser.newPage();
const errs = [];
page.on('console', (m) => { if (m.type() === 'error') errs.push(m.text().slice(0, 200)); });
page.on('pageerror', (e) => errs.push('pageerror: ' + String(e).slice(0, 300)));

// The fixture daemon analyzes a track when it is first asked for it: durations
// and BPMs in the library list stay 0 until then.
for (const t of await get('/api/tracks')) await get(`/api/analysis/${t.id}`);
ok('the daemon has analyzed the fixtures', !!(await until(async () => (await get('/api/tracks')).every((t) => t.duration > 0 && t.bpm > 0), 60000)));

// A tag to pick from (the fixture library has none). Created before the page loads.
const tag = await (await fetch(API + '/api/tags', { method: 'POST', body: JSON.stringify({ name: 'ci tag' }) })).json();

await page.goto(APP + '/');
await page.waitForSelector('.conn');
await page.$eval('.conn input', (e) => { e.value = ''; });
await page.type('.conn input', API.replace('http://', ''));
await page.click('.conn button[type=submit]');
await page.waitForSelector('.top');
await page.evaluate(() => [...document.querySelectorAll('.tab')].find((b) => b.textContent.includes('LIBRARY')).click());
await page.waitForSelector('.lib .rw', { timeout: 30000 });

const tracks = await get('/api/tracks');
const wav = tracks.find((t) => t.file_path.endsWith('fixture-120bpm.wav'));
const rowFor = (title) => page.evaluate((t) => {
  const r = [...document.querySelectorAll('.lib .rw')].find((x) => x.querySelector('.ttl').textContent.startsWith(t));
  if (!r) return false; r.click(); return true;
}, title);
const timecode = () => page.evaluate(() => document.querySelector('.tcode')?.textContent.trim().split(' ')[0]);
// Click the first match whose text, aria-label or title starts with `text`.
const btn = async (sel, text, exact = false) => {
  const found = await page.evaluate((s, t, ex) => {
    const b = [...document.querySelectorAll(s)].find((x) => [x.textContent, x.getAttribute('aria-label'), x.getAttribute('title')].some((v) => v && (ex ? v.trim() === t : v.trim().startsWith(t))));
    if (!b) return false; b.click(); return true;
  }, sel, text, exact);
  if (!found) { failed++; console.log(`FAIL no button "${text}" in ${sel}`); }
  return found;
};
const beats = async () => (await get(`/api/analysis/${wav.id}`)).beats;

ok('open the drawer by clicking a row', await rowFor('fixture-120bpm'));
await page.waitForSelector('.drw .zw .zin svg', { timeout: 15000 });
await sleep(500);
const head = await page.evaluate(() => ({
  title: document.querySelector('.drw .dh .ttl').textContent,
  kv: [...document.querySelectorAll('.drw .dh .kv')].map((k) => k.textContent.replace(/\s+/g, ' ')),
  bars: document.querySelectorAll('.drw .zin .bn').length,
  paths: document.querySelectorAll('.drw .zin svg path').length,
  ovw: document.querySelectorAll('.drw .ovw svg path').length,
}));
ok('header shows the track and its BPM', head.title.includes('fixture-120bpm') && head.kv[0].includes('120.00'), JSON.stringify(head.kv));
ok('zoom draws grid and three waveform bands', head.paths === 5, `${head.paths} paths`);
ok('zoom labels every fourth bar', head.bars >= 1, `${head.bars} labels`);
ok('overview is drawn', head.ovw >= 3);
ok('deck buttons are listed (none on the link)', await page.evaluate(() => /NO PLAYERS/.test(document.querySelector('.drw .ldg').textContent)));

// beat jumps follow the daemon's grid
const b0 = await beats();
await btn('.drw .bj .btn.sm', '+4', true); await sleep(200);
ok('beat jump +4 lands on the fifth beat', (await timecode()) === `00:0${Math.floor(b0[4].time_ms / 1000)}.${Math.floor((b0[4].time_ms % 1000) / 100)}`, `${await timecode()} vs ${b0[4].time_ms}`);
await btn('.drw .bj .btn.sm', '-1', true); await sleep(200);
ok('beat jump -1 goes back one beat', (await timecode()) === '00:01.5', await timecode());

// playback
const t0 = await timecode();
await page.click('button[aria-label="Play or pause"]');
const moved = await until(async () => (await timecode()) !== t0);
const status = await page.evaluate(() => document.querySelector('.drw .pinfo').textContent);
ok('play moves the playhead', !!moved, `${t0} -> ${await timecode()} | ${status}`);
ok('info shows DIRECT for a wav', /DIRECT/.test(status));
await page.click('button[aria-label="Play or pause"]');
const pausedAt = await timecode(); await sleep(400);
ok('pause stops it', (await timecode()) === pausedAt);
await page.click('button[aria-label=Metronome]');
ok('metronome toggles and is remembered', await page.evaluate(() => JSON.parse(localStorage.getItem('vynull.metronome'))) === true);
await page.click('button[aria-label=Metronome]');
await btn('.drw .btn.wavb', 'WAV'); await sleep(600);
ok('WAV option switches the source to TRANSCODED', /TRANSCODED/.test(await page.evaluate(() => document.querySelector('.drw .pinfo').textContent)));
await btn('.drw .btn.wavb', 'WAV');

// grid edits: each one is read back from the daemon
const first0 = (await beats())[0].time_ms;
await page.$eval('input[aria-label=BPM]', (e) => { e.focus(); e.select(); });
await page.keyboard.type('130');
await btn('.drw .btn', 'SAVE BPM');
ok('SAVE BPM sets the tempo', !!(await until(async () => (await get(`/api/tracks/${wav.id}/beats`)).bpm === 130)));
const after = await beats();
ok('the downbeat stays put when the BPM changes', Math.abs(after[0].time_ms - first0) < 1, `${first0} -> ${after[0].time_ms}`);
ok('the grid spacing follows the new BPM', Math.abs(after[1].time_ms - after[0].time_ms - 60000 / 130) < 1);
ok('the header shows the new tempo', !!(await until(() => page.evaluate(() => document.querySelector('.drw .dh .kv b').textContent === '130.00'))));

await btn('.drw .seg button', '+1');
ok('PHASE +1 renumbers the bars', !!(await until(async () => (await get(`/api/tracks/${wav.id}/beats`)).beat_phase_shift === 1)));
ok('the PHASE button shows the active value', await until(() => page.evaluate(() => document.querySelector('.drw .seg button.on')?.textContent === '+1')));
const shifted = await beats();
ok('first bar line moved to the second beat', shifted[0].beat_in_bar !== 1 && shifted[1].beat_in_bar === 1, shifted.slice(0, 2).map((b) => b.beat_in_bar).join(','));

await btn('.drw .btn', 'Clear the BPM');
ok('CLEAR restores 120 BPM', !!(await until(async () => (await get(`/api/tracks/${wav.id}/beats`)).bpm === 120)));
const cleared = await beats();
ok('CLEAR rebuilds the beat list too', Math.abs(cleared[1].time_ms - cleared[0].time_ms - 500) < 2, `${(cleared[1].time_ms - cleared[0].time_ms).toFixed(1)} ms between beats`);

const before = (await beats())[0].time_ms;
await btn('.drw .btn', 'Shift the grid 10 ms later');
ok('shift later moves the grid by 10 ms', !!(await until(async () => Math.abs((await beats())[0].time_ms - before - 10) < 0.5)));
ok('the shift counter shows it', await until(() => page.evaluate(() => /SHIFT \+10 MS/.test(document.body.innerText))));

await btn('.drw .bj .btn.sm', '+16', true); await sleep(200);
const at = await page.evaluate(() => document.querySelector('.tcode').textContent.trim().split(' ')[0]);
await btn('.drw .btn', 'Set the downbeat');
const down = await until(async () => (await beats()).find((x) => x.beat_in_bar === 1 && Math.abs(x.time_ms - toMs(at)) < 150));
ok('set downbeat at the playhead', !!down, `playhead ${at}`);
function toMs(tc) { const [m, rest] = tc.split(':'); const [s, d] = rest.split('.'); return (+m * 60 + +s) * 1000 + +d * 100; }

// metadata
await page.evaluate(() => { const l = [...document.querySelectorAll('.drw .fld label')].find((x) => x.textContent.startsWith('COMMENT')); const i = l.querySelector('input'); i.focus(); i.select(); });
await page.keyboard.type('edited from the drawer');
await btn('.drw .dh .btn', 'SAVE');
ok('SAVE writes the metadata', !!(await until(async () => (await get(`/api/tracks/${wav.id}`)).comment === 'edited from the drawer')));
ok('the list shows the edit at once', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.lib .rw .c-cm')].some((e) => e.textContent === 'edited from the drawer')))));
ok('a search finds the new comment', await (async () => {
  await page.$eval('.search input', (e) => { e.focus(); e.select(); });
  await page.keyboard.type('edited drawer'); await sleep(300);
  const n = await page.evaluate(() => document.querySelectorAll('.lib .rw').length);
  await page.$eval('.search input', (e) => { e.select(); }); await page.keyboard.press('Backspace'); await sleep(200);
  return n === 1;
})());

// cues
await btn('.drw .card .btn.sm', '+ CUE');
ok('+ CUE adds hot cue A at the playhead', !!(await until(async () => (await get(`/api/tracks/${wav.id}/cues`)).some((c) => c.number === 1))));
ok('the cue is listed and flagged in the zoom', await until(() => page.evaluate(() => document.querySelectorAll('.drw .cue').length === 1 && document.querySelectorAll('.drw .zin .fg').length === 1)));
await btn('.drw .card .btn.sm', '+ MEM');
ok('+ MEM numbers memory cues from 9', !!(await until(async () => (await get(`/api/tracks/${wav.id}/cues`)).some((c) => c.number === 9))));
await page.evaluate(() => document.querySelector('.drw .cue .btn').click());
ok('a cue can be deleted', !!(await until(async () => (await get(`/api/tracks/${wav.id}/cues`)).length === 1)));

// color
await page.evaluate(() => document.querySelector('.drw .sw button[title=RED]').click());
const colorOf = async () => (await get(`/api/tracks/${wav.id}`)).color_id || 0; // the daemon omits it when 0
ok('a color is set', !!(await until(async () => (await colorOf()) === 2)));
ok('the row is tinted at once', await until(() => page.evaluate(() => /--rb:color-mix/.test(document.querySelector('.lib .rw.on').getAttribute('style')))));
ok('recent colors remember it', await page.evaluate(() => JSON.parse(localStorage.getItem('vynull.color_history'))[0]) === 2);
await page.evaluate(() => document.querySelector('.drw .sw button[title=NONE]').click());
ok('the color can be cleared', !!(await until(async () => (await colorOf()) === 0)));
ok('the row tint goes back to plain', await until(() => page.evaluate(() => /--rb:var\(--bg\)/.test(document.querySelector('.lib .rw.on').getAttribute('style')))));

// tags
await btn('.drw .card button.tag', '+ TAG');
await btn('.drw .bj button.tag', 'ci tag');
ok('adding a tag from the picker', !!(await until(async () => (await get(`/api/tracks/${wav.id}/tags`)).length === 1)));
ok('the list row shows the tag', !!(await until(async () => (await page.evaluate(() => document.querySelectorAll('.lib .rw.on .sub i').length)) === 1)));
await btn('.drw .card button.tag', 'DONE');

// route: the open track is in the URL
ok('the open track is in the URL', (await page.evaluate(() => location.hash)) === `#/library/track/${wav.id}`);

// keyboard shortcuts
const playing = () => page.evaluate(() => document.querySelector('button[aria-label="Play or pause"]').classList.contains('act'));
await page.evaluate(() => document.activeElement && document.activeElement.blur());
await page.keyboard.press(' ');
ok('Space starts playback', !!(await until(playing)));
await page.keyboard.press(' ');
ok('Space pauses it', !!(await until(async () => !(await playing()))));
await page.$eval('input[aria-label=BPM]', (e) => e.focus());
await page.keyboard.press(' ');
await sleep(300);
ok('Space typed in a field does not play', !(await playing()));
await page.evaluate(() => document.activeElement && document.activeElement.blur());
const mem = (await get(`/api/tracks/${wav.id}/cues`));
for (const [n, ms] of [[2, 3000], [3, 6000]]) {
  await fetch(`${API}/api/tracks/${wav.id}/cues`, { method: 'POST', body: JSON.stringify({ number: n, type: 'cue', time_ms: ms, loop_ms: -1, color_id: 0, label: '' }) });
}
await btn('.drw .bj .btn.sm', '-16', true);
await page.reload();
await page.waitForSelector('.drw .zw .zin svg', { timeout: 20000 });
ok('a reload keeps the drawer open on the same track', (await timecode()) === '00:00.0');
await until(async () => (await page.evaluate(() => document.querySelectorAll('.drw .cue').length)) >= 3);
await page.keyboard.press(']');
ok('] jumps to the next cue', !!(await until(async () => (await timecode()) === '00:03.0')), await timecode());
await page.keyboard.press(']');
ok('] again goes on', !!(await until(async () => (await timecode()) === '00:06.0')), await timecode());
await page.keyboard.press('[');
ok('[ goes back', !!(await until(async () => (await timecode()) === '00:03.0')), await timecode());
await page.keyboard.press('m');
ok('M toggles the metronome', !!(await until(() => page.evaluate(() => JSON.parse(localStorage.getItem('vynull.metronome')) === true))));
await page.keyboard.press('m');

// dragging the zoom view scrubs
const zb = await page.evaluate(() => { const r = document.querySelector('.drw .zw').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width }; });
const before2 = toMs(await timecode());
await page.mouse.move(zb.x, zb.y);
await page.mouse.down();
await page.mouse.move(zb.x - zb.w / 4, zb.y, { steps: 6 });
await page.mouse.up();
await sleep(300);
const after2 = toMs(await timecode());
ok('dragging left moves later in the track', after2 > before2 + 2000, `${before2} -> ${after2} ms`);

// clicking the overview seeks
const ob = await page.evaluate(() => { const r = document.querySelector('.drw .ovw').getBoundingClientRect(); return { x: r.x, y: r.y + r.height / 2, w: r.width }; });
await page.mouse.click(ob.x + ob.w * 0.5, ob.y);
ok('clicking the overview seeks to that point', !!(await until(async () => Math.abs(toMs(await timecode()) - 12000) < 400)), await timecode());

// a loop cue plays as a loop
await fetch(`${API}/api/tracks/${tracks.find((t) => t.file_path.endsWith('fixture-128bpm.mp3')).id}/cues`, { method: 'POST', body: JSON.stringify({ number: 4, type: 'loop', time_ms: 1000, loop_ms: 2500, color_id: 0, label: 'loop' }) });

// reanalyze, switching tracks, closing
await btn('.drw .btn', 'Analyze the track again');
ok('re-analyze keeps the drawer working', await until(async () => (await get(`/api/tracks/${wav.id}/beats`)).bpm > 0));
ok('open another track', await rowFor('Fixture 128'));
ok('the drawer follows the new track', !!(await until(() => page.evaluate(() => document.querySelector('.drw .dh .ttl').textContent.includes('Fixture 128')))));
ok('the playhead restarts at zero', (await timecode()) === '00:00.0', await timecode());
ok('the loop cue is badged', !!(await until(() => page.evaluate(() => /LOOP/.test(document.querySelector('.drw .cue')?.textContent || '')))));
await page.evaluate(() => document.querySelector('.drw .cue').click());
const seen = [];
for (let i = 0; i < 40; i++) { await sleep(100); seen.push(toMs(await timecode())); }
ok('a loop cue keeps playback inside the loop', Math.max(...seen) <= 2700 && Math.min(...seen) >= 900 && seen.some((v) => v > 2300), `min ${Math.min(...seen)} max ${Math.max(...seen)} ms`);
ok('the info line names the loop', await page.evaluate(() => /LOOP 00:01\.0 - 00:02\.5/.test(document.querySelector('.drw .pinfo').textContent)));
await btn('.drw .btn.sm', 'EXIT LOOP');
await sleep(1800);
ok('exiting the loop lets playback continue past it', toMs(await timecode()) > 2700, await timecode());
await page.keyboard.press(' ');
await btn('.drw .dh .btn', 'CLOSE');
ok('CLOSE hides the drawer', await until(() => page.evaluate(() => !document.querySelector('.drw'))));

console.log('console errors:', errs.slice(0, 5));
await browser.close();
process.exit(failed ? 1 : 0);
