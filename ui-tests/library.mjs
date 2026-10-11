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
// ---- playlist order: EDIT ORDER, then drag a row
await side('WARM-UP');
await sleep(500);
const plRows = () => page.evaluate(() => [...document.querySelectorAll('.lib .rw .ttl')].slice(0, 5).map((e) => e.textContent.replace('MISSING', '').trim()));
const plBefore = await plRows();
const idOf = (title) => Number(title.match(/(\d{4})/)[1]);
const editBtn = () => page.evaluate(() => { const b = [...document.querySelectorAll('.lib .bar .btn')].find((x) => /EDIT ORDER|DONE/.test(x.textContent)); return b ? { text: b.textContent.trim(), disabled: b.disabled } : null; });
ok('a plain playlist has an EDIT ORDER button in the toolbar', (await editBtn())?.text === 'EDIT ORDER', JSON.stringify(await editBtn()));
ok('and no per-row move buttons', await page.evaluate(() => document.querySelector('.lib .rw [aria-label*="Move"]') === null));
await btn('.lib .bar .btn', 'EDIT ORDER', true);
ok('EDIT ORDER turns the rows draggable and the button into DONE', !!(await until(async () => (await editBtn())?.text === 'DONE')) && await page.evaluate(() => document.querySelectorAll('.lib .rw.ed').length > 3));
const rowBox = (n) => page.evaluate((i) => { const r = document.querySelectorAll('.lib .rw')[i].getBoundingClientRect(); return { x: r.x + 200, y: r.y + r.height / 2, h: r.height }; }, n);
const drag = async (from, to, steps = 8) => {
  const a = await rowBox(from), b = await rowBox(to);
  await page.mouse.move(a.x, a.y); await page.mouse.down();
  await page.mouse.move(a.x, (a.y + b.y) / 2, { steps }); await page.mouse.move(b.x, b.y, { steps });
  return { a, b };
};
await drag(0, 2);
ok('a drop line shows where the track will land', await page.evaluate(() => { const l = document.querySelector('.lib .dropline'); return !!l && l.getBoundingClientRect().height > 0; }));
ok('the dragged row is dimmed', await page.evaluate(() => document.querySelectorAll('.lib .rw.dg').length === 1));
await page.mouse.up();
const moved = await until(async () => { const r = await plRows(); return r[2] === plBefore[0] && r[0] === plBefore[1] && r[1] === plBefore[2] ? r : false; });
ok('dragging the first row onto the third moves it there', !!moved, JSON.stringify([plBefore.slice(0, 3), (await plRows()).slice(0, 3)]));
const sent = await until(async () => (await log()).filter((x) => x[1] === '/api/playlists/1/tracks').at(-1));
const expect = [plBefore[1], plBefore[2], plBefore[0]].map(idOf);
ok('and the whole new order is posted to the daemon', !!sent && sent[2].track_ids.length === 22 && expect.every((v, i) => sent[2].track_ids[i] === v), JSON.stringify(sent?.[2].track_ids.slice(0, 4)));
await sleep(500);
ok('the list still shows it after the daemon answered', (await plRows()).slice(0, 3).map(idOf).join() === expect.join());
await drag(2, 0);
await page.mouse.up();
ok('dragging it back up restores the first order', !!(await until(async () => (await plRows()).join() === plBefore.join())), JSON.stringify(await plRows()));
const postsBefore = (await log()).filter((x) => x[1] === '/api/playlists/1/tracks').length;
const aa = await rowBox(1);
await page.mouse.move(aa.x, aa.y); await page.mouse.down(); await page.mouse.up();
await sleep(300);
ok('a press without moving changes nothing and sends nothing', (await plRows()).join() === plBefore.join() && (await log()).filter((x) => x[1] === '/api/playlists/1/tracks').length === postsBefore);
ok('a click in edit mode does not open the drawer', await page.evaluate(() => !document.querySelector('.drw')));
await btn('.lib .bar .btn', 'DONE', true);
ok('DONE leaves edit mode', !!(await until(async () => (await editBtn())?.text === 'EDIT ORDER')) && await page.evaluate(() => document.querySelectorAll('.lib .rw.ed').length === 0));
await page.evaluate(() => document.querySelectorAll('.lib .rw')[0].click());
ok('rows open the drawer again', !!(await until(() => page.evaluate(() => !!document.querySelector('.drw')))));
await page.evaluate(() => document.querySelector('.drw .dh button[aria-label="Close track details"]').click());
await setQ('plain');
ok('EDIT ORDER is disabled while a search filters the list', (await editBtn())?.disabled === true);
await setQ('');
await page.evaluate(() => document.querySelector('.lh .c-b').dispatchEvent(new MouseEvent('click', { bubbles: true })));
await sleep(300);
ok('and while a column sort is on', (await editBtn())?.disabled === true);
await page.evaluate(() => { const h = document.querySelector('.lh .c-b'); h.dispatchEvent(new MouseEvent('click', { bubbles: true })); h.dispatchEvent(new MouseEvent('click', { bubbles: true })); });
await sleep(300);
await side('DNB');
await sleep(400);
ok('a smart playlist has no EDIT ORDER', (await editBtn()) === null);
await side('COLLECTION');
await sleep(300);
ok('nor does the collection', (await editBtn()) === null);
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
const qCount = () => page.evaluate(() => Number([...document.querySelectorAll('.side .src')].find((b) => b.textContent.includes('QUEUE')).querySelector('.n').textContent));
const qDrawer0 = await qCount();
await btn('.drw .dh .btn', '+ QUEUE', true);
ok('the drawer can add its track to the queue', !!(await until(async () => (await qCount()) === qDrawer0 + 1)), `${qDrawer0} -> ${await qCount()}`);
ok('and the button shows it is queued', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.drw .dh .btn')].some((b) => /IN QUEUE/.test(b.textContent) && b.getAttribute('aria-pressed') === 'true')))));
ok('the queue stores track 5', await page.evaluate(() => JSON.parse(localStorage.getItem('vynull.queue')).ids.includes(5)));
await btn('.drw .dh .btn', 'IN QUEUE');
ok('a second click takes it out again', !!(await until(async () => (await qCount()) === qDrawer0)));
ok('the drawer shows the full-track phrase strip', !!(await until(() => page.evaluate(() => document.querySelectorAll('.drw .phs.full .ph1').length === 5))));
ok('the drawer marks sung phrases (MED: two)', await page.evaluate(() => document.querySelectorAll('.drw .phs.full .mic').length === 2));
ok('the full strip covers the track from the first phrase', await page.evaluate(() => { const f = document.querySelector('.drw .phs.full .ph1'); return parseFloat(f.style.left) === 0 && parseFloat(f.style.width) > 0; }));
await btn('.drw .dh .btn', 'CLOSE');
await openTab(page, 'PLAYERS');
await openTab(page, 'LIBRARY');
await sleep(300);
await side('WARM-UP');

// ---- key colors against the master deck (the fake daemon's master plays 9A)
const keyColors = () => page.evaluate(() => Object.fromEntries([...document.querySelectorAll('.lib .rw .c-k .key')].map((k) => [k.textContent.trim(), getComputedStyle(k).backgroundColor])));
const kc = await until(async () => { const c = await keyColors(); return c['8A'] && c['11A'] ? c : false; });
ok('green: same key, neighbours and the relative key are drawn green, not the plain chip', !!kc && ['8A', '9B'].every((k) => !kc[k] || kc[k] === 'rgb(63, 214, 192)'), JSON.stringify(kc));
ok('green is really painted for at least one chip', !!kc && Object.values(kc).includes('rgb(63, 214, 192)'));
ok('yellow, orange and a plain key keep their colors', !!kc && kc['11A'] === 'rgb(232, 224, 90)' && kc['4A'] === 'rgb(255, 154, 60)');

// ---- a long genre stays inside its column
const longGenre = await page.evaluate(() => { const r = [...document.querySelectorAll('.lib .rw')].find((x) => /Progressive/.test(x.querySelector('.c-g').textContent)); if (!r) return null; const g = r.querySelector('.c-g'), b = g.getBoundingClientRect(), n = r.querySelector('.c-b').getBoundingClientRect(); const cs = getComputedStyle(g); return { gw: Math.round(b.width), overflow: cs.overflow, ellipsis: cs.textOverflow, clipped: g.scrollWidth > g.clientWidth, title: g.title.length > 40, gright: Math.round(b.right), nleft: Math.round(n.left) }; });
ok('a very long genre is clipped with an ellipsis inside its column', !!longGenre && longGenre.overflow === 'hidden' && longGenre.ellipsis === 'ellipsis' && longGenre.clipped && longGenre.gright <= longGenre.nleft, JSON.stringify(longGenre));
ok('and its full text is in the tooltip', !!longGenre && longGenre.title);

// ---- artwork and row waveforms
await side('COLLECTION');
await until(() => page.evaluate(() => document.querySelectorAll('.lib .rw .c-a img').length > 0));
await sleep(500);
const artRows = await page.evaluate(() => [...document.querySelectorAll('.lib .rw')].map((r) => ({ n: Number(r.querySelector('.ttl').textContent.match(/(\d{4})/)?.[1]), img: r.querySelector('.c-a img')?.getAttribute('src') || null, ok: r.querySelector('.c-a img')?.naturalWidth > 0 })));
ok('rows with cover art ask for the TRACK id, not the art id', artRows.some((r) => r.img) && artRows.filter((r) => r.img).every((r) => r.img.endsWith('/api/artwork/' + r.n) && !/artwork\/9\d{3}$/.test(r.img)), JSON.stringify(artRows.slice(0, 4)));
ok('and the cover is the one the daemon sent for that track', !!(await until(() => page.evaluate(() => [...document.querySelectorAll('.lib .rw .c-a img')].filter((i) => i.getBoundingClientRect().top < innerHeight - 40).every((i) => i.complete && i.naturalWidth > 0)))));
ok('rows the daemon says have no cover do not ask for one', artRows.filter((r) => r.n % 3 !== 0 && r.n > 0).every((r) => !r.img), JSON.stringify(artRows.filter((r) => r.n % 3 !== 0).slice(0, 3)));
const waveSrc = await page.evaluate(() => document.querySelector('.lib .rw .c-w img').getAttribute('src'));
ok('row waveforms use the daemon detail render in its color and overview type', /type=detail/.test(waveSrc) && /color=blue/.test(waveSrc) && /overview=half/.test(waveSrc) && !/color_preview/.test(waveSrc), waveSrc);

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

// ---- column layout and sorting
const geo = () => page.evaluate(() => {
  const h = {}, r = {};
  for (const e of document.querySelectorAll('.lh .hc')) { const b = e.getBoundingClientRect(); h[e.className.split(' ')[0]] = [Math.round(b.left), Math.round(b.width)]; }
  for (const e of document.querySelector('.rw').children) { const b = e.getBoundingClientRect(); r[e.className.split(' ')[0]] = [Math.round(b.left), Math.round(b.width)]; }
  return { h, r };
});
const g0 = await geo();
ok('header labels sit over their cells', Object.keys(g0.h).every((k) => g0.r[k] && g0.h[k][0] === g0.r[k][0] && g0.h[k][1] === g0.r[k][1]), JSON.stringify(g0.h));
const allRows = await page.evaluate(() => [...document.querySelectorAll('.lib .rw')].map((r) => [...r.children].map((e) => Math.round(e.getBoundingClientRect().left)).join(',')));
ok('every row has the same column positions, a MISSING badge included', new Set(allRows).size === 1, [...new Set(allRows)].join(' | '));
const rzBox = async (cls) => page.evaluate((c) => { const r = document.querySelector(`.lh .${c} .rz`).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; }, cls);
const rb = await rzBox('c-cm');
await page.mouse.move(rb.x, rb.y); await page.mouse.down(); await page.mouse.move(rb.x + 60, rb.y, { steps: 4 }); await page.mouse.up(); await sleep(300);
const g1 = await geo();
ok('widening a column keeps the others at their width', g1.h['c-cm'][1] > g0.h['c-cm'][1] + 40 && g1.h['c-b'][1] === g0.h['c-b'][1] && g1.h['c-w'][1] === g0.h['c-w'][1], JSON.stringify([g0.h['c-cm'], g1.h['c-cm']]));
ok('and the labels still match their cells', Object.keys(g1.h).every((k) => g1.r[k] && g1.h[k][0] === g1.r[k][0] && g1.h[k][1] === g1.r[k][1]));
ok('the resize handle is a thin bar, not a dot', await page.evaluate(() => { const b = document.querySelector('.lh .c-r .rz').getBoundingClientRect(); return b.width >= 10 && b.height >= 20; }));
await page.evaluate(() => document.querySelector('.lh .c-cm .rz').dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
await sleep(200);

const cells = (cls) => page.evaluate((c) => [...document.querySelectorAll(`.lib .rw .${c}`)].map((e) => e.textContent.trim()), cls);
const sorted = (a, cmp) => a.every((v, i) => i === 0 || cmp(a[i - 1], v) <= 0);
const bpmOf = (a) => a.map(Number);
const head = (cls) => page.evaluate((c) => document.querySelector(`.lh .${c}`).textContent.trim(), cls);
const clickHead = (cls, shift = false) => page.evaluate((c, sh) => {
  document.querySelector(`.lh .${c}`).dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: sh }));
}, cls, shift);
await clickHead('c-b'); await sleep(300);
ok('clicking BPM sorts ascending', sorted(bpmOf(await cells('c-b')), (a, b) => a - b) && /▲/.test(await head('c-b')), (await cells('c-b')).slice(0, 4).join(','));
await clickHead('c-b'); await sleep(300);
ok('clicking again sorts descending', sorted(bpmOf(await cells('c-b')), (a, b) => b - a) && /▼/.test(await head('c-b')), (await cells('c-b')).slice(0, 4).join(','));
await clickHead('c-b'); await sleep(300);
ok('a third click clears the sort', !/[▲▼]/.test(await head('c-b')));
await clickHead('c-k'); await sleep(200);
await clickHead('c-b', true); await sleep(300);
const pairs = await page.evaluate(() => [...document.querySelectorAll('.lib .rw')].map((r) => [r.querySelector('.c-k').textContent.trim(), Number(r.querySelector('.c-b').textContent)]));
const num = (k) => parseInt(k, 10) * 2 + (k.endsWith('B') ? 1 : 0);
ok('KEY first and BPM second: keys in order, BPM breaks ties', pairs.every((p, i) => i === 0 || num(pairs[i - 1][0]) < num(p[0]) || (num(pairs[i - 1][0]) === num(p[0]) && pairs[i - 1][1] <= p[1])), JSON.stringify(pairs.slice(0, 5)));
ok('the secondary key is marked 2', /2/.test(await head('c-b')) && /▲/.test(await head('c-b')));
await clickHead('c-b'); await sleep(300);
ok('clicking the secondary promotes it', sorted(bpmOf(await cells('c-b')), (a, b) => a - b));
await page.reload();
await page.waitForSelector('.lib .rw', { timeout: 30000 });
ok('the sort is remembered', /▲/.test(await head('c-b')));
await clickHead('c-b'); await clickHead('c-b'); await clickHead('c-k'); await clickHead('c-k'); await sleep(200);
ok('sorting is cleared again', !(await page.evaluate(() => /[▲▼]/.test(document.querySelector('.lh').textContent))));

await page.setViewport({ width: 1440, height: 400 });
await sleep(300);
const sc = await page.evaluate(() => { const s = document.querySelector('.lib .side'); s.scrollTop = 60; return { client: s.clientHeight, scroll: s.scrollHeight, top: s.scrollTop }; });
ok('the left column scrolls when it is taller than the window', sc.scroll > sc.client && sc.top > 0, JSON.stringify(sc));
await page.setViewport({ width: 1440, height: 1100 });

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
await until(() => page.evaluate(() => [...document.querySelectorAll('.fp .btn')].some((b) => /USE THIS FOLDER/.test(b.textContent)))); // a slow runner needs longer than a fixed wait
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
