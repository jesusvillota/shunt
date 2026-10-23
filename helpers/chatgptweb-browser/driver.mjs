import { isAbsolute } from 'node:path';
import { ChatPage, PageError } from './page.mjs';

const nonblank = value => typeof value === 'string' && value.trim().length > 0;
const uint = value => Number.isInteger(value) && value >= 0 && value <= 0xffffffff;

export class BrowserDriver {
  constructor(context, account, profile, url) {
    this.context = context; this.account = account; this.profile = profile; this.url = url;
    this.ids = new Set();
  }
  async request(request, emit) {
    let submission = 'not_sent';
    let page;
    let expired = false;
    const event = (type, fields = {}) => emit({ version: 1, id: request.id, type, ...fields });
    const fail = code => event('error', { error: {
      code, message: `Browser driver: ${code}`, submission,
    } });
    if (!nonblank(request.id)) throw new Error('Uncorrelated request');
    if (!uint(request.version)) return fail('invalid_request');
    if (request.version !== 1) return fail('unsupported_version');
    if (!nonblank(request.account) || !nonblank(request.profile_dir) || !isAbsolute(request.profile_dir) ||
        !uint(request.timeout_ms) || request.timeout_ms === 0 || this.ids.has(request.id))
      return fail('invalid_request');
    this.ids.add(request.id);
    if (request.account !== this.account || request.profile_dir !== this.profile) return fail('account_mismatch');
    if (!['health', 'generate'].includes(request.type) || (request.type === 'generate' &&
        (!nonblank(request.prompt) || !nonblank(request.model)))) return fail('invalid_request');
    const deadline = Date.now() + request.timeout_ms;
    const timer = setTimeout(() => {
      expired = true;
      // Close the active page before returning timeout; never leave a generation running.
      void page?.close().catch(() => {});
    }, request.timeout_ms);
    try {
      page = await this.context.newPage();
      if (expired) throw new PageError('timeout');
      page.setDefaultTimeout(Math.max(1, deadline - Date.now()));
      const chat = new ChatPage(page, this.url);
      await chat.open(Math.max(1, deadline - Date.now()));
      await chat.check();
      if (request.type === 'health') { event('health'); return; }
      await chat.send(request.prompt, request.model, () => { submission = 'may_have_been_sent'; });
      let text = '';
      let identity;
      while (!expired) {
        const state = await chat.answer();
        if (state.accepted) submission = 'sent';
        if (state.error || state.invalid || (identity && state.identity !== identity)) throw new PageError('ui_drift');
        if (state.identity) identity = state.identity;
        if (!state.text.startsWith(text)) throw new PageError('ui_drift');
        if (state.text !== text) {
          if (!state.accepted) throw new PageError('ui_drift');
          event('text_delta', { text: state.text.slice(text.length) }); text = state.text;
        }
        if (state.done && state.accepted && identity) { event('done'); return; }
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      throw new PageError('timeout');
    } catch (error) {
      const code = expired || error.name === 'TimeoutError' ? 'timeout' :
        error instanceof PageError ? error.code : 'internal_error';
      // Guard rejection proves no click, despite entering the evaluate call.
      if (code === 'not_chat_mode' || code === 'invalid_request') submission = 'not_sent';
      await page?.close().catch(() => {});
      fail(code);
    } finally {
      clearTimeout(timer);
      await page?.close().catch(() => {});
    }
  }
}
