// Helpers shared by the browser tests.
import { launch } from './browser.mjs';

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function start() {
  let failed = 0;
  const ok = (name, cond, extra = '') => {
    if (!cond) failed++;
    console.log((cond ? 'PASS ' : 'FAIL ') + name + (extra ? '  ' + extra : ''));
  };
  return { ok, failures: () => failed, fail: () => { failed++; } };
}

export const until = async (fn, ms = 6000) => {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = await fn();
    if (v) return v;
    await sleep(100);
  }
  return false;
};

/** Open the app, type the daemon's address on the connection screen, and wait for the shell. */
export async function connect(page, app, api) {
  await page.goto(app + '/');
  await page.waitForSelector('.conn input');
  await page.$eval('.conn input', (e) => { e.value = ''; });
  await page.type('.conn input', api.replace('http://', ''));
  await page.click('.conn button[type=submit]');
  await page.waitForSelector('.top');
}

/** A tab by its label. Settings is the gear beside the connection button, and Diag and Overlay are
 *  sections inside it, so those three are reached through the gear. */
export async function openTab(page, name) {
  const section = { SETTINGS: null, DIAG: 'DIAG', OVERLAY: 'STREAM OVERLAY' }[name];
  if (section === undefined) {
    return page.evaluate((n) => [...document.querySelectorAll('.tab')].find((b) => b.textContent.includes(n)).click(), name);
  }
  await page.evaluate(() => document.querySelector('.gear').click());
  if (section) {
    await page.waitForSelector('.pgs .ni');
    await page.evaluate((x) => [...document.querySelectorAll('.pgs .ni')].find((b) => b.textContent === x).click(), section);
  }
}

/** Click the first match whose text, aria-label or title starts with `text` (or equals it when `exact`). */
export async function btn(page, ctx, sel, text, exact = false) {
  const found = await page.evaluate((s, t, ex) => {
    const b = [...document.querySelectorAll(s)].find((x) => [x.textContent, x.getAttribute('aria-label'), x.getAttribute('title')].some((v) => v && (ex ? v.trim() === t : v.trim().startsWith(t))));
    if (!b) return false;
    b.click();
    return true;
  }, sel, text, exact);
  if (!found) { ctx.fail(); console.log(`FAIL no button "${text}" in ${sel}`); }
  return found;
}

/** A page with console errors and external requests recorded. */
export async function newPage(browser, origins) {
  const page = await browser.newPage();
  page.errors = [];
  page.external = [];
  page.on('console', (m) => { if (m.type() === 'error' && !/404/.test(m.text())) page.errors.push(m.text().slice(0, 200)); });
  page.on('pageerror', (e) => page.errors.push('pageerror: ' + String(e).slice(0, 300)));
  page.on('request', (r) => {
    const u = r.url();
    if (!u.startsWith('data:') && !u.startsWith('blob:') && !origins.some((o) => u.startsWith(o))) page.external.push(u);
  });
  return page;
}

export { launch };
