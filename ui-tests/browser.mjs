// Launch the browser under test: Firefox by default, or BROWSER=chrome with
// FIREFOX=/path/to/chrome (the variable names the executable in both cases).
import puppeteer from 'puppeteer-core';

export function launch() {
  const chrome = process.env.BROWSER === 'chrome';
  return puppeteer.launch({
    browser: chrome ? 'chrome' : 'firefox',
    executablePath: process.env.FIREFOX,
    headless: true,
    defaultViewport: { width: 1440, height: 1100 },
    // The tests may run as the root of a user namespace, where Chrome's sandbox cannot start.
    args: chrome ? ['--no-sandbox', '--autoplay-policy=no-user-gesture-required'] : [],
  });
}
