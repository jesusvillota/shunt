// All page access stays in the rendered UI. Unknown markup fails closed.
export class PageError extends Error {
  constructor(code) { super(code); this.code = code; }
}

// This function also runs inside the page immediately before the physical click.
export function inspect(action) {
  const visible = (selector) => [...document.querySelectorAll(selector)]
    .filter(el => el.getClientRects().length && getComputedStyle(el).visibility !== 'hidden');
  if (visible('[data-login-button], iframe[src*="challenges.cloudflare.com"]').length)
    return { error: 'logged_out' };
  const choices = visible('[role="group"][aria-label="Composer mode"] button[aria-pressed]');
  if (choices.length !== 2 ||
      !choices.some(el => el.textContent.trim() === 'Chat' && el.getAttribute('aria-pressed') === 'true') ||
      !choices.some(el => el.textContent.trim() === 'Work' && el.getAttribute('aria-pressed') === 'false'))
    return { error: 'not_chat_mode' };
  const one = selector => { const elements = visible(selector); return elements.length === 1 ? elements[0] : null; };
  const editor = one('[role="textbox"][contenteditable="true"][data-composer-markdown]');
  const send = one('button[type="submit"]');
  const model = one('button[aria-label="Select ChatGPT model"]');
  const account = one('button[aria-label="Open profile menu"]');
  const prompt = editor?.textContent === '' ? '' : editor?.innerText;
  if (!editor || (!send && !(prompt === '' && one('button[aria-label="Start Voice"]'))) || !model || !account) return { error: 'ui_drift' };
  const effort = [...model.querySelectorAll('[aria-hidden="false"]')].filter(el => el.getClientRects().length);
  if (effort.length !== 1 || !effort[0].innerText.trim()) return { error: 'ui_drift' };
  const state = { model: effort[0].innerText.trim(), prompt, disabled: !send || send.disabled };
  if (action) {
    if ((action.model !== undefined && state.model !== action.model) || state.prompt !== action.prompt || state.disabled)
      return { error: 'ui_drift' };
    send.click();
  }
  return state;
}

export class ChatPage {
  constructor(page, url) { this.page = page; this.url = url; }
  async open(timeout) {
    await this.page.goto(this.url, { waitUntil: 'domcontentloaded', timeout });
    // The composer or a login/challenge indicator must have rendered. No network-idle heuristic.
    try {
      await this.page.locator('[role="textbox"][contenteditable="true"][data-composer-markdown], [data-login-button], iframe[src*="challenges.cloudflare.com"]')
        .first().waitFor({ state: 'visible', timeout: Math.min(timeout, 3000) });
      const readyBy = Date.now() + Math.min(timeout, 3000);
      while ((await this.page.evaluate(inspect)).error === 'ui_drift') {
        if (Date.now() >= readyBy) throw new PageError('ui_drift');
        await this.page.waitForTimeout(25);
      }
    } catch (error) {
      if (error.name === 'TimeoutError') throw new PageError('ui_drift');
      throw error;
    }
  }
  async check() {
    const state = await this.page.evaluate(inspect);
    if (state.error) throw new PageError(state.error);
    return state;
  }
  async send(prompt, model, attempted) {
    const state = await this.check();
    if (model !== undefined && state.model !== model) throw new PageError('invalid_request');
    if (state.prompt !== '') throw new PageError('ui_drift');
    await this.page.evaluate(async text => navigator.clipboard.writeText(text), prompt);
    await this.page.locator('[role="textbox"][contenteditable="true"][data-composer-markdown]').focus();
    await this.page.locator('[role="textbox"][contenteditable="true"][data-composer-markdown]').press('ControlOrMeta+V');
    const inserted = await this.check();
    if (inserted.prompt !== prompt || (model !== undefined && inserted.model !== model) || inserted.disabled)
      throw new PageError('ui_drift');
    // Evaluate guard and click in one JS task: no mode change can interleave them.
    attempted();
    const result = await this.page.evaluate(inspect, { prompt, model });
    if (result.error) throw new PageError(result.error);
  }
  async answer() {
    return this.page.evaluate(() => {
      const users = [...document.querySelectorAll('[data-user-message-bubble="true"]')];
      const answers = [...document.querySelectorAll('[data-markdown-text-style="assistant-message"]')];
      if (users.length > 1 || answers.length > 1) return { error: 'ui_drift' };
      const answer = answers[0];
      const turn = answer?.closest('[data-turn-key]');
      return {
        accepted: users.length === 1,
        identity: turn?.getAttribute('data-turn-key'),
        text: answer?.innerText ?? '',
        invalid: !!answer && (!turn || !answer.querySelector('[data-dil-message-id]')),
        // shortcut: Copy/Stop use English UI labels; replace when stable attributes are available.
        done: !!turn?.querySelector('[data-talvt-turn-state="complete"] button[aria-label="Copy"]') &&
          !document.querySelector('button[aria-label="Stop"]'),
      };
    });
  }
}
