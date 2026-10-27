import { createServer } from 'node:http';
import { pathToFileURL } from 'node:url';

// Sanitized structural projection of the live Chrome page (2026-10-27).
// Work/drift and timing variants mutate this markup locally; no live Work send.
export async function serve() {
  const stats = { sends: 0, prompts: [], opens: 0 };
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/stats') {
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify(stats)); return;
    }
    if (url.pathname === '/sent') {
      let body = ''; for await (const chunk of request) body += chunk;
      stats.sends++; stats.prompts.push(body); response.end('ok'); return;
    }
    if (url.pathname === '/favicon.ico') { response.end(); return; }
    stats.opens++;
    const scenario = url.searchParams.get('state') ?? 'chat';
    response.setHeader('Content-Type', 'text/html; charset=utf-8');
    response.end(`<!doctype html><html><body>
      ${scenario === 'logged_out' ? '<button command="show-modal" commandfor="mobile-auth-dialog" data-login-button data-mobile-auth-entry-action="login" type="button">Log in</button>' : ''}
      <div role="group" aria-label="Composer mode">
        <button aria-pressed="${scenario !== 'work'}">Chat</button>
        <button aria-pressed="${scenario === 'work' || scenario === 'conflicting'}">Work</button>
      </div>
      <button aria-label="Open profile menu">Account</button>
      <button aria-label="Select ChatGPT model" data-selected-reasoning-effort="medium"><span aria-hidden="true">Thinking effort</span><span aria-hidden="false">Medium</span></button>
      <div id="fixture-editor" role="${scenario === 'composer_drift' ? 'changed-composer' : 'textbox'}" data-composer-markdown contenteditable="true" style="white-space:pre-wrap"><p><br class="ProseMirror-trailingBreak"></p></div>
      <button type="${scenario === 'drift' ? 'button' : 'submit'}">Envoyer</button>
      <main></main>
      <script>
        const scenario = ${JSON.stringify(scenario)};
        const editor = document.querySelector('#fixture-editor');
        if (scenario === 'loading') {
          const account = document.querySelector('[aria-label="Open profile menu"]');
          account.hidden = true; setTimeout(() => { account.hidden = false; }, 150);
        }
        if (scenario === 'missing') document.querySelector('[aria-label="Composer mode"]').remove();
        if (scenario === 'mixed_work') document.querySelectorAll('[aria-pressed]')[1].setAttribute('aria-pressed', 'mixed');
        if (scenario === 'duplicate_chat') document.querySelectorAll('[aria-pressed]')[1].textContent = 'Chat';
        if (scenario === 'unknown') document.querySelector('[aria-pressed]').textContent = 'Discussion';
        editor?.addEventListener('input', () => {
          if (scenario === 'switch_on_paste') {
            const choices = document.querySelectorAll('[aria-pressed]');
            choices[0].setAttribute('aria-pressed', 'false'); choices[1].setAttribute('aria-pressed', 'true');
          }
        });
        document.querySelector('button[type="submit"]')?.addEventListener('click', async () => {
          const prompt = editor.innerText;
          await fetch('/sent', { method: 'POST', body: prompt });
          editor.innerText = '';
          const main = document.querySelector('main');
          main.innerHTML = '<div data-turn-key="turn-1"><div data-talvt-turn-state="streaming"><div data-user-message-bubble="true"></div><div data-markdown-text-style="assistant-message"><div data-dil-source-message-id="answer-1" data-markdown-copy="contents"><div data-dil-message-id="answer-1" style="white-space:pre-wrap"></div></div></div></div></div><button aria-label="Stop">Arrêter</button>';
          document.querySelector('[data-user-message-bubble]').innerText = prompt;
          const answer = document.querySelector('[data-dil-message-id]');
          if (scenario === 'uncertain') { main.innerHTML = ''; return; }
          if (scenario !== 'empty') {
            answer.innerText = 'Hello';
            await new Promise(resolve => setTimeout(resolve, 150));
            answer.innerText += ' 🌍\\n你好';
          }
          if (scenario === 'timeout') return;
          if (scenario === 'revision') { answer.innerText = 'Rewritten'; return; }
          document.querySelector('button[aria-label=Stop]').remove();
          const copy = document.createElement('button');
          copy.setAttribute('aria-label', 'Copy'); copy.textContent = 'Copier';
          document.querySelector('[data-talvt-turn-state]').setAttribute('data-talvt-turn-state', 'complete');
          document.querySelector('[data-talvt-turn-state]').append(copy);
        });
      </script></body></html>`);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { server, stats, url: `http://127.0.0.1:${server.address().port}/` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { server, url } = await serve();
  process.stdout.write(`${url}\n`);
  process.stdin.resume();
  process.stdin.once('end', () => server.closeAllConnections() || server.close());
}
