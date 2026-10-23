// All page access stays in the rendered UI. Unknown markup fails closed.
export class PageError extends Error {
  constructor(code) { super(code); this.code = code; }
}

// This function also runs inside the page immediately before the physical click.
export function inspect(action) {
  const visible = (selector) => [...document.querySelectorAll(selector)]
    .filter(el => el.getClientRects().length && getComputedStyle(el).visibility !== 'hidden');
  if (visible('[data-testid="login-button"], iframe[src*="challenges.cloudflare.com"]').length)
    return { error: 'logged_out' };
  const choices = visible('[role="tab"][aria-selected], [role="radio"][aria-checked]')
    .filter(el => ['Chat', 'Work'].includes(el.textContent.trim()));
  const selected = choices.filter(el =>
    el.getAttribute('aria-selected') === 'true' || el.getAttribute('aria-checked') === 'true');
  if (choices.length !== 2 || selected.length !== 1 || selected[0].textContent.trim() !== 'Chat')
    return { error: 'not_chat_mode' };
  const one = selector => { const elements = visible(selector); return elements.length === 1 ? elements[0] : null; };
  const editor = one('#prompt-textarea[contenteditable="true"]');
  const send = one('[data-testid="send-button"]');
  const model = one('[data-testid="model-switcher-dropdown-button"]');
  const account = one('[data-testid="accounts-profile-button"]');
  if (!editor || !send || !model || !account) return { error: 'ui_drift' };
  const state = { model: model.textContent.trim(), prompt: editor.innerText, disabled: send.disabled };
  if (action) {
    if (state.model !== action.model || state.prompt !== action.prompt || state.disabled)
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
      await this.page.locator('#prompt-textarea, [data-testid="login-button"], iframe[src*="challenges.cloudflare.com"]')
        .first().waitFor({ state: 'visible', timeout: Math.min(timeout, 3000) });
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
    if (state.model !== model) throw new PageError('invalid_request');
    if (state.prompt !== '') throw new PageError('ui_drift');
    await this.page.evaluate(async text => navigator.clipboard.writeText(text), prompt);
    await this.page.locator('#prompt-textarea').focus();
    await this.page.locator('#prompt-textarea').press('ControlOrMeta+V');
    const inserted = await this.check();
    if (inserted.prompt !== prompt || inserted.model !== model || inserted.disabled)
      throw new PageError('ui_drift');
    // Evaluate guard and click in one JS task: no mode change can interleave them.
    attempted();
    const result = await this.page.evaluate(inspect, { prompt, model });
    if (result.error) throw new PageError(result.error);
  }
  async answer() {
    return this.page.evaluate(() => {
      const users = [...document.querySelectorAll('[data-message-author-role="user"]')];
      const answers = [...document.querySelectorAll('[data-message-author-role="assistant"]')];
      if (users.length > 1 || answers.length > 1) return { error: 'ui_drift' };
      const answer = answers[0];
      const turn = answer?.closest('[data-turn-id]');
      return {
        accepted: users.length === 1,
        identity: turn?.getAttribute('data-turn-id'),
        text: answer?.querySelector('.markdown')?.innerText ?? '',
        invalid: !!answer && (!turn || !answer.querySelector('.markdown')),
        // A completed turn exposes its action bar. Stop absence alone is insufficient.
        done: !!turn?.querySelector('[data-testid="copy-turn-action-button"]') &&
          !document.querySelector('[data-testid="stop-button"]'),
      };
    });
  }
}
