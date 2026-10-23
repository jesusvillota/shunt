import { createServer } from 'node:http';
import { pathToFileURL } from 'node:url';

// A local imitation, not a captured authenticated page. No production-driver test hooks.
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
      ${scenario === 'logged_out' ? '<button data-testid="login-button">Connexion</button>' : ''}
      <div role="tablist">
        <button role="tab" aria-selected="${scenario !== 'work'}">Chat</button>
        <button role="tab" aria-selected="${scenario === 'work' || scenario === 'conflicting'}">Work</button>
      </div>
      <button data-testid="accounts-profile-button">Account</button>
      <button data-testid="model-switcher-dropdown-button">fixture-model</button>
      <div id="${scenario === 'composer_drift' ? 'changed-composer' : 'prompt-textarea'}" contenteditable="true" style="white-space:pre-wrap"></div>
      <button data-testid="${scenario === 'drift' ? 'changed-send' : 'send-button'}">Envoyer</button>
      <main></main>
      <script>
        const scenario = ${JSON.stringify(scenario)};
        const editor = document.querySelector('#prompt-textarea');
        if (scenario === 'missing') document.querySelector('[role=tablist]').remove();
        if (scenario === 'unknown') document.querySelector('[role=tab]').textContent = 'Discussion';
        editor?.addEventListener('input', () => {
          if (scenario === 'switch_on_paste') {
            const tabs = document.querySelectorAll('[role=tab]');
            tabs[0].setAttribute('aria-selected', 'false'); tabs[1].setAttribute('aria-selected', 'true');
          }
        });
        document.querySelector('button[data-testid$="send-button"]')?.addEventListener('click', async () => {
          const prompt = editor.innerText;
          await fetch('/sent', { method: 'POST', body: prompt });
          editor.innerText = '';
          const main = document.querySelector('main');
          main.innerHTML = '<div data-message-author-role="user"></div><article data-turn-id="turn-1"><div data-message-author-role="assistant"><div class="markdown" style="white-space:pre-wrap"></div></div></article><button data-testid="stop-button">Arrêter</button>';
          document.querySelector('[data-message-author-role=user]').innerText = prompt;
          const answer = document.querySelector('.markdown');
          if (scenario === 'uncertain') { main.innerHTML = ''; return; }
          if (scenario !== 'empty') {
            answer.innerText = 'Hello';
            await new Promise(resolve => setTimeout(resolve, 150));
            answer.innerText += ' 🌍\\n你好';
          }
          if (scenario === 'timeout') return;
          if (scenario === 'revision') { answer.innerText = 'Rewritten'; return; }
          document.querySelector('[data-testid=stop-button]').remove();
          const copy = document.createElement('button');
          copy.dataset.testid = 'copy-turn-action-button'; copy.textContent = 'Copier';
          document.querySelector('article').append(copy);
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
