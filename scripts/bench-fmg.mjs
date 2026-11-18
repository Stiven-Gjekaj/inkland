// Measure the time of FMG generation and layer rendering in headless
// Chrome. The script uses the DevTools protocol of a Chrome that
// scripts/bench-fmg.sh starts. Usage: node scripts/bench-fmg.mjs PORT
//
// For each cell count, the script makes 1 map to warm up and 6 maps to
// measure. It reads the "Generation Pipeline" and "Layers Rendering"
// measures that FMG records, and prints the median of each.
const port = process.argv[2] || "9333";
const sleep = ms => new Promise(r => setTimeout(r, ms));

let targets;
for (let i = 0; i < 50; i++) {
  try {
    targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    if (targets.some(t => t.type === "page")) break;
  } catch {}
  await sleep(200);
}
const page = targets.find(t => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener("open", r));
let id = 0;
const pending = new Map();
ws.addEventListener("message", ev => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
});
const send = (method, params = {}) =>
  new Promise(r => {
    const k = ++id;
    pending.set(k, r);
    ws.send(JSON.stringify({ id: k, method, params }));
  });
const evaluate = async expr => {
  const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.result.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails));
  return r.result.result.value;
};

await send("Page.enable");
await send("Page.navigate", { url: "https://azgaar.github.io/Fantasy-Map-Generator/" });
// Wait until FMG shows its first map.
for (let i = 0; i < 300; i++) {
  await sleep(200);
  const ok = await evaluate("performance.getEntriesByName('Layers Rendering').length > 0").catch(() => false);
  if (ok) break;
}
const info = await evaluate("({ title: document.title, w: innerWidth, h: innerHeight, cores: navigator.hardwareConcurrency, ua: navigator.userAgent })");
console.log(JSON.stringify(info));

const bench = `
(async () => {
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  async function one() {
    performance.clearMeasures();
    document.getElementById('regenerate').click();
    for (let t = 0; t < 3000; t++) {
      await sleep(50);
      const b = [...document.querySelectorAll('.dialog-window button, .ui-dialog button')].find(x => x.offsetParent && x.textContent.trim() === 'Generate');
      // FMG asks to confirm when the map is older than 1 minute.
      if (b) b.click();
      // FMG can ignore the first click after the page loads. Click again.
      if (t > 0 && t % 100 === 0) document.getElementById('regenerate').click();
      const m = performance.getEntriesByName('Layers Rendering');
      if (m.length) {
        const g = performance.getEntriesByName('Generation Pipeline')[0];
        return { gen: g.duration, draw: m[0].duration, grid: FMG.grid.cells.i.length, pack: FMG.pack.cells.i.length };
      }
    }
    return null;
  }
  const inp = document.getElementById('pointsInput');
  inp.value = '__D__';
  inp.dispatchEvent(new Event('input', { bubbles: true }));
  inp.dispatchEvent(new Event('change', { bubbles: true }));
  await sleep(300);
  await one(); // A map to warm up. It does not count.
  const out = [];
  for (let i = 0; i < 6; i++) { out.push(await one()); await sleep(300); }
  return out;
})()`;

const med = a => { const s = [...a].sort((x, y) => x - y); return (s[2] + s[3]) / 2; };
for (const d of ["4", "5", "8", "13"]) {
  const runs = await evaluate(bench.replace("__D__", d));
  if (runs.some(r => r === null)) { console.log(`density ${d}: a map did not finish`); continue; }
  console.log(JSON.stringify({
    density: d,
    grid: runs[0].grid,
    gen: runs.map(r => Math.round(r.gen)),
    draw: runs.map(r => Math.round(r.draw)),
    pack: runs.map(r => r.pack),
    genMedian: Math.round(med(runs.map(r => r.gen))),
    drawMedian: Math.round(med(runs.map(r => r.draw))),
    packMedian: med(runs.map(r => r.pack)),
  }));
}
ws.close();
process.exit(0);
