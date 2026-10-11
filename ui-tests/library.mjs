// Library screen against the fake daemon (1,200 synthetic tracks, four decks):
// list, search, tags, playlists, columns, routes, queue, history, new playlists,
// import and export panels, loading on decks.
import { launch, start, sleep, until, connect, openTab, btn as btn0, newPage } from './common.mjs';

const APP = process.env.APP, FAKE = process.env.FAKE;
const T = start();
const { ok } = T;
const browser = await launch();
const page = await newPage(browser, [APP, FAKE]);
const btn = (sel, text, exact) => btn0(page, T, sel, text, exact);
const log = async () => (await (await fetch(FAKE + '/__log')).json());
const rows = () => page.evaluate(() => document.querySelectorAll('.lib .rw').length);
const titles = () => page.evaluate(() => [...document.querySelectorAll('.lib .rw .ttl')].map((e) => e.textContent.replace('MISSING', '')));
const count = () => page.evaluate(() => document.querySelector('.tgs .mono')?.textContent.trim().replace(/\s+/g, ' '));
const hash = () => page.evaluate(() => location.hash);
const side = (label) => page.evaluate((l) => { const b = [...document.querySelectorAll('.side .src')].find((x) => x.textContent.includes(l)); if (!b) return false; b.click(); return true; }, label);
const setQ = async (v) => {
  await page.$eval('.search input', (e) => { e.focus(); e.select(); });
  await page.keyboard.press('Backspace');
  if (v) await page.type('.search input', v);
  await sleep(250);
};
const rowQueueBtn = (i = 0) => page.evaluate((n) => document.querySelectorAll('.lib .rw')[n].querySelector('.c-ld .ldb').click(), i);

await connect(page, APP, FAKE);
await openTab(page, 'LIBRARY');
await page.waitForSelector('.lib .rw', { timeout: 30000 });
await sleep(600);

// ---- list
ok('the whole library loads', (await count()).startsWith('1200 OF 1200'), await count());
const n0 = await rows();
ok('the list is virtual', n0 > 10 && n0 < 60, `${n0} rows in the DOM`);
ok('missing files are flagged', (await count()).includes('MISSING'));
ok('the sidebar lists playlists as a tree', await page.evaluate(() => { const t = [...document.querySelectorAll('.side .src')].map((b) => b.textContent); return t.some((x) => /WARM-UP/.test(x)) && t.some((x) => /FOLDER\s+ARCHIVE/.test(x)) && t.some((x) => /SMART\s+DNB/.test(x)) && t.some((x) => /SUMMER SETS/.test(x)); }));
ok('a playlist inside a folder is indented', await page.evaluate(() => { const pad = (l) => parseFloat([...document.querySelectorAll('.side .src')].find((b) => b.textContent.includes(l)).style.paddingLeft); return pad('SUMMER') > pad('WARM-UP'); }));
const first0 = (await titles())[0];
await page.evaluate(() => { document.querySelector('.lib .rows').scrollTop = 20000; });
await sleep(400);
ok('scrolling swaps the rows', first0 !== (await titles())[0]);
await page.evaluate(() => { document.querySelector('.lib .rows').scrollTop = 0; });
ok('row tint follows the track color', await page.evaluate(() => [...document.querySelectorAll('.lib .rw')].some((r) => /--rb:color-mix/.test(r.getAttribute('style'))) && [...document.querySelectorAll('.lib .rw')].some((r) => /--rb:var\(--bg\)/.test(r.getAttribute('style')))));

// ---- search
await setQ('theme');
ok('search ignores accents', (await titles()).some((t) => t.includes('Thème')), await count());
await setQ('sample thème 0009');
ok('words AND together', (await rows()) === 1, await count());
await setQ('female vocal');
ok('a hidden field explains the match', await page.evaluate(() => /MATCH IN COMMENT/.test(document.querySelector('.lib .rw .mt')?.textContent || '')));
await setQ('zzzzqqq');
ok('no match message and a way out', await page.evaluate(() => /NO TRACK MATCHES/.test(document.body.innerText) && !!document.querySelector('.none .btn')));
await setQ('');
await page.click('.search input');
await page.type('.search input', 'tech');
await sleep(300);
const sug = await page.evaluate(() => [...document.querySelectorAll('.sug .si')].map((e) => e.querySelector('.f').textContent));
ok('suggestions appear, tags first', sug.length > 0 && sug.length <= 8 && sug[0] === 'TAG', sug.join());
ok('the match is highlighted', await page.evaluate(() => !!document.querySelector('.sug .si b')?.textContent));
await page.keyboard.press('ArrowDown');
await page.keyboard.press('Enter');
await sleep(300);
ok('keyboard pick fills the query', (await page.$eval('.search input', (e) => e.value)).length > 0 && !(await page.evaluate(() => !!document.querySelector('.sug'))));
await setQ('');
await page.click('.search input');
await sleep(250);
ok('an empty query offers recent searches', await page.evaluate(() => { const f = [...document.querySelectorAll('.sug .si .f')]; return f.length >= 1 && f.every((x) => x.textContent === 'RECENT'); }));
await page.keyboard.press('Escape');

// ---- tags
const all = await count();
await page.evaluate(() => document.querySelectorAll('.tgs .tag')[1].click());
await sleep(300);
const one = await count();
ok('a tag narrows the list', one !== all, `${all} -> ${one}`);
await page.evaluate(() => document.querySelectorAll('.tgs .tag')[1].click());
await sleep(300);
ok('and clearing it restores the list', (await count()) === all);
await page.evaluate(() => document.querySelectorAll('.tgs .tag')[1].click());
await page.evaluate(() => document.querySelectorAll('.tgs .tag')[2].click());
await sleep(300);
const any = await count();
await page.evaluate(() => document.querySelector('.tgs .tag.md').click());
await sleep(300);
ok('ALL matches fewer tracks than ANY', parseInt((await count())) < parseInt(any), `${any} -> ${await count()}`);
await page.evaluate(() => { document.querySelector('.tgs .tag.md').click(); document.querySelectorAll('.tgs .tag')[1].click(); document.querySelectorAll('.tgs .tag')[2].click(); });
await sleep(300);

// ---- playlists and routes
await side('WARM-UP');
await sleep(500);
ok('a playlist shows its tracks', (await count()).startsWith('22 OF 1200'), await count());
ok('and puts its id in the URL', (await hash()) === '#/library/playlist/1', await hash());
await side('DNB');
await sleep(500);
ok('a smart playlist is evaluated by the daemon', /^2\d\d OF 1200/.test(await count()) || parseInt(await count()) > 22, await count());
await side('WARM-UP');
await sleep(400);
await page.evaluate(() => document.querySelectorAll('.lib .rw')[3].click());
await page.waitForSelector('.drw');
const trackHash = await hash();
ok('opening a track puts it in the URL', /^#\/library\/playlist\/1\/track\/\d+$/.test(trackHash), trackHash);
await page.reload();
await page.waitForSelector('.drw', { timeout: 30000 });
ok('a reload comes back to the same playlist and track', (await hash()) === trackHash && (await count()).startsWith('22 OF 1200'), `${await hash()} ${await count()}`);
await page.goBack();
ok('back closes the drawer', !!(await until(() => page.evaluate(() => !document.querySelector('.drw')))), await hash());
await page.goForward();
ok('forward opens it again', !!(await until(() => page.evaluate(() => !!document.querySelector('.drw')))));
await page.goto(APP + '/#/library/track/5');
ok('a link to a track opens its drawer', !!(await until(() => page.evaluate(() => /Sample Track 0005/.test(document.querySelector('.drw .dh .ttl')?.textContent || ''))), 20000));
await btn('.drw .dh .btn', 'CLOSE');
await openTab(page, 'PLAYERS');
await openTab(page, 'LIBRARY');
await sleep(300);
await side('WARM-UP');

// ---- columns
const hdr0 = await page.evaluate(() => [...document.querySelectorAll('.lh .hc')].map((e) => e.textContent.trim()));
await page.click('button[aria-label=Columns]');
await sleep(200);
ok('the column panel lists 11 columns', (await page.evaluate(() => document.querySelectorAll('.colp .cr').length)) === 11);
await page.evaluate(() => { const r = [...document.querySelectorAll('.colp .cr')].find((x) => x.querySelector('.n').textContent === 'GENRE'); r.querySelector('.tg').click(); });
await sleep(300);
const hdr1 = await page.evaluate(() => [...document.querySelectorAll('.lh .hc')].map((e) => e.textContent.trim()));
ok('hiding a column removes it', hdr0.includes('GENRE') && !hdr1.includes('GENRE'));
await page.evaluate(() => { const r = [...document.querySelectorAll('.colp .cr')].find((x) => x.querySelector('.n').textContent === 'RATING'); r.querySelectorAll('button')[0].click(); });
await sleep(300);
const ord = await page.evaluate(() => { const rw = document.querySelector('.rw'); return [getComputedStyle(rw.querySelector('.c-r')).order, getComputedStyle(rw.querySelector('.c-m')).order].map(Number); });
ok('reordering changes the CSS order', ord[0] < ord[1], ord.join('/'));
await page.click('button[aria-label=Columns]');
const w0 = await page.evaluate(() => document.querySelector('.lh .c-b').getBoundingClientRect().width);
const box = await page.evaluate(() => { const r = document.querySelector('.lh .c-b .rz').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; });
await page.mouse.move(box.x, box.y);
await page.mouse.down();
await page.mouse.move(box.x + 40, box.y, { steps: 4 });
await page.mouse.up();
await sleep(300);
ok('dragging a handle widens the column', (await page.evaluate(() => document.querySelector('.lh .c-b').getBoundingClientRect().width)) > w0 + 20);
await page.reload();
await page.waitForSelector('.lib .rw', { timeout: 30000 });
ok('columns are remembered', !(await page.evaluate(() => [...document.querySelectorAll('.lh .hc')].map((e) => e.textContent.trim()).includes('GENRE'))));

// ---- load on a deck
await page.evaluate(() => document.querySelector('.rw .c-ld .ldb:last-child').click());
await sleep(300);
const decks = await page.evaluate(() => [...document.querySelectorAll('.ldm .dkb')].map((e) => e.textContent + ':' + e.className));
ok('the deck chooser shows each deck and its state', decks.length === 4 && decks[0].includes('pl') && decks[3].includes('em'), decks.join(' '));
await page.evaluate(() => [...document.querySelectorAll('.ldm .dkb')].find((e) => e.textContent === '2').click());
ok('an idle deck loads at once', !!(await until(async () => (await log()).filter((l) => l[1] === '/api/load').length === 1)));
ok('a toast confirms', !!(await until(() => page.evaluate(() => /LOADED ON DECK 2/.test(document.querySelector('.toast')?.textContent || '')))));
await page.evaluate(() => document.querySelector('.rw .c-ld .ldb:last-child').click());
await sleep(300);
await page.evaluate(() => [...document.querySelectorAll('.ldm .dkb')].find((e) => e.textContent === '1').click());
await sleep(400);
ok('a playing deck asks first', (await log()).filter((l) => l[1] === '/api/load').length === 1 && (await page.evaluate(() => /IS PLAYING/.test(document.querySelector('.toast')?.textContent || ''))));
await btn('.toast .btn', 'LOAD', true);
ok('confirming loads', !!(await until(async () => (await log()).filter((l) => l[1] === '/api/load').length === 2)));

// ---- queue
await side('COLLECTION');
await sleep(300);
const q0 = await titles();
await rowQueueBtn(0);
await rowQueueBtn(1);
await rowQueueBtn(2);
ok('queue buttons add tracks (the count shows in the sidebar)', !!(await until(() => page.evaluate(() => /QUEUE\s*3/.test(document.querySelector('.side .src').textContent)))));
await side('QUEUE');
await sleep(300);
ok('the queue view lists them in order', JSON.stringify(await titles()) === JSON.stringify(q0.slice(0, 3)) && (await hash()) === '#/library/queue', (await titles()).join('|'));
ok('the queue bar shows count and total time', await page.evaluate(() => /3 TRACKS\s+\d:\d\d:\d\d/.test(document.querySelector('.qbar')?.textContent || '')));
ok('the actions column is wider', await page.evaluate(() => getComputedStyle(document.querySelector('.rows')).getPropertyValue('--w-load').trim() === '200px'));
await page.evaluate(() => document.querySelectorAll('.lib .rw')[0].querySelectorAll('.c-ld .ldb')[1].click());
await sleep(300);
ok('move down reorders the queue', (await titles())[1] === q0[0] && (await titles())[0] === q0[1], (await titles()).join('|'));
await page.evaluate(() => document.querySelectorAll('.lib .rw')[2].querySelectorAll('.c-ld .ldb')[2].click());
await sleep(300);
ok('remove takes a track out', (await rows()) === 2);
await rowQueueBtn(0).catch(() => {});
await sleep(100);
ok('the queue is remembered across a reload', await (async () => { await page.reload(); await page.waitForSelector('.lib .rw', { timeout: 30000 }); await sleep(500); return (await rows()) === 2 && (await hash()) === '#/library/queue'; })());
const queued = await titles();
await btn('.qbar .btn', 'Load the next track');
ok('next loads on the emptiest free deck (4)', !!(await until(async () => { const l = (await log()).filter((x) => x[1] === '/api/load'); return l.length === 3 && l[2][2].device_number === 4; })), JSON.stringify((await log()).filter((l) => l[1] === '/api/load').map((l) => l[2])));
ok('a loaded track is dimmed', !!(await until(() => page.evaluate(() => document.querySelectorAll('.lib .rw.pd').length === 1))));
await btn('.qbar .btn', 'Load the next track');
ok('the next unplayed track follows', !!(await until(async () => (await log()).filter((x) => x[1] === '/api/load').length === 4)));
await btn('.qbar .btn', 'Load the next track');
ok('an exhausted queue says so', !!(await until(() => page.evaluate(() => /NO TRACK LEFT/.test(document.querySelector('.toast')?.textContent || '')))));
await btn('.qbar .btn', 'Save the queue as a playlist');
ok('saving creates a playlist with the queue', !!(await until(async () => { const l = await log(); const mk = l.find((x) => x[1] === '/api/playlists'); const set = l.filter((x) => /\/tracks$/.test(x[1]) && x[0] === 'POST').pop(); return mk && mk[2].name === 'Queue 2 tracks' && set && set[2].track_ids.length === 2; })));
ok('and it appears in the sidebar', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.side .src')].some((b) => /QUEUE 2 TRACKS/.test(b.textContent))))));
await side('QUEUE');
await btn('.qbar .btn', 'Empty the queue');
ok('emptying the queue', !!(await until(() => page.evaluate(() => /THE QUEUE IS EMPTY/.test(document.body.innerText)))));

// ---- history
ok('the history source counts today only', await page.evaluate(() => /TODAY\s*2/.test([...document.querySelectorAll('.side .src')].find((b) => b.textContent.includes('TODAY')).textContent)));
await side('TODAY');
await sleep(500);
ok('and lists the newest play first', (await titles()).join('|') === 'Sample Track 0005|Sample Track 0012' && (await hash()) === '#/library/history', (await titles()).join('|'));

// ---- new playlist, folder, smart playlist
await btn('.sfoot .btn', 'New playlist');
await page.type('.tool input', 'Friday set');
await page.keyboard.press('Enter');
ok('a new playlist is created and opened', !!(await until(async () => { const m = (await log()).filter((x) => x[1] === '/api/playlists' && x[2].name === 'Friday set'); return m.length === 1; })) && !!(await until(() => hash().then((h) => /^#\/library\/playlist\/\d+$/.test(h)))));
await btn('.sfoot .btn', 'New folder');
await page.type('.tool input', 'Gigs');
await btn('.tool .btn', 'CREATE');
ok('a folder is created as a folder', !!(await until(async () => (await log()).some((x) => x[1] === '/api/playlists' && x[2].name === 'Gigs' && x[2].is_folder === true))));
await btn('.sfoot .btn', 'New smart playlist');
await page.type('.tool input[type=text]', 'Fast');
ok('a smart playlist needs a complete rule', await page.evaluate(() => [...document.querySelectorAll('.tool .btn')].find((b) => b.textContent === 'CREATE').disabled));
await page.type('.tool input[type=number]', '150');
await btn('.tool .btn', 'CREATE');
ok('a smart playlist sends its rules', !!(await until(async () => { const m = (await log()).find((x) => x[1] === '/api/playlists' && x[2].name === 'Fast'); return m && m[2].is_smart === true && m[2].rules.combinator === 'all' && m[2].rules.conditions[0].field === 'bpm' && m[2].rules.conditions[0].operator === 'gte' && m[2].rules.conditions[0].value === 150; })));

// ---- import and export
await btn('.bar .btn', 'Import and export');
ok('the export panel counts tracks', !!(await until(() => page.evaluate(() => /1200 TRACKS WILL EXPORT/.test(document.body.innerText)))));
await page.select('.tool select', 'playlist:1');
ok('and recounts for another source', !!(await until(() => page.evaluate(() => /4 TRACKS WILL EXPORT/.test(document.body.innerText)))));
await btn('.tool .btn', 'BROWSE');
ok('the folder picker lists the daemon folders', !!(await until(() => page.evaluate(() => /\/media/.test(document.querySelector('.fp')?.textContent || '')))));
await page.evaluate(() => [...document.querySelectorAll('.fpe')].find((e) => e.textContent.includes('/media')).click());
await sleep(400);
await btn('.fp .btn', 'USE THIS FOLDER');
ok('picking a folder fills the destination', await page.evaluate(() => document.querySelector('.tool input.mono').value === '/media'));
await btn('.tool .btn', 'WRITE USB');
ok('writing sends the options', !!(await until(async () => { const m = (await log()).find((x) => x[1] === '/api/export'); return m && m[2].destination === '/media' && m[2].source === 'playlist:1' && m[2].copy_files === true && m[2].merge === false; })));
await btn('.tool .btn', 'CHOOSE FILE');
await until(() => page.evaluate(() => [...document.querySelectorAll('.fpe')].some((e) => e.textContent.includes('/media'))));
await page.evaluate(() => [...document.querySelectorAll('.fpe')].find((e) => e.textContent.includes('/media')).click());
await sleep(400);
ok('the file picker greys out other file types', await page.evaluate(() => [...document.querySelectorAll('.fpe')].find((e) => e.textContent.includes('song.mp3'))?.disabled === true));
await page.evaluate(() => [...document.querySelectorAll('.fpe')].find((e) => e.textContent.includes('library.xml')).click());
await sleep(300);
await btn('.tool .btn', 'IMPORT', true);
ok('importing reports the result', !!(await until(() => page.evaluate(() => /DONE: 2 TRACKS ADDED, 1 UPDATED/.test(document.body.innerText)))));
ok('and sends the chosen file', (await log()).some((x) => x[1] === '/api/import/rekordbox' && x[2].path === '/media/library.xml'));

ok('no request left the daemon and the app', page.external.length === 0, page.external.slice(0, 3).join(' '));
console.log('console errors:', page.errors.slice(0, 5));
ok('no console errors', page.errors.length === 0);
await browser.close();
process.exit(T.failures() ? 1 : 0);
