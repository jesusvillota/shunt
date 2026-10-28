// Test-only wrapper: keep the production page guards and wait for tab cleanup.
import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
const cli = __CLI__;
const ledgerPath = __LEDGER__;
if (process.argv.includes('--cleanup')) {
  const ledger = JSON.parse(readFileSync(ledgerPath, 'utf8'));
  for (const id of ledger.targets ?? []) {
    const response = await fetch(new URL(`/json/close/${encodeURIComponent(id)}`, __CDP__),
      { signal: AbortSignal.timeout(3000) });
    if (!response.ok && response.status !== 404) throw new Error('Owned tab cleanup failed');
  }
  process.exit(0);
}
const { BrowserDriver } = await import(new URL('./driver.mjs', pathToFileURL(cli)));
const request = BrowserDriver.prototype.request;
BrowserDriver.prototype.request = async function (input, emit) {
  const ledger = JSON.parse(readFileSync(ledgerPath, 'utf8'));
  const save = () => {
    ledger.targets = JSON.parse(readFileSync(ledgerPath, 'utf8')).targets ?? [];
    writeFileSync(ledgerPath, JSON.stringify(ledger));
  };
  const fail = code => emit({ version: 1, id: input.id, type: 'error', error: {
    code, message: 'Live harness stopped', submission: 'not_sent',
  } });
  if (ledger.stopped || Date.now() >= ledger.deadline ||
      (input.type === 'generate' && ledger.sends >= 6)) {
    ledger.stopped = true; save(); fail('timeout'); return;
  }
  // Reserve before any possible click; a killed helper leaves a permanent latch.
  ledger.stopped = true;
  if (input.type === 'generate') {
    ledger.sends++;
    const transcript = JSON.parse(input.prompt.split('\n').at(-1));
    ledger.results = transcript.input.filter(item =>
      ['function_call_output', 'custom_tool_call_output'].includes(item.type)).length;
  }
  save();
  const newPage = this.context.newPage.bind(this.context);
  this.context.newPage = async (...args) => {
    const page = await newPage(...args);
    const session = await this.context.newCDPSession(page);
    const { targetInfo } = await session.send('Target.getTargetInfo');
    ledger.targets.push(targetInfo.targetId);
    writeFileSync(ledgerPath, JSON.stringify(ledger));
    await session.detach();
    return page;
  };
  let terminal;
  let answer = '';
  await request.call(this, { ...input,
    timeout_ms: Math.max(1, Math.min(input.timeout_ms, ledger.deadline - Date.now())),
  }, event => {
    if (event.type === 'text_delta') { answer += event.text; emit(event); }
    else terminal = event;
  });
  if (!terminal) { save(); fail('internal_error'); return; }
  if (terminal.type === 'done') {
    ledger.calls += answer.split('\n').filter(line => {
      const marker = line.indexOf('>>>');
      if (marker < 0) return false;
      try { return ['function_call', 'custom_tool_call'].includes(JSON.parse(line.slice(marker + 3)).type); }
      catch { return false; }
    }).length;
  }
  ledger.stopped = !['done', 'health'].includes(terminal.type);
  save();
  emit(terminal);
};
process.argv.push('--chrome', '--cdp-url', __CDP__);
await import(pathToFileURL(cli));
