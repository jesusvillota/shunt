import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { chromium } from 'playwright';
import { BrowserDriver } from '../driver.mjs';
import { serve } from './fixture.mjs';

const root = new URL('../', import.meta.url);
const prompt = '  exact 🌍\n你好\nlast line  ';
async function harness(state, run) {
  const fixture = await serve();
  const profile = await mkdtemp(join(tmpdir(), 'shunt-web-test-'));
  const context = await chromium.launchPersistentContext(profile, {
    headless: true, permissions: ['clipboard-read', 'clipboard-write'],
  });
  const driver = new BrowserDriver(context, 'fixture', profile, `${fixture.url}?state=${state}`);
  let id = 0;
  const request = async (fields = {}) => {
    const events = [];
    await driver.request({ version: 1, id: `request-${id++}`, type: 'generate', account: 'fixture',
      profile_dir: profile, timeout_ms: 5000, prompt, model: 'fixture-model', ...fields }, e => events.push(e));
    return events;
  };
  try { await run({ request, fixture, context }); }
  finally { await context.close(); fixture.server.closeAllConnections(); await new Promise(r => fixture.server.close(r)); await rm(profile, { recursive: true, force: true }); }
}

test('clipboard preserves prompt; streams before completion; fresh chat per turn; health sends nothing', async () => {
  await harness('chat', async ({ request, fixture }) => {
    assert.equal((await request({ type: 'health' })).at(-1).type, 'health');
    assert.equal(fixture.stats.sends, 0);
    for (let i = 0; i < 2; i++) {
      const events = await request();
      assert.equal(events.at(-1).type, 'done');
      assert.ok(events.filter(e => e.type === 'text_delta').length >= 2);
      assert.equal(events.filter(e => e.type === 'text_delta').map(e => e.text).join(''), 'Hello 🌍\n你好');
    }
    assert.deepEqual(fixture.stats.prompts, [prompt, prompt]);
    assert.equal(fixture.stats.opens, 3);
  });
});

for (const [state, code] of [
  ['work', 'not_chat_mode'], ['logged_out', 'logged_out'], ['drift', 'ui_drift'], ['composer_drift', 'ui_drift'],
  ['missing', 'not_chat_mode'], ['unknown', 'not_chat_mode'], ['conflicting', 'not_chat_mode'],
  ['switch_on_paste', 'not_chat_mode'],
]) test(`${state}: fails closed and sends nothing`, async () => {
  await harness(state, async ({ request, fixture }) => {
    for (const type of ['generate', 'health']) {
      // Health does not paste, so only this case still reports ready.
      if (state === 'switch_on_paste' && type === 'health') continue;
      const events = await request({ type });
      assert.equal(events.length, 1);
      assert.equal(events[0].error.code, code);
      assert.equal(events[0].error.submission, 'not_sent');
    }
    assert.equal(fixture.stats.sends, 0);
  });
});

test('validation and profile/account binding happen before page actions', async () => {
  await harness('chat', async ({ request, fixture }) => {
    for (const [fields, code] of [
      [{ version: 2 }, 'unsupported_version'], [{ timeout_ms: 0 }, 'invalid_request'],
      [{ account: 'other' }, 'account_mismatch'], [{ profile_dir: '/other' }, 'account_mismatch'],
      [{ prompt: ' ' }, 'invalid_request'], [{ type: 'unknown' }, 'invalid_request'],
    ]) assert.equal((await request(fields))[0].error.code, code);
    assert.equal(fixture.stats.opens, 0);
    assert.equal((await request({ model: 'unknown' }))[0].error.code, 'invalid_request');
    assert.equal(fixture.stats.sends, 0);
  });
});

for (const state of ['timeout', 'uncertain', 'revision', 'empty'])
  test(`${state}: terminal and submission evidence`, async () => {
    await harness(state, async ({ request, context }) => {
      const events = await request({ timeout_ms: 1500 });
      const last = events.at(-1);
      if (state === 'empty') assert.deepEqual(events.map(e => e.type), ['done']);
      else {
        assert.equal(last.type, 'error');
        assert.equal(last.error.code, state === 'revision' ? 'ui_drift' : 'timeout');
        assert.equal(last.error.submission, state === 'uncertain' ? 'may_have_been_sent' : 'sent');
      }
      assert.equal(context.pages().length, 1, 'request page closed; only startup tab remains');
    });
  });

test('production source has no cookie, storage, credential or network extraction access', async () => {
  const forbidden = /\b(?:cookies|addCookies|clearCookies|storageState|localStorage|sessionStorage|indexedDB|password|credentials)\b|document\s*\.\s*cookie|readFile|readFileSync|\.route\(|\.request\.|fetch\(|Network\./;
  for (const file of ['cli.mjs', 'driver.mjs', 'page.mjs']) {
    assert.doesNotMatch(await readFile(new URL(file, root), 'utf8'), forbidden, file);
  }
});

test('CLI uses JSONL, keeps stdout clean and exits on malformed input', { timeout: 15000 }, async () => {
  const profile = await mkdtemp(join(tmpdir(), 'shunt-web-cli-'));
  const fixture = await serve();
  const child = spawn(process.execPath, [new URL('cli.mjs', root).pathname, '--account', 'fixture',
    '--profile-dir', profile, '--headless', '--fixture-url', fixture.url], { stdio: ['pipe', 'pipe', 'pipe'] });
  const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  let stderr = ''; child.stderr.on('data', data => { stderr += data; });
  const exit = new Promise(resolve => child.once('exit', resolve));
  try {
    child.stdin.write(`${JSON.stringify({ version: 1, id: 'health', type: 'health', account: 'fixture', profile_dir: profile, timeout_ms: 5000 })}\n`);
    assert.equal(JSON.parse((await lines.next()).value).type, 'health');
    child.stdin.write('malformed\n');
    assert.equal(await exit, 1);
    assert.equal(stderr, 'Browser driver transport failed\n');
  } finally {
    child.kill(); fixture.server.closeAllConnections(); fixture.server.close();
    await rm(profile, { recursive: true, force: true });
  }
});

for (const stop of ['eof', 'SIGTERM']) test(`CLI ${stop} cancels an active generation`, { timeout: 15000 }, async () => {
  const profile = await mkdtemp(join(tmpdir(), 'shunt-web-cancel-'));
  const fixture = await serve();
  const child = spawn(process.execPath, [new URL('cli.mjs', root).pathname, '--account', 'fixture',
    '--profile-dir', profile, '--headless', '--fixture-url', `${fixture.url}?state=timeout`]);
  const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  const exit = new Promise(resolve => child.once('exit', resolve));
  try {
    child.stdin.write(`${JSON.stringify({ version: 1, id: 'cancel', type: 'generate', prompt,
      model: 'fixture-model', account: 'fixture', profile_dir: profile, timeout_ms: 60000 })}\n`);
    assert.equal(JSON.parse((await lines.next()).value).type, 'text_delta');
    if (stop === 'eof') child.stdin.end(); else child.kill(stop);
    assert.equal(await exit, 0);
    assert.equal(fixture.stats.sends, 1);
  } finally {
    child.kill(); fixture.server.closeAllConnections(); fixture.server.close();
    await rm(profile, { recursive: true, force: true });
  }
});
