// Tags, Settings, Diag and Overlay against the fake daemon. Writes are checked
// in the daemon's own log and state, not only on screen.
import { launch, start, sleep, until, connect, openTab, btn as btn0, newPage } from './common.mjs';

const APP = process.env.APP, FAKE = process.env.FAKE;
const T = start();
const { ok } = T;
const browser = await launch();
const page = await newPage(browser, [APP, FAKE]);
const btn = (sel, text, exact) => btn0(page, T, sel, text, exact);
const get = async (p) => (await fetch(FAKE + p)).json();
const log = async () => get('/__log');
const writes = async (method, path) => (await log()).filter((l) => l[0] === method && l[1] === path);
const text = () => page.evaluate(() => document.body.innerText);
const hash = () => page.evaluate(() => location.hash);
const type = async (sel, v) => { await page.$eval(sel, (e) => { e.focus(); e.select(); }); await page.keyboard.press('Backspace'); if (v) await page.type(sel, v); };

await connect(page, APP, FAKE);

// ======================================================================= TAGS
await openTab(page, 'TAGS');
await page.waitForSelector('.pgt .card');
await sleep(500);
ok('tags: the URL names the screen', (await hash()) === '#/tags');
ok('tags: categories and counts show', await page.evaluate(() => /TAGS AND TRACK COLORS/i.test(document.body.innerText) && /12 tags in 3 categories/.test(document.body.innerText)));
ok('tags: each category lists its tags with a track count', await page.evaluate(() => document.querySelectorAll('.pgt .cats .card').length === 3 && [...document.querySelectorAll('.pgt .tr')].every((r) => /\d+/.test(r.querySelector('.c').textContent))));
ok('tags: the color dot sits in line with the name', await page.evaluate(() => { const r = document.querySelector('.pgt .tr'); const d = r.querySelector('i').getBoundingClientRect(), n = r.querySelector('.n').getBoundingClientRect(); return Math.abs((d.top + d.height / 2) - (n.top + n.height / 2)) < 6; }));
ok('tags: a tag with a palette color uses it', await page.evaluate(() => { const r = [...document.querySelectorAll('.pgt .tr')].find((x) => x.textContent.includes('Drum & Bass')); return getComputedStyle(r.querySelector('i')).backgroundColor === 'rgb(95, 211, 141)'; }));
ok('tags: track colors are counted', await page.evaluate(() => [...document.querySelectorAll('.pgt .tcc')].some((e) => /PINK/.test(e.textContent)) && [...document.querySelectorAll('.pgt .tcc b')].some((b) => +b.textContent > 0)));
ok('tags: before picking one the editor says so', await page.evaluate(() => /Pick a tag to rename it/.test(document.body.innerText)));

await page.evaluate(() => [...document.querySelectorAll('.pgt .tr')].find((x) => x.textContent.includes('Techno')).click());
await sleep(300);
ok('tags: picking a tag opens the editor', await page.evaluate(() => document.querySelector('.pgt .ed input.in').value === 'Techno'));
ok('tags: the editor counts the tracks using it', await page.evaluate(() => /\d+ TRACKS USE THIS TAG/.test(document.body.innerText)));
ok('tags: SAVE waits for a change', await page.evaluate(() => [...document.querySelectorAll('.pgt .ed .btn')].find((b) => b.textContent === 'SAVE').disabled));
await type('.pgt .ed input.in', 'Techno Plus');
await page.evaluate(() => document.querySelector('.pgt .ed .sw button[title=ORANGE]').click());
await btn('.pgt .ed .btn', 'SAVE', true);
ok('tags: SAVE sends only what changed', !!(await until(async () => { const w = await writes('PUT', '/api/tags/2'); return w.length === 1 && w[0][2].name === 'Techno Plus' && w[0][2].color === 3 && !('category_id' in w[0][2]); })), JSON.stringify(await writes('PUT', '/api/tags/2')));
ok('tags: the list shows the new name', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.pgt .tr .n')].some((e) => e.textContent === 'Techno Plus')))));
await openTab(page, 'LIBRARY');
await page.waitForSelector('.lib .rw');
ok('tags: the library filter chips follow the rename', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.tgs .tag')].some((e) => e.textContent.includes('Techno Plus'))))));
await openTab(page, 'TAGS');
await page.waitForSelector('.pgt .card');

await until(() => page.evaluate(() => document.querySelectorAll('.pgt .tr').length > 5));
await btn('.pgt .hd .btn', '+ TAG');
await page.type('.pgt .card input.in', 'brand new');
await page.keyboard.press('Enter');
ok('tags: a new tag is created and selected', !!(await until(async () => (await writes('POST', '/api/tags')).some((w) => w[2].name === 'brand new'))) && !!(await until(() => page.evaluate(() => document.querySelector('.pgt .ed input.in')?.value === 'brand new'))));
ok('tags: it lands in a category', (await writes('POST', '/api/tags')).at(-1)[2].category_id > 0, JSON.stringify((await writes('POST', '/api/tags')).at(-1)[2]));
await btn('.pgt .hd .btn', '+ CATEGORY');
await page.type('.pgt .card input.in', 'My category');
await btn('.pgt .card .btn', 'CREATE');
ok('tags: a category is created', !!(await until(async () => (await writes('POST', '/api/tag-categories')).some((w) => w[2].name === 'My category'))));
ok('tags: and shown as a card', !!(await until(() => page.evaluate(() => document.querySelectorAll('.pgt .cats .card').length === 4))));
await page.evaluate(() => { const card = [...document.querySelectorAll('.pgt .cats .card')].find((c) => c.textContent.includes('My category')); [...card.querySelectorAll('.btn')].find((b) => b.textContent === 'RENAME').click(); });
await sleep(200);
await type('.pgt .cats input.in', 'Renamed');
await page.keyboard.press('Enter');
ok('tags: a category can be renamed', !!(await until(async () => (await writes('PUT', '/api/tag-categories/101')).some((w) => w[2].name === 'Renamed'))));

// deletes ask twice
await page.screenshot({ path: 'ui-tags.png' });
await page.evaluate(() => [...document.querySelectorAll('.pgt .tr')].find((x) => x.textContent.includes('brand new')).click());
await sleep(200);
await btn('.pgt .ed .btn', 'DELETE TAG');
await sleep(300);
ok('tags: the first click on DELETE asks to confirm and deletes nothing', (await writes('DELETE', '/api/tags/101')).length === 0 && /CONFIRM DELETE/.test(await text()));
await btn('.pgt .ed .btn', 'KEEP IT');
ok('tags: KEEP IT backs out', !/CONFIRM DELETE/.test(await text()));
await btn('.pgt .ed .btn', 'DELETE TAG');
await btn('.pgt .ed .btn', 'CONFIRM DELETE');
ok('tags: the second click deletes it', !!(await until(async () => (await writes('DELETE', '/api/tags/101')).length === 1)));
ok('tags: and it leaves the list', !!(await until(() => page.evaluate(() => ![...document.querySelectorAll('.pgt .tr .n')].some((e) => e.textContent === 'brand new')))));
await page.evaluate(() => { const card = [...document.querySelectorAll('.pgt .cats .card')].find((c) => c.textContent.includes('Renamed')); [...card.querySelectorAll('.btn')].find((b) => b.textContent === 'DELETE').click(); });
await sleep(300);
ok('tags: a category also asks first', (await writes('DELETE', '/api/tag-categories/101')).length === 0);
await page.evaluate(() => { const card = [...document.querySelectorAll('.pgt .cats .card')].find((c) => c.textContent.includes('Renamed')); [...card.querySelectorAll('.btn')].find((b) => b.textContent === 'CONFIRM DELETE').click(); });
ok('tags: and deletes on the second click', !!(await until(async () => (await writes('DELETE', '/api/tag-categories/101')).length === 1)));

// ================================================================== SETTINGS
await openTab(page, 'SETTINGS');
await page.waitForSelector('.pgs .card');
await sleep(500);
ok('settings: the URL names the screen', (await hash()) === '#/settings');
ok('settings: the live-edit banner shows', await page.evaluate(() => /LAST WRITE WINS/.test(document.querySelector('.pgs .note').textContent)));
ok('settings: seven sections', (await page.evaluate(() => [...document.querySelectorAll('.pgs .ni')].map((b) => b.textContent))).join() === 'WEB UI,LIBRARY SOURCE,STREAM OVERLAY,PLAYER MENU,MYSETTING,DJMMYSETTING,DEVSETTING');
const section = (n) => page.evaluate((x) => [...document.querySelectorAll('.pgs .ni')].find((b) => b.textContent === x).click(), n);

// WEB UI is the section that opens first
ok('settings: WEB UI opens first', await page.evaluate(() => document.querySelector('.pgs .ni.on')?.textContent === 'WEB UI' && !!document.querySelector('.pgs .wpv')));
await section('MYSETTING');
await page.waitForSelector('.pgs .sr .seg');
// MYSETTING: editable, each click rewrites the whole document
ok('settings: player settings open first, grouped', await page.evaluate(() => ['DISPLAY', 'PLAYBACK'].every((g) => [...document.querySelectorAll('.pgs .grp')].some((e) => e.textContent === g))));
ok('settings: options come from the daemon', await page.evaluate(() => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => /Tempo range/.test(x.textContent)); return [...r.querySelectorAll('.seg button')].map((b) => b.textContent).join() === '6,10,16,WIDE'; }));
ok('settings: the current value is marked', await page.evaluate(() => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => /Tempo range/.test(x.textContent)); return r.querySelector('.seg button.on').textContent === '10'; }));
await page.evaluate(() => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => /Tempo range/.test(x.textContent)); [...r.querySelectorAll('.seg button')].find((b) => b.textContent === '16').click(); });
ok('settings: a click posts the whole config with one change', !!(await until(async () => { const w = await writes('POST', '/api/settings'); const f = w.at(-1)?.[2].full; return f && f.my_setting.tempo_range === 'sixteen' && f.my_setting.quantize === 'on' && f.dev_setting.waveform_color === 'blue' && f.bpm_range_min === 100; })));
ok('settings: the screen shows the new value', !!(await until(() => page.evaluate(() => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => /Tempo range/.test(x.textContent)); return r.querySelector('.seg button.on').textContent === '16'; }))));
// a change made elsewhere is not undone by the next click
await fetch(FAKE + '/api/settings', { method: 'POST', body: JSON.stringify({ full: { ...(await get('/api/settings')).full, my_setting: { ...(await get('/api/settings')).full.my_setting, quantize: 'off' } } }) });
await page.evaluate(() => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => /Sync/.test(x.textContent)); [...r.querySelectorAll('.seg button')].find((b) => b.textContent === 'ON').click(); });
ok('settings: an edit starts from the daemon\'s current config, not a stale copy', !!(await until(async () => { const f = (await get('/api/settings')).full; return f.my_setting.sync === 'on' && f.my_setting.quantize === 'off'; })));

// DJMMYSETTING and DEVSETTING: editable the same way
const rowBtn = (label, text) => page.evaluate((l, t) => { const r = [...document.querySelectorAll('.pgs .sr')].find((x) => x.textContent.includes(l)); [...r.querySelectorAll('.seg button')].find((b) => b.textContent === t).click(); }, label, text);
await section('DJMMYSETTING');
ok('settings: the mixer section lists its values with choices', !!(await until(() => page.evaluate(() => /Cross fader curve/.test(document.body.innerText) && document.querySelectorAll('.pgs .sr .seg').length === 3 && !/READ ONLY/.test(document.body.innerText)))));
await rowBtn('Cross fader curve', 'SLOW CUT');
ok('settings: a mixer change posts the whole config', !!(await until(async () => { const f = (await writes('POST', '/api/settings')).at(-1)?.[2].full; return f && f.djm_my_setting.cross_fader_curve === 'slow_cut' && f.djm_my_setting.mic_low_cut === 'on' && f.my_setting.sync === 'on' && f.dev_setting.waveform_color === 'blue'; })));
await section('DEVSETTING');
ok('settings: the waveform section is editable', !!(await until(() => page.evaluate(() => /Waveform color/.test(document.body.innerText) && document.querySelectorAll('.pgs .sr .seg').length === 4 && !/READ ONLY/.test(document.body.innerText)))));
await rowBtn('Waveform color', 'RGB');
ok('settings: a waveform change posts the whole config', !!(await until(async () => { const f = (await writes('POST', '/api/settings')).at(-1)?.[2].full; return f && f.dev_setting.waveform_color === 'rgb' && f.dev_setting.key_display === 'classic' && f.djm_my_setting.cross_fader_curve === 'slow_cut'; })));

// PLAYER MENU
await section('PLAYER MENU');
await page.waitForSelector('.pgs .mi');
ok('settings: the menu lists its entries in order', (await page.evaluate(() => [...document.querySelectorAll('.pgs .mi .n')].map((e) => e.textContent))).join() === 'ARTIST,ALBUM,TRACK,BPM,GENRE');
ok('settings: locked entries cannot be hidden', await page.evaluate(() => [...document.querySelectorAll('.pgs .mi')].find((m) => m.textContent.includes('TRACK')).querySelector('.tg').disabled));
await page.evaluate(() => [...document.querySelectorAll('.pgs .mi')].find((m) => m.textContent.includes('ALBUM')).querySelector('button[aria-label="Move up"]').click());
ok('settings: moving an entry sends the new order', !!(await until(async () => { const w = await writes('PUT', '/api/menu-items'); return w.at(-1)?.[2].items?.map((i) => i.key).join() === 'album,artist,track,bpm,genre'; })));
await page.evaluate(() => [...document.querySelectorAll('.pgs .mi')].find((m) => m.textContent.includes('GENRE')).querySelector('.tg').click());
ok('settings: toggling shows an entry', !!(await until(async () => { const w = await writes('PUT', '/api/menu-items'); return w.at(-1)?.[2].items?.find((i) => i.key === 'genre')?.visible === true; })));
await page.select('.pgs select[aria-label="Track detail field"]', 'genre');
ok('settings: the track detail field is saved', !!(await until(async () => (await writes('PUT', '/api/menu-items')).some((w) => w[2].track_detail === 'genre'))));
await btn('.pgs .btn', 'RESET MENU');
await sleep(300);
ok('settings: RESET MENU asks first', (await writes('POST', '/api/menu-items/reset')).length === 0);
await btn('.pgs .btn', 'CONFIRM RESET');
ok('settings: and then restores the default', !!(await until(async () => (await writes('POST', '/api/menu-items/reset')).length === 1)));
ok('settings: the list follows the default order', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.pgs .mi .n')].map((e) => e.textContent).join() === 'ARTIST,ALBUM,TRACK,BPM,GENRE'))));

// LIBRARY SOURCE
await section('LIBRARY SOURCE');
await page.waitForSelector('.pgs input[aria-label="Lowest BPM"]');
ok('settings: the analysis range shows', await page.evaluate(() => document.querySelector('.pgs input[aria-label="Lowest BPM"]').value === '100' && document.querySelector('.pgs input[aria-label="Highest BPM"]').value === '200'));
ok('settings: SAVE waits for a change', await page.evaluate(() => [...document.querySelectorAll('.pgs .btn')].find((b) => b.textContent === 'SAVE').disabled));
await type('.pgs input[aria-label="Lowest BPM"]', '90');
await page.keyboard.press('Tab');
await type('.pgs input[aria-label="Highest BPM"]', '95');
await page.keyboard.press('Tab');
ok('settings: the range keeps at least 10 BPM between the ends', await page.evaluate(() => +document.querySelector('.pgs input[aria-label="Highest BPM"]').value >= +document.querySelector('.pgs input[aria-label="Lowest BPM"]').value + 10));
await type('.pgs input[aria-label="Highest BPM"]', '180');
await page.keyboard.press('Tab');
await btn('.pgs .btn', 'SAVE', true);
ok('settings: saving writes the range into the config', !!(await until(async () => { const f = (await get('/api/settings')).full; return f.bpm_range_min === 90 && f.bpm_range_max === 180 && f.my_setting.sync === 'on'; })));
await page.type('.pgs input[aria-label="From"]', '/old/music');
await page.type('.pgs input[aria-label="To"]', '/music');
await btn('.pgs .btn', 'REPLACE', true);
ok('settings: replace sends both prefixes and reports the result', !!(await until(async () => (await writes('POST', '/api/library/remap-paths')).some((w) => w[2].from === '/old/music' && w[2].to === '/music'))) && !!(await until(() => page.evaluate(() => /3 TRACKS CHANGED, 1 STILL MISSING/.test(document.body.innerText)))));
await btn('.pgs .btn', 'RE-IMPORT', true);
await sleep(300);
ok('settings: re-import asks first', (await writes('POST', '/api/tracks/reimport')).length === 0 && /CONFIRM RE-IMPORT/.test(await text()));
await btn('.pgs .btn', 'CONFIRM RE-IMPORT');
ok('settings: and then reads the tags again', !!(await until(async () => (await writes('POST', '/api/tracks/reimport')).length === 1)));
ok('settings: the options the API lacks are explained', await page.evaluate(() => /--music-dir/.test(document.body.innerText)));

// STREAM OVERLAY link
await section('STREAM OVERLAY');
ok('settings: the overlay address is shown', await page.evaluate(() => document.querySelector('.pgs input[aria-label="Overlay address"]').value.endsWith('/overlay')));

// WEB UI
await section('WEB UI');
await page.waitForSelector('.pgs .wpv');
ok('settings: night is the default theme', await page.evaluate(() => !document.querySelector('.app').classList.contains('day')));
const bg = () => page.evaluate(() => getComputedStyle(document.querySelector('.app')).backgroundColor);
const night = await bg();
await btn('.pgs .seg button', 'DAY', true);
ok('settings: DAY switches the whole app to the light palette', !!(await until(async () => (await bg()) !== night)) && await page.evaluate(() => document.querySelector('.app').classList.contains('day')), `${night} -> ${await bg()}`);
ok('settings: and the choice is remembered', await page.evaluate(() => JSON.parse(localStorage.getItem('vynull.look')).day === true));
// vocal markers, waveform AUTO, cue default colors
const stored = (k) => page.evaluate((key) => JSON.parse(localStorage.getItem('vynull.' + key)), k);
ok('settings: vocal markers default to MED', await page.evaluate(() => [...document.querySelectorAll('.pgs .sr')].find((x) => /Vocal markers/.test(x.textContent)).querySelector('.seg button.on').textContent === 'MED'));
await page.evaluate(() => [...[...document.querySelectorAll('.pgs .sr')].find((x) => /Vocal markers/.test(x.textContent)).querySelectorAll('.seg button')].find((b) => b.textContent === 'HIGH').click());
ok('settings: and the level is remembered', !!(await until(async () => (await stored('vocal')) === 3)));
await page.evaluate(() => [...document.querySelectorAll('.pgs .cds')][0].click());
await page.evaluate(() => [...document.querySelectorAll('.pgs .cds')][0].click());
ok('settings: a click on a cue slot steps its default color', !!(await until(async () => (await stored('cuecolors'))?.[0] === 2)) && (await stored('cuecolors')).slice(1).every((c) => c === 0));
await btn('.pgs .seg button', 'AUTO', true);
ok('settings: AUTO takes the daemon waveform color (rgb here)', !!(await until(() => page.evaluate(() => getComputedStyle(document.querySelector('.app')).getPropertyValue('--wk').trim().toUpperCase() === '#FF4D4D'), 12000)), await page.evaluate(() => getComputedStyle(document.querySelector('.app')).getPropertyValue('--wk')));
await page.screenshot({ path: 'ui-day-settings.png' });
await btn('.pgs .seg button', 'COMPACT', true);
ok('settings: COMPACT turns on compact rows', await page.evaluate(() => document.querySelector('.app').classList.contains('compact')));
await btn('.pgs .bnd', 'KICK');
await page.evaluate(() => document.querySelector('.pgs .sw2 button[aria-label="#FF6B6B"]').click());
ok('settings: a band color is picked from the swatches', await page.evaluate(() => JSON.parse(localStorage.getItem('vynull.wave')).lo === '#FF6B6B'));
ok('settings: the preview and the app use it at once', await page.evaluate(() => document.querySelector('.pgs .wpv path').getAttribute('fill') === '#FF6B6B' && document.querySelector('.app').getAttribute('style').includes('--wk:#FF6B6B')));
await btn('.pgs .seg button', 'WARM', true);
ok('settings: a preset replaces all three colors', await page.evaluate(() => { const w = JSON.parse(localStorage.getItem('vynull.wave')); return w.lo === '#FF6B3C' && w.mi === '#FFC15A'; }));
await openTab(page, 'LIBRARY');
await page.waitForSelector('.lib .rw');
ok('settings: compact rows are 48 px in the library', await page.evaluate(() => Math.round(document.querySelector('.lib .rw').getBoundingClientRect().height) === 48));
ok('settings: the library list still scrolls in compact mode', await (async () => { await page.evaluate(() => { document.querySelector('.lib .rows').scrollTop = 6000; }); await sleep(300); return (await page.evaluate(() => document.querySelector('.lib .rw .ttl').textContent)) !== 'Sample Track 0001'; })());
await page.screenshot({ path: 'ui-day-library.png' });
await page.reload();
await page.waitForSelector('.top');
ok('settings: day and compact survive a reload', await page.evaluate(() => { const c = document.querySelector('.app').classList; return c.contains('day') && c.contains('compact'); }));
await openTab(page, 'SETTINGS');
await page.waitForSelector('.pgs .card');
await section('WEB UI');
await btn('.pgs .seg button', 'NIGHT', true);
await btn('.pgs .seg button', 'COMFORT', true);

// ======================================================================= DIAG
await openTab(page, 'DIAG');
await page.waitForSelector('.pgd .card');
await sleep(2500);
ok('diag: the URL names the screen', (await hash()) === '#/diag');
const dd = () => page.evaluate(() => [...document.querySelectorAll('.pgd .dl')].map((d) => d.innerText.replace(/\s+/g, ' ')));
const cards = await dd();
ok('diag: runtime counters', /LINK ON/.test(cards[0]) && /UPTIME 3h 12m/.test(cards[0]) && /GO go1\.25\.7/.test(cards[0]) && /HEAP 92 MB/.test(cards[0]) && /GC COUNT 103/.test(cards[0]), cards[0]);
ok('diag: library counters', /TRACKS 1,200/.test(cards[1]) && /CACHE ON DISK 1\.4 GB/.test(cards[1]), cards[1]);
ok('diag: analysis counters and a progress bar', /PENDING 12/.test(cards[2]) && /ANALYZED 1,188/.test(cards[2]) && await page.evaluate(() => parseFloat(document.querySelector('.pgd .pr i').style.width) > 98), cards[2]);
ok('diag: what the daemon is busy with shows', await page.evaluate(() => /ANALYSIS\s+Analyzing \(12 pending\)/.test(document.body.innerText)));
const lines = () => page.evaluate(() => [...document.querySelectorAll('.pgd .log .ln')].map((e) => e.innerText.replace(/\s+/g, ' ').trim()));
const all = await lines();
ok('diag: log lines come with a clock time and no raw date stamp', all.length === 10 && /^\d\d:\d\d:\d\d /.test(all[0]) && !/2026\/10\/11/.test(all[0]), all[0]);
ok('diag: errors are colored', await page.evaluate(() => document.querySelectorAll('.pgd .log .ln.er').length === 2 && document.querySelectorAll('.pgd .log .ln.wn').length === 0));
await type('.pgd input[aria-label="Filter log"]', 'TRACK');
ok('diag: the filter is not case sensitive', (await lines()).length === 5 && (await lines()).every((l) => /track/i.test(l)), String((await lines()).length));
await page.evaluate(() => [...document.querySelectorAll('.pgd .pill')].find((p) => p.textContent === 'NFS').click());
ok('diag: a source pill narrows it further', (await lines()).length === 2);
await page.evaluate(() => document.querySelector('.pgd .chk input').click());
ok('diag: errors only keeps the failed read', (await lines()).length === 1 && /not found/.test((await lines())[0]));
await type('.pgd input[aria-label="Filter log"]', '');
await page.evaluate(() => document.querySelector('.pgd .chk input').click());
await page.evaluate(() => [...document.querySelectorAll('.pgd .pill')].find((p) => p.textContent === 'ALL').click());
ok('diag: clearing the filters brings every line back', (await lines()).length === 10);
await page.screenshot({ path: 'ui-diag.png' });
await btn('.pgd .hd .btn', 'UNLINK', true);
await sleep(300);
ok('diag: UNLINK asks first and says what it does', (await writes('POST', '/api/unlink')).length === 0 && /closes every player session/.test(await text()));
await btn('.pgd .hd .btn', 'CANCEL');
ok('diag: CANCEL backs out', !/CONFIRM UNLINK/.test(await text()));
await btn('.pgd .hd .btn', 'UNLINK', true);
await btn('.pgd .hd .btn', 'CONFIRM UNLINK');
ok('diag: the second click unlinks', !!(await until(async () => (await writes('POST', '/api/unlink')).length === 1)) && !!(await until(() => page.evaluate(() => /UNLINKED/.test(document.body.innerText)))));

// ===================================================================== OVERLAY
await openTab(page, 'OVERLAY');
await page.waitForSelector('.pgo .stage');
await sleep(1500);
ok('overlay: the URL names the screen', (await hash()) === '#/overlay');
ok('overlay: the address is the daemon\'s overlay page', await page.evaluate((f) => document.querySelector('.pgo .url input').value === f + '/overlay', FAKE));
ok('overlay: the settings are laid out (rows and segments are flex, switches are round)', await page.evaluate(() => {
  const cs = (sel) => getComputedStyle(document.querySelector(sel));
  return cs('.pgo .sr').display === 'flex' && ['flex', 'inline-flex'].includes(cs('.pgo .seg').display) && cs('.pgo .tg').borderRadius !== '0px' && cs('.pgo .seg button').height === '44px' && cs('.pgo .sw button').width === '44px';
}));
ok('overlay: the stage keeps its 16:9 shape', await page.evaluate(() => { const r = document.querySelector('.pgo .stage').getBoundingClientRect(); return Math.abs(r.width / r.height - 16 / 9) < 0.02; }));
const frameInfo = () => page.evaluate(() => { const f = document.querySelector('.pgo .stage iframe'); if (!f) return null; const r = f.getBoundingClientRect(), stEl = document.querySelector('.pgo .stage'), st = stEl.getBoundingClientRect(); return { doc: f.getAttribute('srcdoc') || '', sandbox: f.getAttribute('sandbox'), w: f.offsetWidth, h: f.offsetHeight, shown: Math.round(r.width), stage: stEl.clientWidth, events: getComputedStyle(f).pointerEvents, left: Math.round(r.left - st.left - stEl.clientLeft), top: Math.round(r.top - st.top - stEl.clientTop) }; });
ok('overlay: the preview is the daemon\'s own /overlay page, handed to the frame as text (a desktop webview would open an address in the browser)', !!(await frameInfo())?.doc.includes('fake overlay page'), (await frameInfo())?.doc.slice(0, 80));
ok('overlay: its requests are pointed at the daemon with a base tag', (await frameInfo())?.doc.includes('<base href="' + FAKE + '/">'));
ok('overlay: the frame is sandboxed to scripts only (no access to this app\'s storage)', (await frameInfo())?.sandbox === 'allow-scripts');
ok('overlay: the page runs and reaches the daemon through that base', !!(await until(async () => (await get('/__gets')).some((x) => x.includes('from=overlay-page')), 8000)));
ok('overlay: the frame is laid out at 1920 x 1080', (await frameInfo())?.w === 1920 && (await frameInfo())?.h === 1080);
ok('overlay: and scaled to fill the stage exactly', !!(await until(async () => { const f = await frameInfo(); return f && Math.abs(f.shown - f.stage) <= 1 && f.left === 0 && f.top === 0; }, 8000)), JSON.stringify(await frameInfo()));
ok('overlay: the frame ignores the pointer (it is a picture, not a page to use)', (await frameInfo())?.events === 'none');
const stageBefore = (await frameInfo()).stage;
await page.setViewport({ width: 1000, height: 900 });
ok('overlay: it follows the window when it is resized', !!(await until(async () => { const f = await frameInfo(); return f && f.stage < stageBefore - 20 && Math.abs(f.shown - f.stage) <= 1; }, 8000)), JSON.stringify([stageBefore, await frameInfo()]));
await page.setViewport({ width: 1440, height: 1100 });
ok('overlay: our own drawing of the card is gone (nothing made up)', await page.evaluate(() => !document.querySelector('.pgo .np') && !document.querySelector('.pgo .vinyl') && !document.querySelector('.pgo .band')));
const cfg = () => get('/api/overlay/config');
const click = (sel, text) => page.evaluate((s, t) => [...document.querySelectorAll(s)].find((b) => b.textContent.trim() === t || b.getAttribute('aria-label') === t).click(), sel, text);
// What each control sends. The picture itself is the daemon's page, so only the config is checked here.
await click('.pgo .pos button', 'TOP RIGHT');
ok('overlay: a position is sent to the daemon', !!(await until(async () => (await cfg()).position === 'top-right')));
await click('.pgo .seg button', 'COVER');
ok('overlay: the style is sent', !!(await until(async () => (await cfg()).style === 'cover')));
await click('.pgo .sw button', 'Teal');
ok('overlay: the accent color is sent', !!(await until(async () => (await cfg()).accent === '#3FD6C0')));
await click('.pgo .stp button', 'Wider');
ok('overlay: the card gets wider by 20 px', !!(await until(async () => (await cfg()).width === 460)));
for (let i = 0; i < 40; i++) await click('.pgo .stp button', 'Wider');
ok('overlay: the width stops at the daemon\'s limit', !!(await until(async () => (await cfg()).width === 1000)));
await type('.pgo input[aria-label="Header text"]', 'On Air');
await page.keyboard.press('Tab');
ok('overlay: the header text is sent when the field is left', !!(await until(async () => (await cfg()).label === 'On Air')));
await type('.pgo input[aria-label="Header text"]', '');
await page.keyboard.press('Tab');
ok('overlay: an empty header is sent as empty', !!(await until(async () => (await cfg()).label === '')));
await click('.pgo .tg', 'Deck by deck');
ok('overlay: the decks section is sent', !!(await until(async () => (await cfg()).show_decks === true)));
await click('.pgo .tg', 'Recently played');
ok('overlay: recently played is sent', !!(await until(async () => (await cfg()).show_history === true)));
await click('.pgo .stp button', 'Fewer');
ok('overlay: tracks listed is sent', !!(await until(async () => (await cfg()).history_count < 5)));
await click('.pgo .seg button', 'HALF');
ok('overlay: the waveform shape is sent', !!(await until(async () => (await cfg()).waveform_style === 'half')));
await click('.pgo .tg', 'Full-width band');
ok('overlay: the full-width band is sent', !!(await until(async () => (await cfg()).waveform_band === true)));
await click('.pgo .stp button', 'Taller');
ok('overlay: the band height is sent', !!(await until(async () => (await cfg()).waveform_height === 140)));
await click('.pgo .tg', 'Now-playing card');
ok('overlay: the card switch is sent', !!(await until(async () => (await cfg()).show_nowplaying === false)));
await page.screenshot({ path: 'ui-overlay.png' });
await click('.pgo .tg', 'Now-playing card');
await page.evaluate(() => { window.__copied = null; navigator.clipboard.writeText = (t) => { window.__copied = t; return Promise.resolve(); }; });
await click('.pgo .url .btn', 'COPY');
ok('overlay: COPY copies the address and says so', await page.evaluate((f) => window.__copied === f + '/overlay' && /COPIED/.test(document.querySelector('.pgo .url .btn').textContent), FAKE));
await page.reload();
await page.waitForSelector('.pgo .stage');
await sleep(800);
ok('overlay: the saved config comes back after a reload', await page.evaluate(() => document.querySelector('.pgo .pos button.on')?.textContent === 'TOP RIGHT' && document.querySelector('.pgo .seg button.on')?.textContent !== undefined));

ok('no request left the daemon and the app', page.external.length === 0, page.external.slice(0, 3).join(' '));
console.log('console errors:', page.errors.slice(0, 5));
ok('no console errors', page.errors.length === 0);
await browser.close();
process.exit(T.failures() ? 1 : 0);
