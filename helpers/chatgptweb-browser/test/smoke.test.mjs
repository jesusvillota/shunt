import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:net';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { chromium } from 'playwright';
import { serve } from './fixture.mjs';
const root = new URL('../', import.meta.url);

// Execute the exact template used by the Rust live harness, with local pages only.
test('live launcher guards, shared budgets, failure latch and owner preservation', { timeout: 60000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'shunt-live-fixture-'));
  const fixture = await serve();
  const socket = createServer();
  await new Promise(resolve => socket.listen(0, '127.0.0.1', resolve));
  const port = socket.address().port;
  await new Promise(resolve => socket.close(resolve));
  const context = await chromium.launchPersistentContext(join(directory, 'profile'), {
    headless: true, args: [`--remote-debugging-port=${port}`],
    permissions: ['clipboard-read', 'clipboard-write'],
  });
  const owner = context.pages()[0];
  await owner.goto(`${fixture.url}?state=chat`);
  const ownerUrl = owner.url();
  const ledgerPath = join(directory, 'ledger.json');
  const launcher = join(directory, 'launcher.mjs');
  const template = await readFile(new URL('../../tests/common/chatgptweb_smoke_launcher.mjs', root), 'utf8');
  await writeFile(launcher, template.replace('__CLI__', JSON.stringify(new URL('cli.mjs', root).pathname))
    .replace('__LEDGER__', JSON.stringify(ledgerPath)).replaceAll('__CDP__', JSON.stringify(`http://127.0.0.1:${port}`)));
  const read = async () => JSON.parse(await readFile(ledgerPath, 'utf8'));
  const reset = async (overrides = {}) => writeFile(ledgerPath, JSON.stringify({
    sends: 0, calls: 0, results: 0, stopped: false, deadline: Date.now() + 900000, ...overrides,
  }));
  const run = async (type, state = 'chat') => {
    const child = spawn(process.execPath, [launcher, '--account', 'cemfi', '--profile-dir', directory,
      '--fixture-url', `${fixture.url}?state=${state}`]);
    const exit = new Promise(resolve => child.once('exit', resolve));
    const frames = [];
    try {
      child.stdin.write(`${JSON.stringify({ version: 1, id: 'fixture', type, account: 'cemfi',
        profile_dir: directory, timeout_ms: 1500, model: 'Medium',
        prompt: 'instructions\n' + JSON.stringify({ input: [{ role: 'user', content: 'harmless' }] }),
      })}\n`);
      for await (const line of createInterface({ input: child.stdout })) {
        const event = JSON.parse(line); frames.push(event);
        if (event.type !== 'text_delta') { child.stdin.end(); break; }
      }
      assert.equal(await exit, 0);
      assert.equal(owner.isClosed(), false);
      assert.equal(owner.url(), ownerUrl);
      assert.equal(await owner.evaluate(() => 6 * 7), 42);
      assert.equal(context.pages().length, 1, 'helper tab closes before terminal is returned');
      return frames.at(-1);
    } finally { child.kill(); }
  };
  try {
    await reset();
    assert.equal((await run('health')).type, 'health');
    assert.equal(fixture.stats.sends, 0);
    for (let i = 0; i < 6; i++) assert.equal((await run('generate')).type, 'done');
    assert.equal((await read()).sends, 6);
    assert.equal((await run('generate')).type, 'error');
    assert.equal(fixture.stats.sends, 6);
    for (const state of ['work', 'switch_on_paste']) {
      await reset(); const sends = fixture.stats.sends;
      assert.equal((await run('health', state)).type, state === 'work' ? 'error' : 'health');
      assert.equal((await run('generate', state)).type, 'error');
      assert.equal(fixture.stats.sends, sends, 'Chat-only preflight and atomic send guard');
      assert.equal((await read()).stopped, true);
      assert.equal((await run('generate')).type, 'error', 'errors persist across fresh helpers');
    }
    await reset({ deadline: Date.now() - 1 });
    const sends = fixture.stats.sends;
    assert.equal((await run('generate')).type, 'error');
    assert.equal(fixture.stats.sends, sends);
    await reset();
    assert.equal((await run('generate', 'uncertain')).error.submission, 'may_have_been_sent');
    assert.equal((await run('generate')).type, 'error');
    assert.equal(fixture.stats.sends, sends + 1, 'uncertain send never retries');
    await reset();
    const child = spawn(process.execPath, [launcher, '--account', 'cemfi', '--profile-dir', directory,
      '--fixture-url', `${fixture.url}?state=timeout`]);
    const killed = new Promise(resolve => child.once('exit', resolve));
    child.stdin.write(`${JSON.stringify({version:1,id:'killed',type:'generate',account:'cemfi',
      profile_dir:directory,timeout_ms:60000,model:'Medium',prompt:'instructions\n'+JSON.stringify({input:[]})})}\n`);
    // Wait for the real send and first text; then reproduce ProcessDriver cancellation.
    const lines = createInterface({input:child.stdout})[Symbol.asyncIterator]();
    assert.equal(JSON.parse((await lines.next()).value).type, 'text_delta');
    child.kill('SIGKILL'); await killed;
    assert.equal((await read()).stopped, true);
    const extraOwner = await context.newPage();
    const cleanup = spawn(process.execPath, [launcher, '--cleanup']);
    assert.equal(await new Promise(resolve => cleanup.once('exit', resolve)), 0);
    assert.equal(context.pages().length, 2);
    assert.equal(owner.isClosed(), false);
    assert.equal(extraOwner.isClosed(), false, 'concurrent owner tab preserved');
    await extraOwner.close();
    await reset({ stopped: true });
    assert.equal((await run('generate')).type, 'error', 'a killed helper leaves its reservation latched');
  } finally {
    await context.close(); fixture.server.closeAllConnections();
    await new Promise(resolve => fixture.server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});
