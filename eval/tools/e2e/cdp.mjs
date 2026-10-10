// Drives the real desktop app's WebView2 over the Chrome DevTools Protocol (Node 22 built-in WebSocket). No dependencies.
// Start the app first with eval/tools/e2e/launch_app.ps1 (it opens the debug port 9222).
// Usage: node cdp.mjs <command> [args...]
//   eval "<js>"                      run an expression in the page (awaits promises) and print the JSON result
//   text                             visible text of the page
//   shot <file.png>                  screenshot
//   click "<text>"                   click the first visible button/tab/link/summary/label whose text matches (exact match preferred, case-insensitive)
//   type "<label or placeholder>" "<value>"   set a React-controlled input/textarea/select found by its label, placeholder or aria-label
//   fill "<css selector>" <file>     set a React-controlled input/textarea from a UTF-8 file (no shell quoting problems)
//   upload "<css selector>" <files>  set files on a file input
//   wait "<text>" [seconds]          wait until the text appears
//   smoke '<json array of labels>'   click each label in turn and report console errors and exceptions
//   targets                          list debuggable pages
// Host calls: eval "window.__TAURI_INTERNALS__.invoke('command_name', {camelCaseArgs})".
import fs from "node:fs";

const base = "http://127.0.0.1:9222";
const [cmd, ...args] = process.argv.slice(2);

async function targets() {
  const r = await fetch(base + "/json");
  return (await r.json()).filter((t) => t.type === "page");
}

async function open() {
  const t = (await targets())[0];
  if (!t) throw new Error("no page target");
  const ws = new WebSocket(t.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0;
  const pending = new Map();
  const events = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } else events.push(d);
  };
  const send = (method, params = {}) => new Promise((res, rej) => {
    const i = ++id;
    pending.set(i, (d) => (d.error ? rej(new Error(JSON.stringify(d.error))) : res(d.result)));
    ws.send(JSON.stringify({ id: i, method, params }));
  });
  return { send, close: () => ws.close(), events };
}

const evalIn = async (c, expression) => {
  const r = await c.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) { const x = r.exceptionDetails.exception; throw new Error((x && (x.description || (x.value !== undefined ? 'rejected: ' + JSON.stringify(x.value) : ''))) || r.exceptionDetails.text); }
  return r.result.value;
};

const helpers = `
(() => {
  const vis = (e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
  window.__cf = {
    clickText(t) {
      const els = [...document.querySelectorAll('button, [role=tab], a, summary, label')].filter(vis);
      const norm = (e) => (e.innerText || e.textContent || '').trim().replace(/\\s+/g, ' ');
      const tl = t.toLowerCase();
      let el = els.find((e) => norm(e).toLowerCase() === tl) || els.find((e) => norm(e).toLowerCase().includes(tl));
      if (!el) return 'NOT FOUND: ' + t + ' | visible: ' + els.map(norm).filter(Boolean).slice(0, 60).join(' / ');
      el.scrollIntoView({block: 'center'}); el.click(); return 'clicked: ' + norm(el);
    },
    setInput(key, value) {
      const inputs = [...document.querySelectorAll('input, textarea, select')].filter(vis);
      const label = (e) => ((e.closest('label')?.innerText) || '') + ' ' + (e.placeholder || '') + ' ' + (e.getAttribute('aria-label') || '');
      const el = inputs.find((e) => label(e).toLowerCase().includes(key.toLowerCase()));
      if (!el) return 'NOT FOUND: ' + key + ' | inputs: ' + inputs.map((e) => label(e).trim().slice(0, 40)).join(' / ');
      const proto = el.tagName === 'SELECT' ? HTMLSelectElement.prototype : el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, value);
      el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true }));
      return 'set: ' + key;
    },
  };
  return 'ok';
})()`;

const c = await open();
try {
  await c.send("Runtime.enable");
  if (cmd === "targets") console.log(JSON.stringify((await targets()).map((t) => ({ title: t.title, url: t.url }))));
  else if (cmd === "eval") console.log(JSON.stringify(await evalIn(c, args[0]), null, 1));
  else if (cmd === "text") console.log(await evalIn(c, "document.body.innerText"));
  else if (cmd === "shot") {
    const r = await c.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(args[0], Buffer.from(r.data, "base64"));
    console.log("saved " + args[0]);
  } else if (cmd === "click") {
    await evalIn(c, helpers);
    console.log(await evalIn(c, `window.__cf.clickText(${JSON.stringify(args[0])})`));
  } else if (cmd === "type") {
    await evalIn(c, helpers);
    console.log(await evalIn(c, `window.__cf.setInput(${JSON.stringify(args[0])}, ${JSON.stringify(args[1])})`));
  } else if (cmd === "upload") {
    const doc = await c.send("DOM.getDocument", { depth: 1 });
    const q = await c.send("DOM.querySelector", { nodeId: doc.root.nodeId, selector: args[0] });
    if (!q.nodeId) throw new Error("no element for " + args[0]);
    await c.send("DOM.setFileInputFiles", { nodeId: q.nodeId, files: args.slice(1) });
    console.log("uploaded " + (args.length - 1) + " file(s)");
  } else if (cmd === "fill") {
    const text = fs.readFileSync(args[1], "utf-8");
    console.log(await evalIn(c, `(()=>{const e=document.querySelector(${JSON.stringify(args[0])});if(!e)return 'no element';const proto=e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,${JSON.stringify(text)});e.dispatchEvent(new Event('input',{bubbles:true}));return 'filled '+e.value.length})()`));
  } else if (cmd === "wait") {
    const until = Date.now() + (Number(args[1] || 30) * 1000);
    let ok = false;
    while (Date.now() < until) {
      if ((await evalIn(c, "document.body.innerText")).toLowerCase().includes(args[0].toLowerCase())) { ok = true; break; }
      await new Promise((r) => setTimeout(r, 500));
    }
    console.log(ok ? "found: " + args[0] : "TIMEOUT waiting for: " + args[0]);
  } else if (cmd === "smoke") {
    await c.send("Log.enable");
    await evalIn(c, helpers);
    const out = [];
    for (const label of JSON.parse(args[0])) {
      c.events.length = 0;
      const r = await evalIn(c, `window.__cf.clickText(${JSON.stringify(label)})`);
      await new Promise((res) => setTimeout(res, 1800));
      const errs = c.events
        .filter((e) => e.method === "Runtime.exceptionThrown" || (e.method === "Runtime.consoleAPICalled" && e.params.type === "error") || (e.method === "Log.entryAdded" && e.params.entry.level === "error"))
        .map((e) => JSON.stringify(e.params).slice(0, 240));
      const txt = await evalIn(c, "document.body.innerText");
      out.push({ label, click: r.slice(0, 30), chars: txt.length, errors: errs });
    }
    console.log(JSON.stringify(out, null, 1));
  } else console.log("unknown command");
} finally {
  c.close();
}
