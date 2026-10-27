import { parseArgs } from 'node:util';
import { isAbsolute, normalize } from 'node:path';
import { createInterface } from 'node:readline';
import { chromium } from 'playwright';
import { BrowserDriver } from './driver.mjs';

const { values } = parseArgs({ options: {
  account: { type: 'string' }, 'profile-dir': { type: 'string' },
  'fixture-url': { type: 'string' }, headless: { type: 'boolean', default: false },
  chrome: { type: 'boolean', default: false },
  'cdp-url': { type: 'string' },
} });
const profile = values['profile-dir'];
if (!values.account?.trim() || !profile || !isAbsolute(profile) || normalize(profile) !== profile)
  throw new Error('Use --account ID --profile-dir ABSOLUTE_NORMALIZED_PATH');
const url = values['fixture-url'] ?? 'https://chatgpt.com/';
const target = new URL(url);
if (values['fixture-url'] && (target.protocol !== 'http:' || target.hostname !== '127.0.0.1'))
  throw new Error('Fixture URL must use http://127.0.0.1');

const cdp = new URL(values['cdp-url'] ?? 'http://127.0.0.1:9333');
if ((values['cdp-url'] && !values.chrome) || (values.chrome &&
    (values.headless || cdp.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(cdp.hostname) ||
      cdp.href.includes('@'))))
  throw new Error('CDP requires --chrome, an HTTP loopback URL without authentication, and no --headless');
const browser = values.chrome ? await chromium.connectOverCDP(cdp.href) : null;
const context = browser ? browser.contexts()[0] : await chromium.launchPersistentContext(profile, {
  headless: values.headless, permissions: ['clipboard-read', 'clipboard-write'],
});
const driver = new BrowserDriver(context, values.account, profile, url);
let closing;
const shutdown = () => closing ??= browser ? driver.close() : context.close();
process.once('SIGTERM', () => { void shutdown().finally(() => process.exit(0)); });
process.once('SIGINT', () => { void shutdown().finally(() => process.exit(0)); });
const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
// EOF cancels an active request instead of waiting for its generation deadline.
input.once('close', () => { void shutdown(); });
try {
  for await (const line of input) {
    const request = JSON.parse(line);
    await driver.request(request, event => process.stdout.write(`${JSON.stringify(event)}\n`));
  }
} catch {
  process.stderr.write('Browser driver transport failed\n');
  process.exitCode = 1;
} finally {
  input.close();
  process.stdin.destroy();
  await shutdown();
}
// End the CDP transport without sending Browser.close to the owner's browser.
if (browser) process.exit(process.exitCode ?? 0);
