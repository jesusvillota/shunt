import { parseArgs } from 'node:util';
import { isAbsolute, normalize } from 'node:path';
import { createInterface } from 'node:readline';
import { chromium } from 'playwright';
import { BrowserDriver } from './driver.mjs';

const { values } = parseArgs({ options: {
  account: { type: 'string' }, 'profile-dir': { type: 'string' },
  'fixture-url': { type: 'string' }, headless: { type: 'boolean', default: false },
  chrome: { type: 'boolean', default: false },
} });
const profile = values['profile-dir'];
if (!values.account?.trim() || !profile || !isAbsolute(profile) || normalize(profile) !== profile)
  throw new Error('Use --account ID --profile-dir ABSOLUTE_NORMALIZED_PATH');
const url = values['fixture-url'] ?? 'https://chatgpt.com/';
const target = new URL(url);
if (values['fixture-url'] && (target.protocol !== 'http:' || target.hostname !== '127.0.0.1'))
  throw new Error('Fixture URL must use http://127.0.0.1');

const context = await chromium.launchPersistentContext(profile, {
  ...(values.chrome ? { channel: 'chrome' } : {}),
  headless: values.headless, permissions: ['clipboard-read', 'clipboard-write'],
});
let closing;
const shutdown = () => closing ??= context.close();
process.once('SIGTERM', () => { void shutdown().finally(() => process.exit(0)); });
process.once('SIGINT', () => { void shutdown().finally(() => process.exit(0)); });
const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
// EOF cancels an active request instead of waiting for its generation deadline.
input.once('close', () => { void shutdown(); });
const driver = new BrowserDriver(context, values.account, profile, url);
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
