// Players screen and the shell around it, against the fake daemon (four scripted decks).
import { launch, start, sleep, until, connect, openTab, newPage } from './common.mjs';

const APP = process.env.APP, FAKE = process.env.FAKE;
const { ok, failures } = start();
const browser = await launch();
const page = await newPage(browser, [APP, FAKE]);

await page.goto(APP + '/');
await page.waitForSelector('.conn input');
ok('first visit asks for the daemon', await page.evaluate(() => /HOST AND PORT/.test(document.body.innerText)));
await page.$eval('.conn input', (e) => { e.value = ''; });
await page.type('.conn input', FAKE.replace('http://', ''));
await page.click('.conn button[type=submit]');
await page.waitForSelector('.pcs .card', { timeout: 20000 });
ok('connecting saves the daemon', await page.evaluate(() => (localStorage.getItem('vynull.connection') || '').includes('127.0.0.1')));
ok('the URL names the screen', (await page.evaluate(() => location.hash)) === '#/players');
await sleep(2500);

const info = await page.evaluate(() => ({
  decks: document.querySelectorAll('.pcs article').length,
  names: [...document.querySelectorAll('.pcs .pn')].map((e) => e.textContent),
  states: [...document.querySelectorAll('.pcs .pch .bdg')].map((e) => e.textContent),
  paths: document.querySelectorAll('.ov svg path').length,
  phd: [...document.querySelectorAll('.phd')].map((e) => parseFloat(e.style.left)),
  keys: [...document.querySelectorAll('.key')].map((e) => e.className + ':' + e.textContent),
  mixer: [...document.querySelectorAll('.mx .m1')].map((e) => e.innerText.replace(/\s+/g, ' ')),
  history: [...document.querySelectorAll('.row .ttl')].map((e) => e.textContent),
  subtitle: document.querySelector('.hd .mut').textContent,
  chips: [...document.querySelectorAll('.top .chip')].map((e) => e.textContent.trim()),
}));
ok('four decks', info.decks === 4 && info.names.join() === 'P1,P2,P3,P4', info.names.join());
ok('deck states show', ['PLAYING', 'CUED', 'MASTER', 'ON AIR', 'EXT METADATA', 'EMPTY'].every((s) => info.states.includes(s)));
ok('an empty deck offers to load a track', await page.evaluate(() => /NO TRACK LOADED/.test(document.body.innerText)));
ok('overview waveforms draw in the CDJ RGB format (colored bars and cue marks)', info.paths >= 4, `${info.paths} paths`);
ok('the bars take their color from the band weights, not a fixed palette', await page.evaluate(() => new Set([...document.querySelectorAll('.ov svg path')].map((p) => p.getAttribute('fill')).filter((f) => /^rgb\(/.test(f))).size > 6));
ok('compatible keys are tinted, others are not', info.keys.join() === 'key ok:9A,key ok:8A,key:4A', info.keys.join());
ok('mixer shows on-air state only, no faders', info.mixer.some((m) => /CH 1 ON AIR/.test(m)) && info.mixer.some((m) => /CH 2 OFF/.test(m)) && !/%/.test(info.mixer.join()), info.mixer.join(' | '));
ok('subtitle counts players and mixers', /3 players and 1 mixer/.test(info.subtitle), info.subtitle);
ok('top bar shows link, stream and device', info.chips.some((c) => /LINK ON/.test(c)) && info.chips.includes('LIVE') && info.chips.some((c) => /Vynull #17/.test(c)), info.chips.join(' | '));
ok('recent plays are newest first', info.history.length === 3 && info.history[0] === 'Sample Track 0005', info.history.join(' | '));

ok('player cards show the phrase strip', !!(await until(() => page.evaluate(() => document.querySelectorAll('.pcs .phs.full .ph1').length >= 5))));
const mics = () => page.evaluate(() => document.querySelectorAll('.pcs .phs.full .mic').length);
const perDeck = await page.evaluate(() => document.querySelectorAll('.pcs .phs.full').length);
ok('MED marks the two phrases whose score is above 0.12', (await mics()) === 2 * perDeck, `${await mics()} for ${perDeck} strips`);
ok('phrases carry their name and times', await page.evaluate(() => /Chorus .*1:30 . 2:10/.test(document.querySelector('.pcs .phs.full .ph1:nth-child(4)').title)));
await page.evaluate(() => localStorage.setItem('vynull.vocal', '3'));
await page.reload(); await page.waitForSelector('.pcs .phs.full .ph1', { timeout: 20000 }); await sleep(500);
ok('HIGH also marks the 0.10 phrase', (await mics()) === 3 * (await page.evaluate(() => document.querySelectorAll('.pcs .phs.full').length)));
await page.evaluate(() => localStorage.setItem('vynull.vocal', '0'));
await page.reload(); await page.waitForSelector('.pcs .phs.full .ph1', { timeout: 20000 }); await sleep(500);
ok('OFF marks none', (await mics()) === 0);
await page.evaluate(() => localStorage.removeItem('vynull.vocal'));
await page.reload(); await page.waitForSelector('.pcs .phs.full .ph1', { timeout: 20000 }); await sleep(500);

ok('MASTER and SYNC are always shown, lit only when on', await page.evaluate(() => [...document.querySelectorAll('.pcs article')].every((a) => { const b = [...a.querySelectorAll('.pch .bdg')].map((x) => x.textContent); return b.includes('MASTER') && b.includes('SYNC') || a.textContent.includes('NO TRACK'); })));
ok('the master deck lights MASTER and the others do not', await page.evaluate(() => { const lit = (a, t) => !![...a.querySelectorAll('.pch .bdg.ms')].find((x) => x.textContent === t); const d = [...document.querySelectorAll('.pcs article')]; return lit(d[0], 'MASTER') && !lit(d[1], 'MASTER') && !lit(d[2], 'MASTER'); }));
ok('SYNC is lit on the decks in sync (1 and 3) only', await page.evaluate(() => { const lit = (a) => !![...a.querySelectorAll('.pch .bdg.sy')].length; const d = [...document.querySelectorAll('.pcs article')]; return lit(d[0]) && !lit(d[1]) && lit(d[2]); }));
ok('an external-source deck gets its waveform from the deck USB (after the daemon has read it)', !!(await until(() => page.evaluate(() => { const a = document.querySelectorAll('.pcs article')[2]; return a.querySelectorAll('.ov svg path[fill^="rgb("]').length > 5 && !/NO WAVEFORM|WAITING/.test(a.querySelector('.ov').textContent); }), 20000)));
ok('and its cue from the USB is marked', await page.evaluate(() => document.querySelectorAll('.pcs article')[2].querySelectorAll('.ov svg path').length > 1));
const a = info.phd;
await sleep(1500);
const b = await page.evaluate(() => [...document.querySelectorAll('.phd')].map((e) => parseFloat(e.style.left)));
// A deck that has reached the end of its track stays at 100% (a slow runner gets there before this check).
ok('playheads of playing decks move, the cued one does not', b[0] > a[0] && (b[2] > a[2] || a[2] >= 100) && b[1] === a[1], `${a.join(',')} -> ${b.join(',')}`);

// fonts: bundled, so nothing is fetched from outside and the faces are in use
const fonts = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family.replace(/"/g, '')); });
ok('bundled fonts load', fonts.includes('Roboto') && fonts.includes('Cousine'), [...new Set(fonts)].join());
ok('no request leaves the daemon and the app', page.external.length === 0, page.external.slice(0, 3).join(' '));
ok('the page uses Roboto and Cousine', await page.evaluate(() => getComputedStyle(document.querySelector('.app')).fontFamily.startsWith('Roboto') && getComputedStyle(document.querySelector('.pn')).fontFamily.startsWith('Cousine')));

// navigation: tabs are URLs
await openTab(page, 'DIAG');
ok('a tab changes the URL', (await page.evaluate(() => location.hash)) === '#/diag');
ok('the Diag screen opens', !!(await until(() => page.evaluate(() => /Runtime, library and analysis/.test(document.body.innerText)))));
await page.goBack();
ok('back returns to the previous screen', !!(await until(() => page.evaluate(() => location.hash === '#/players' && !!document.querySelector('.pcs')))));
await page.goto(APP + '/#/diag');
await page.waitForSelector('.top');
ok('opening a link lands on that screen', !!(await until(() => page.evaluate(() => /Runtime, library and analysis/.test(document.body.innerText)))));

// reconnect: the stream shows when the daemon is gone
ok('the connection chip changes the daemon', await page.evaluate(() => { const c = [...document.querySelectorAll('.top button.chip')][0]; c.click(); return true; }));
ok('and shows the connection screen', !!(await until(() => page.evaluate(() => !!document.querySelector('.conn')))));

// ---- 3band: the daemon's CDJ colors and its overview type
const FAKEURL = process.env.FAKE;
const setDev = async (patch) => { const cur = (await (await fetch(FAKEURL + '/api/settings')).json()).full; cur.dev_setting = { ...cur.dev_setting, ...patch }; await fetch(FAKEURL + '/api/settings', { method: 'POST', body: JSON.stringify({ full: cur }) }); };
await page.$eval('.conn input', (e) => { e.value = ''; });
await page.type('.conn input', FAKE.replace('http://', ''));
await page.click('.conn button[type=submit]');
await page.waitForSelector('.top');
await openTab(page, 'PLAYERS');
await page.waitForSelector('.pcs .ov svg');
await setDev({ waveform_color: '3band', overview_type: 'half' });
const band = () => page.evaluate(() => { const p = document.querySelector('.pcs .ov svg path.wl'); return p ? { fill: getComputedStyle(p).fill, blend: getComputedStyle(p).mixBlendMode, d: p.getAttribute('d'), mid: getComputedStyle(document.querySelector('.pcs .ov svg path.wm')).fill } : null; });
ok('3band: the daemon colors (blue, amber) replace the preset colors', !!(await until(async () => { const b = await band(); return b && b.fill === 'rgb(26, 102, 217)' && b.mid === 'rgb(217, 145, 26)'; }, 15000)), JSON.stringify(await band()));
ok('3band: the bands add together', (await band())?.blend === 'plus-lighter');
ok('3band + half overview: bars stand on the floor', /L\d+ 100Z$|L0 100Z$/.test((await band())?.d || ''), (await band())?.d?.slice(-30));
await setDev({ overview_type: 'full' });
ok('3band + full overview: bars are mirrored about the middle', !!(await until(async () => { const d = (await band())?.d || ''; return d && !/L0 100Z$/.test(d); }, 15000)), (await band())?.d?.slice(-30));

await setDev({ waveform_color: 'blue', overview_type: 'half' }); // the fake daemon is shared with the other scripts
console.log('console errors:', page.errors.slice(0, 5));
ok('no console errors', page.errors.length === 0);
await browser.close();
process.exit(failures() ? 1 : 0);
