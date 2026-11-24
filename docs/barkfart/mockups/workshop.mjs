// The Workshop board: a page of its own for plugins (scripts, effects,
// modifiers): browse them, import, see what each does and where it's used,
// why one is switched off, peek at its code, remove it. The settings of one
// use (a script's run, an effect on a figure) stay in Properties, next to the
// stage. Built on the Settings board's page (run `node boards.mjs` first);
// writes project/Workshop.dc.html and preview/workshop.html, at the user's
// iPad size in landscape (1180 x 820).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const W = 1180, H = 820;
const C = { ink: '#222428', ink2: '#6B7078', blue: '#1577D6', red: '#CC1736', line: '#DADDE2', field: '#EEF0F3', orange: '#D9821E' };
const base = readFileSync(join(HERE, 'preview/settings.html'), 'utf8');
const head = base.slice(0, base.indexOf('<body'));

const svg = (d, size, color, sw = 2) => `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="${color}" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON = {
  script: '<path d="M8.5 5.5C6.5 5.5 6.5 7 6.5 9s-1.5 3-2.5 3c1 0 2.5 1 2.5 3s0 3.5 2 3.5M15.5 5.5c2 0 2 1.5 2 3.5s1.5 3 2.5 3c-1 0-2.5 1-2.5 3s0 3.5-2 3.5"/>',
  effect: '<path d="M12 3.5l1.9 5.2 5.6.3-4.4 3.5 1.5 5.4L12 14.8l-4.6 3.1 1.5-5.4-4.4-3.5 5.6-.3z"/>',
  modifier: '<path d="M3 12l2.2-4 2.6 8 2.6-9 2.6 10 2.6-9 2.6 8L20.5 12"/>',
  all: '<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><rect x="13" y="13" width="7" height="7" rx="1.5"/>',
  off: '<circle cx="12" cy="12" r="8"/><path d="M6.5 17.5l11-11"/>',
  back: '<path d="m15 5-7 7 7 7"/>',
  chev: '<path d="m9 6 6 6-6 6"/>',
};

// ---- the shelf: kinds of plugin ----
const kinds = [['all', 'Everything', 5, true], ['script', 'Scripts', 3], ['effect', 'Effects', 1], ['modifier', 'Modifiers', 1], ['off', 'Switched off', 1]];
let shelf = '';
for (const [k, label, n, on] of kinds) {
  if (k === 'off') shelf += '<hr>';
  shelf += `<button type="button" class="kind${on ? ' on' : ''}">${svg(ICON[k], 20, on ? C.blue : k === 'off' ? C.orange : '#4A4F57', 2)}<span>${label}</span><b>${n}</b></button>`;
}
shelf += `<p class="phint" style="margin:14px 6px 0">A plugin is a .luau file. It runs inside Barkfart only: it can't reach the internet or your other files.</p>`;

// ---- the cards ----
const cards = [
  { k: 'script', name: 'Sway', by: 'Barkfart', v: '1.0.0', d: 'The figure leans into its movement and settles when it stops.', used: 'In this animation: Hero, frames 3 to 11', sel: true },
  { k: 'script', name: 'Breathe', by: 'Barkfart', v: '1.0.0', d: 'A gentle rise and fall, so a figure standing still looks alive.', used: 'Not used in this animation' },
  { k: 'script', name: 'Pulse', by: 'Barkfart', v: '1.0.0', d: 'Makes a value rise and fall: point it at a glow’s strength or a segment’s length.', used: 'In this animation: Hero › Glow strength' },
  { k: 'effect', name: 'Ripple', by: 'yeah', v: '1.2.0', d: 'Waves run across the layer, like heat over a road.', used: 'In this animation: Rival' },
  { k: 'modifier', name: 'Stutter', by: 'yeah', v: '1.0.0', d: 'Holds a value for a few frames, then jumps on.', off: 'Switched off: it took too long on frame 14.' },
];
const kindName = { script: 'Script', effect: 'Effect', modifier: 'Modifier' };
let grid = '';
for (const c of cards) {
  grid += `<button type="button" class="wcard${c.sel ? ' sel' : ''}">
    <div class="wh">${svg(ICON[c.k], 22, c.off ? '#9AA0A8' : C.blue, 2)}<b>${c.name}</b><span class="wk">${kindName[c.k]}</span></div>
    <div class="wby">by ${c.by} · ${c.v}</div>
    <p class="wd">${c.d}</p>
    ${c.off ? `<div class="woff">${svg(ICON.off, 14, C.orange, 2.2)}${c.off}</div>` : `<div class="wused${c.used.startsWith('Not') ? ' none' : ''}">${c.used}</div>`}
  </button>`;
}
grid += `<button type="button" class="wcard wnew">${svg('<path d="M12 5v14M5 12h14"/>', 26, C.blue, 2.2)}<b>Import a plugin…</b><span>A .luau file: a script, an effect or a modifier</span></button>`;

// ---- the picked plugin ----
const code = [
  ['c', '-- Sway: a figure leans into its movement and settles'],
  ['k', 'return', ' bf.script {'],
  ['', '  id = "barkfart.sway", name = "Sway", version = "1.0.0",'],
  ['', '  targets = { figure = bf.figure() },'],
  ['', '  props = { lean = bf.number { default = 0.6 }, … },'],
  ['k', '  step = function', '(s, p, t, scene)'],
  ['', '    local pull = -scene.figure.velocity.x * 0.02 * p.lean'],
  ['', '    s.speed = (s.speed + (pull - s.angle) * p.stiffness) * 0.85'],
];
const codeHtml = code.map(([t, a, b]) => t === 'c' ? `<span class="cc">${a}</span>` : t === 'k' ? `<span class="ck">${a}</span>${b}` : a).join('\n');
const row = (l, r, chev = false) => `<div class="prow"><span class="lab">${l}</span><b>${r}${chev ? ' ' + svg(ICON.chev, 14, C.ink2, 2.2) : ''}</b></div>`;
const title = (t) => `<div class="sect"><h2 class="title">${t}</h2></div>`;
const detail = `<section class="wdet" aria-label="Sway">
  <div class="wdin">
    <div class="wdh">${svg(ICON.script, 26, C.blue, 2)}<div><b>Sway</b><span>Script by Barkfart · 1.0.0</span></div></div>
    <p class="phint" style="font-size:13px;color:${C.ink}">The figure leans into its movement and settles when it stops.</p>
    ${title('It moves')}
    ${row('Figure', 'a stick figure')}
    ${title('Its settings')}
    ${row('Lean', '0.60 (0 to 2)')}${row('Stiffness', '0.35 (0.05 to 1)')}
    ${title('In this animation')}
    ${row('Hero, frames 3 to 11', 'Show', true)}
    ${title('The code')}
    <pre class="code">${codeHtml}</pre>
    <div class="pair"><button type="button" class="wb">Share the file…</button><button type="button" class="wb pri">Add to this animation</button></div>
  </div>
  <div class="pfoot"><button type="button" class="rall">Remove from Barkfart</button></div>
</section>`;

const css = `
.wshelf{position:absolute;left:0;top:40px;bottom:0;width:212px;padding:18px 12px;box-sizing:border-box;border-right:1px solid ${C.line};background:#F7F8FA}
.kind{display:flex;align-items:center;gap:10px;width:100%;height:40px;padding:0 10px!important;border-radius:10px;font-size:15px!important;color:${C.ink}!important;text-align:left}
.kind span{flex:1}.kind b{font-size:13px;font-weight:600;color:${C.ink2}}
.kind.on{background:#E4EEFA}.kind.on span{color:${C.blue};font-weight:700}
.wshelf hr{border:0;border-top:1px solid ${C.line};margin:8px 4px}
.wgrid{position:absolute;left:212px;top:40px;bottom:0;right:392px;padding:18px 20px;box-sizing:border-box;display:grid;grid-template-columns:1fr 1fr;gap:14px;align-content:start;overflow:hidden}
.wcard{box-sizing:border-box;display:flex;flex-direction:column;align-items:stretch;gap:4px;text-align:left;background:#fff;border-radius:14px;padding:12px 14px!important;box-shadow:0 0 0 1px rgba(0,0,0,.06),0 2px 8px rgba(16,20,26,.07);min-height:142px}
.wcard.sel{box-shadow:0 0 0 2px ${C.blue},0 2px 8px rgba(16,20,26,.07)}
.wh{display:flex;align-items:center;gap:8px}.wh b{flex:1;font-size:16px;font-weight:700;color:${C.ink}}
.wk{font-size:12px;font-weight:600;color:${C.ink2};background:${C.field};border-radius:6px;padding:2px 7px}
.wby{font-size:12px;color:${C.ink2}}
.wd{margin:2px 0 0;font-size:13px;line-height:18px;color:${C.ink};flex:1}
.wused{font-size:12px;font-weight:600;color:${C.blue}}.wused.none{color:${C.ink2};font-weight:400}
.woff{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:600;color:${C.orange}}
.wnew{align-items:center;justify-content:center;gap:6px;background:transparent;box-shadow:none;border:1.5px dashed #C3C8CF}
.wnew b{font-size:15px;color:${C.blue}}.wnew span{font-size:12px;color:${C.ink2}}
.wdet{position:absolute;right:16px;top:56px;bottom:16px;width:360px;background:#fff;border-radius:16px;box-shadow:0 0 0 1px rgba(0,0,0,.06),0 6px 24px rgba(16,20,26,.12);overflow:hidden}
.wdin{position:absolute;inset:12px 16px 58px 16px;overflow:hidden}
.wdh{display:flex;align-items:center;gap:10px;margin-bottom:4px}.wdh b{display:block;font-size:17px;font-weight:700}.wdh span{font-size:12px;color:${C.ink2}}
.prow b{display:flex;align-items:center;gap:2px;font-weight:600;font-size:14px}
.code{margin:4px 0 10px;padding:10px 12px;background:#F4F5F7;border-radius:10px;font:12px/17px ui-monospace,'DejaVu Sans Mono',monospace;color:#3A3F47;white-space:pre;overflow:hidden}
.cc{color:#8A9099}.ck{color:${C.red};font-weight:600}
.pair{display:grid;grid-template-columns:1fr 1.3fr;gap:8px}
.pfoot{position:absolute;left:0;right:0;bottom:0;padding:9px 16px;border-top:1px solid #E6E8EB;background:#fff}
.rall{width:100%;height:36px;border-radius:10px;background:#FDECEE!important;color:${C.red}!important;font-weight:600!important}
`;

const top = `<header class="top" style="height:40px"><nav class="words" style="left:12px"><button type="button" class="tw" style="display:flex;align-items:center;gap:2px">${svg(ICON.back, 16, C.blue, 2.4)}Back</button></nav><div class="name">Workshop</div><nav class="words" style="right:12px"><button type="button" class="tw">Import…</button></nav></header>`;
const body = `<div class="bf scr" style="position:relative;width:${W}px;height:${H}px;overflow:hidden">${top}<nav class="wshelf" aria-label="Kinds">${shelf}</nav><main class="wgrid">${grid}</main>${detail}</div>`;
const html = head.replace('</style>', css + '</style>') + `<body style="background:#888">${body}</body></html>`;
mkdirSync(join(HERE, 'preview'), { recursive: true });
mkdirSync(join(HERE, 'project'), { recursive: true });
writeFileSync(join(HERE, 'preview/workshop.html'), html);
writeFileSync(join(HERE, 'project/Workshop.dc.html'), html);
console.log('wrote preview/workshop.html');
