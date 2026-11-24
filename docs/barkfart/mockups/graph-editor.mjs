// Graph board (D34): the timeline switched to Curves. The selected figure's
// animated values as curves over the same frames, keys as dots, the
// selected key's handles, and the handle types in the bottom bar.
// Built on the timeline board (run `node boards.mjs` first); writes
// project/Graph.dc.html and preview/graph.html.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const C = { ink: '#222428', ink2: '#6B7078', ink2b: '#4A4F57', blue: '#1577D6', line: '#DADDE2', rail: '#F3F4F6', field: '#EEF0F3' };
const base = readFileSync(join(HERE, 'preview/timeline.html'), 'utf8');
const effectsBoard = readFileSync(join(HERE, 'preview/effects.html'), 'utf8');

// variants: 'graph' (the Curves view), 'mods' (a value with a plugin
// modifier on it, its page in Properties), 'modsadd' (adding a modifier)
function board(variant) {
const MODS = variant !== 'graph';
// ---- layout: a shorter stage, a taller timeline ----
const TL_TOP = MODS ? 292 : 250, TL_H = 600 - TL_TOP, TB = 32, RU = 22, BAR = 32, NAME_W = 224, CELL = 46, LEFT = 8, FRAMES = 24, CUR = 11;
const GRAPH_TOP = TB + RU, GRAPH_H = TL_H - TB - RU - BAR;
let html = base
  .replace('<title>', '<title>')
  .replaceAll('height:286px', `height:${TL_TOP - 40}px`)
  .replace(/<svg viewBox="0 0 1920 1080" width="473" height="266" style="position:absolute;left:242px;top:50px;/, () => {
    const h = TL_TOP - 40 - 20, w = Math.round((h * 1920) / 1080);
    return `<svg viewBox="0 0 1920 1080" width="${w}" height="${h}" style="position:absolute;left:${Math.round(64 + (900 - 64 - w) / 2)}px;top:50px;`;
  });

// ---- the curves: the same rule as the engine (auto-clamped handles) ----
// keys: [frame, value]; hold: frames whose segment holds (a whole-figure pose)
const HOLD = new Set([9]);
const CH = [
  { name: 'Right hand angle', unit: '°', colour: '#D9482B', on: true, sel: true, keys: [[1, 20], [5, 72], [9, 138], [15, 58], [20, 16]] },
  { name: 'Right elbow angle', unit: '°', colour: '#2E8B57', on: true, keys: [[1, -8], [5, 34], [9, 96], [15, 40], [20, -8]] },
  // not in degrees: drawn scaled to fit (dashed), the scale at the left stays the selected one's
  { name: 'Right hand length', unit: '', colour: '#C28A00', on: true, fit: [92, 132], keys: [[1, 100], [5, 100], [9, 128], [15, 128], [20, 100]] },
  { name: 'Main joint x', unit: '', colour: '#2D63E0', on: false, keys: [[1, 0], [5, -20], [9, 35], [15, 35], [20, 0]] },
  { name: 'Main joint y', unit: '', colour: '#9A6BD0', on: false, keys: [[1, 0], [5, 4], [9, -6], [15, -6], [20, 0]] },
];
// colours have no single height: a band of the colour over time, keys as dots
const BAND = { name: 'Shirt colour', keys: [[1, '#9B1C2E'], [9, '#9B1C2E'], [15, '#2160A8'], [20, '#2160A8']] };
const fitted = (c) => (c.fit ? c.keys.map(([f, v]) => [f, VMIN + ((v - c.fit[0]) / (c.fit[1] - c.fit[0])) * (VMAX - VMIN)]) : c.keys);
function slope(k, i) {
  if (i === 0 || i === k.length - 1) return 0;
  const dl = (k[i][1] - k[i - 1][1]) / (k[i][0] - k[i - 1][0]);
  const dr = (k[i + 1][1] - k[i][1]) / (k[i + 1][0] - k[i][0]);
  if (dl * dr <= 0) return 0;
  const m = (k[i + 1][1] - k[i - 1][1]) / (k[i + 1][0] - k[i - 1][0]);
  const cap = 3 * Math.min(Math.abs(dl), Math.abs(dr));
  return Math.max(-cap, Math.min(cap, m));
}
// the keyed value at any frame (1-based), by the same rule as curvePath
function valueAt(k, f) {
  if (f <= k[0][0]) return k[0][1];
  for (let i = 0; i + 1 < k.length; i++) {
    const [f0, v0] = k[i], [f1, v1] = k[i + 1];
    if (f > f1) continue;
    if (HOLD.has(f0)) return f < f1 ? v0 : v1;
    const h = f1 - f0, u = (f - f0) / h, m0 = slope(k, i) * h, m1 = slope(k, i + 1) * h;
    const u2 = u * u, u3 = u2 * u;
    return (2 * u3 - 3 * u2 + 1) * v0 + (u3 - 2 * u2 + u) * m0 + (-2 * u3 + 3 * u2) * v1 + (u3 - u2) * m1;
  }
  return k[k.length - 1][1];
}
// "Stutter" (a plugin modifier): holds every 3rd frame (engine frames from 0)
const stutter = (k, f) => valueAt(k, 1 + Math.floor((f - 1) / 3) * 3);
const X = (f) => NAME_W + LEFT + (f - 1) * CELL + CELL / 2;
const VMIN = -30, VMAX = 160, PAD = 22;
const Y = (v) => GRAPH_TOP + PAD + ((VMAX - v) / (VMAX - VMIN)) * (GRAPH_H - 2 * PAD);
const r1 = (v) => Math.round(v * 10) / 10;
function curvePath(k) {
  let d = `M${r1(X(k[0][0]))} ${r1(Y(k[0][1]))}`;
  for (let i = 0; i + 1 < k.length; i++) {
    const [f0, v0] = k[i], [f1, v1] = k[i + 1];
    if (HOLD.has(f0)) {
      d += `H${r1(X(f1))}V${r1(Y(v1))}`;
      continue;
    }
    const h = f1 - f0, m0 = slope(k, i), m1 = slope(k, i + 1);
    const c1 = [X(f0 + h / 3), Y(v0 + (m0 * h) / 3)], c2 = [X(f1 - h / 3), Y(v1 - (m1 * h) / 3)];
    d += `C${r1(c1[0])} ${r1(c1[1])} ${r1(c2[0])} ${r1(c2[1])} ${r1(X(f1))} ${r1(Y(v1))}`;
  }
  return d;
}

// ---- the timeline section ----
const eye = (on) => `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${on ? '#9AA0A8' : '#D5D8DD'}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M2.5 12s3.5-6.5 9.5-6.5 9.5 6.5 9.5 6.5-3.5 6.5-9.5 6.5S2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>${on ? '' : '<path d="M4 20 20 4"/>'}</svg>`;
const tb = base.slice(base.indexOf('<div class="tb"'), base.indexOf('<div class="names"'));
// the toolbar gains a Curves toggle (pressed) after FX
const curvesBtn = `<button type="button" class="ib on" aria-label="Curves" aria-pressed="true"><svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="${C.blue}" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M3 18c4 0 5-12 9-12s5 12 9 12"/><circle cx="12" cy="6" r="1.6" fill="${C.blue}"/></svg></button>`;
const tbCurves = tb.replace('aria-label="Show effects while editing" aria-pressed="true">FX</button>', 'aria-label="Show effects while editing" aria-pressed="true">FX</button>' + curvesBtn);

let s = `<section class="tl" aria-label="Timeline, showing curves" style="top:${TL_TOP}px;height:${TL_H}px">${tbCurves}`;
// names: the figure, then its animated values with a colour and an eye
s += `<div class="names" style="top:${TB}px;width:${NAME_W}px;height:${TL_H - TB - BAR}px">`;
s += `<div class="rh" style="height:${RU}px"></div>`;
s += `<div class="nrow" style="height:32px"><div class="nin" style="padding-left:12px"><svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="${C.ink2b}" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="4.6" r="2.4"/><path d="M12 7v7M12 14l-4 6M12 14l4 6M7 10h10"/></svg><span style="font-weight:600">Hero</span><span class="sub">6 animated</span></div></div>`;
for (const c of CH) {
  // a value with modifiers: a small wave by its name
  const mod = MODS && c.sel ? `<svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="${C.blue}" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-label="Has modifiers" style="margin-left:2px"><path d="M3 15h4V9h5v6h5V9h4"/></svg>` : '';
  s += `<div class="nrow${c.sel ? ' on' : ''}" style="height:30px;font-size:13px"><div class="nin" style="padding-left:16px"><span style="width:10px;height:10px;border-radius:50%;background:${c.on ? c.colour : '#fff'};box-shadow:inset 0 0 0 2px ${c.colour};flex:none"></span><span style="${c.on ? '' : `color:${C.ink2}`}">${c.name}</span>${mod}<button type="button" class="eye" aria-label="${c.on ? 'Hide' : 'Show'} ${c.name}">${eye(c.on)}</button></div></div>`;
}
s += `<div class="nrow" style="height:30px;font-size:13px"><div class="nin" style="padding-left:16px"><span style="width:10px;height:10px;border-radius:3px;background:linear-gradient(90deg,#9B1C2E,#2160A8);flex:none"></span><span>${BAND.name}</span><button type="button" class="eye" aria-label="Hide ${BAND.name}">${eye(true)}</button></div></div>`;
// importing a value: the figure's numbers and colours not animated yet
s += `<div class="nrow" style="height:30px;font-size:13px;box-shadow:none"><button type="button" class="nin" style="padding-left:14px;color:${C.blue};font-weight:600" aria-expanded="true"><svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>Add a value…</button></div>`;
s += `</div>`;
// ruler (frames), playhead pill
s += `<div class="ruler" style="left:${NAME_W}px;top:${TB}px;height:${RU}px;width:${1344 - NAME_W}px">`;
for (let f = 1; f <= FRAMES; f++) {
  const x = LEFT + (f - 1) * CELL + CELL / 2;
  const big = f === 1 || f % 3 === 0;
  s += `<span class="tk${big ? ' big' : ''}" style="left:${x}px"></span>`;
  if (big && f !== CUR) s += `<span class="rn" style="left:${x - 20}px">${f}</span>`;
}
s += `<span class="pill" style="left:${LEFT + (CUR - 1) * CELL + CELL / 2 - 14}px;top:2px">${CUR}</span></div>`;
// the graph
const GW = 1344 - NAME_W;
let g = `<svg width="${GW}" height="${GRAPH_H}" viewBox="${NAME_W} ${GRAPH_TOP} ${GW} ${GRAPH_H}" style="position:absolute;left:${NAME_W}px;top:${GRAPH_TOP}px" aria-label="Curves of Hero's animated values">`;
// value lines, labelled at the left
for (const v of [150, 90, 30, -30 + 0]) {
  const y = Y(v);
  g += `<line x1="${NAME_W}" x2="${1344}" y1="${r1(y)}" y2="${r1(y)}" stroke="#EEF0F2" stroke-width="1"/>`;
  g += `<text x="${NAME_W + 6}" y="${r1(y - 4)}" font-family="Open Sans, sans-serif" font-size="10.5" fill="#9AA0A8">${v}°</text>`;
}
// frame columns, faint; the held stretch shaded
g += `<rect x="${X(9)}" y="${GRAPH_TOP}" width="${X(15) - X(9)}" height="${GRAPH_H}" fill="#F6F7F9"/>`;
g += `<text x="${(X(9) + X(15)) / 2}" y="${GRAPH_TOP + 14}" text-anchor="middle" font-family="Open Sans, sans-serif" font-size="11" fill="#9AA0A8">holds</text>`;
// the playhead
g += `<line x1="${X(CUR)}" x2="${X(CUR)}" y1="${GRAPH_TOP}" y2="${GRAPH_TOP + GRAPH_H}" stroke="#8DB8E8" stroke-width="1.5"/>`;
// curves: hidden ones not drawn; the selected one on top and stronger
for (const c of [...CH].filter((c) => c.on).sort((a, b) => (a.sel ? 1 : 0) - (b.sel ? 1 : 0))) {
  if (MODS && c.sel) {
    // the keys' own curve, faint: the keys stay on it, where they're dragged
    g += `<path d="${curvePath(fitted(c))}" fill="none" stroke="${c.colour}" stroke-width="1.6" stroke-opacity=".38" stroke-dasharray="6 5" stroke-linecap="round"/>`;
    // what the modifier makes of it, solid
    let d = '';
    for (let f = 1; f <= FRAMES; f += 0.05) d += `${d ? 'L' : 'M'}${r1(X(f))} ${r1(Y(stutter(c.keys, f)))}`;
    g += `<path d="${d}" fill="none" stroke="${c.colour}" stroke-width="2.6" stroke-linejoin="round"/>`;
    continue;
  }
  g += `<path d="${curvePath(fitted(c))}" fill="none" stroke="${c.colour}" stroke-width="${c.sel ? 2.6 : 1.8}" stroke-opacity="${c.sel ? 1 : 0.55}"${c.fit ? ' stroke-dasharray="7 5"' : ''} stroke-linecap="round" stroke-linejoin="round"/>`;
}
if (MODS) g += `<text x="1334" y="${GRAPH_TOP + 30}" text-anchor="end" font-family="Open Sans, sans-serif" font-size="11" fill="#9AA0A8">faint: the keys before Stutter</text>`;
g += `<text x="1334" y="${GRAPH_TOP + 14}" text-anchor="end" font-family="Open Sans, sans-serif" font-size="11" fill="#9AA0A8">dashed: scaled to fit</text>`;
// the colour band along the foot of the graph
{
  const by = GRAPH_TOP + GRAPH_H - 22, bx0 = X(1), bx1 = X(FRAMES);
  g += `<defs><linearGradient id="band" x1="${bx0}" x2="${bx1}" gradientUnits="userSpaceOnUse">${BAND.keys.map(([f, c]) => `<stop offset="${r1(((X(f) - bx0) / (bx1 - bx0)) * 100)}%" stop-color="${c}"/>`).join('')}</linearGradient></defs>`;
  g += `<rect x="${bx0}" y="${by}" width="${bx1 - bx0}" height="12" rx="6" fill="url(#band)"/>`;
  for (const [f] of BAND.keys) g += `<circle cx="${r1(X(f))}" cy="${by + 6}" r="4.5" fill="#fff" stroke="#4A4F57" stroke-width="1.6"/>`;
}
// keys: dots; the selected key (frame 5 of the right hand) with its handles
const SEL = { ch: 0, i: 1 };
for (const [ci, c] of CH.entries()) {
  if (!c.on) continue;
  for (const [i, [f, v]] of fitted(c).entries()) {
    if (!MODS && ci === SEL.ch && i === SEL.i) continue;
    g += `<circle cx="${r1(X(f))}" cy="${r1(Y(v))}" r="${c.sel ? 5 : 4}" fill="#fff" stroke="${c.colour}" stroke-opacity="${c.sel ? 1 : 0.6}" stroke-width="2"/>`;
  }
}
if (!MODS) {
  const k = CH[SEL.ch].keys, [f, v] = k[SEL.i];
  // dragged a little steeper than automatic: Aligned, so both sides stay in line
  const m = slope(k, SEL.i) * 1.35, hl = (k[SEL.i][0] - k[SEL.i - 1][0]) / 3, hr = (k[SEL.i + 1][0] - k[SEL.i][0]) / 3;
  const L = [X(f - hl), Y(v - m * hl)], R = [X(f + hr), Y(v + m * hr)];
  g += `<line x1="${r1(L[0])}" y1="${r1(L[1])}" x2="${r1(R[0])}" y2="${r1(R[1])}" stroke="${C.ink}" stroke-width="1.4"/>`;
  for (const p of [L, R]) g += `<circle cx="${r1(p[0])}" cy="${r1(p[1])}" r="5" fill="#fff" stroke="${C.ink}" stroke-width="1.6"/>`;
  g += `<circle cx="${r1(X(f))}" cy="${r1(Y(v))}" r="6.5" fill="${C.blue}" stroke="#fff" stroke-width="2"/>`;
  // the readout, as while drawing a segment
  const tx = X(f) + 14, ty = Y(v) + 14;
  g += `<g transform="translate(${r1(tx)} ${r1(ty)})"><rect width="104" height="22" rx="11" fill="#222428" fill-opacity=".86"/><text x="52" y="15" text-anchor="middle" font-family="Open Sans, sans-serif" font-weight="600" font-size="11.5" fill="#fff">Frame ${f}, ${v}°</text></g>`;
}
g += `</svg>`;
s += g;
// bottom bar: in Curves, the selected key's handles and Fit
const hbtn = (label, on) => `<button type="button" role="radio" aria-checked="${on}" style="height:28px;padding:0 11px;border-radius:7px;font-size:13px;font-weight:${on ? 700 : 500};color:${on ? C.blue : C.ink};${on ? 'background:#fff;box-shadow:0 1px 2px rgba(0,0,0,.08),0 0 0 1px #DADDE2' : ''}">${label}</button>`;
s += `<div class="bar" style="height:${BAR}px"><span style="font-size:13px;color:${C.ink2};margin-right:8px">Handles</span>`;
s += `<span role="radiogroup" aria-label="Handles of the selected key" style="display:flex;gap:2px;background:${C.field};border-radius:9px;padding:2px;${MODS ? 'opacity:.45' : ''}">${hbtn('Settle', false)}${hbtn('Swing', false)}${hbtn('Sharp', false)}${hbtn('Free', false)}${hbtn('Aligned', !MODS)}</span>`;
s += `<span class="bw"><button type="button">Fit</button><button type="button">New pose</button></span></div>`;
const mi = (label, sub) => `<button type="button" class="mi" style="height:34px;font-size:14px">${label}${sub ? `<span style="margin-left:auto;font-size:12px;color:${C.ink2}">${sub}</span>` : ''}</button>`;
const mh = (t) => `<div style="padding:8px 10px 2px;font-size:12px;font-weight:700;color:#CC1736">${t}</div>`;
s += `</section>`;
// the list opens upward from its row, over the stage (page coordinates)
const ADD_ROW_Y = TL_TOP + TB + RU + 32 + 30 * (CH.length + 1);
const MENU_H = 296;
const menu = MODS ? '' : `<div class="menu" role="menu" aria-label="Add a value" style="left:150px;top:${ADD_ROW_Y - MENU_H - 4}px;width:240px;height:${MENU_H}px">${mh('Right hand')}${mi('Thickness', '24')}${mi('Scale', '1.00')}${mi('Outline colour', '')}${mh('Hero')}${mi('Opacity', '100%')}${mi('Glow amount', '0.70')}<div style="padding:6px 10px 4px;font-size:12px;color:${C.ink2};line-height:1.35">Keyed on this frame, so its curve starts here. Choices like shape or blend stay on their pages.</div></div>`;

// Properties, in Curves: the selected value's page
const pm = (d) => `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="${d}"/></svg>`;
const step = (label, value) => `<div class="prow"><span class="lab">${label}</span><span class="step"><button type="button" class="sb" aria-label="Less">${pm('M6 12h12')}</button><b>${value}</b><button type="button" class="sb" aria-label="More">${pm('M12 6v12M6 12h12')}</button></span></div>`;
const plus = `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="${C.blue}" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>`;
const eyeOn = eye(true);
let panel = '';
if (variant === 'mods') {
  panel = `<section class="panel" aria-label="Right hand angle" style="left:900px;top:48px;width:372px;height:${TL_TOP - 56}px">`;
  panel += `<div class="prow" style="height:30px"><b style="font-size:15px">Right hand angle</b><span style="font-size:12px;color:${C.ink2}">5 keys</span></div>`;
  panel += `<div class="sect s2"><h2 class="title">Modifiers</h2><button type="button" class="tadd" aria-label="Add a modifier">${plus}</button></div>`;
  panel += `<div class="fxrow"><span class="fxn">Stutter</span><span class="fxk">by yeah</span><button type="button" class="eye" aria-label="Hide Stutter">${eyeOn}</button></div>`;
  panel += `<div style="padding:0 2px">${step('Every (frames)', '3')}</div>`;
  panel += `<div class="fxrow miss" style="margin-top:6px"><span class="fxn">Missing: Wobble 1.2</span><span class="fxk">by yeah</span><button type="button" class="eye" aria-label="Hide Wobble">${eyeOn}</button></div>`;
  panel += `<p class="phint" style="margin-top:6px">Applied top to bottom, after the keys. Wobble does nothing until it's imported again; its settings are kept.</p>`;
  panel += `</section>`;
} else if (variant === 'modsadd') {
  // the effects browser's look: back, Import…, search, tiles of three
  const mvTiles = (() => {
    const i = effectsBoard.indexOf('<h2 class="title gt">Movement</h2>');
    const j = effectsBoard.indexOf('</div>', effectsBoard.indexOf('<div class="tiles">', i));
    return effectsBoard.slice(effectsBoard.indexOf('<div class="tiles">', i), j + 6);
  })();
  const ptile = (name, path) => `<button type="button" class="tile"><span class="mv"><svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="${C.blue}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${path}"/></svg></span><span>${name}</span><span class="ptag">by yeah</span></button>`;
  panel = `<section class="panel" aria-label="Add a modifier" style="left:900px;top:48px;width:372px;height:500px">`;
  panel += `<div class="phead"><button type="button" class="pback"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="#1577D6" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m15 5-7 7 7 7"/></svg>Right hand angle</button><button type="button" class="pimp">Import…</button></div>`;
  panel += `<label class="srch"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="#8A9099" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="10.5" cy="10.5" r="5.5"/><path d="m15 15 4.5 4.5"/></svg><span>Search modifiers</span></label>`;
  panel += `<h2 class="title gt">Yours</h2><div class="tiles">${ptile('Stutter', 'M3 17h4v-5h5v-5h5V4h4')}${ptile('Springy', 'M3 18c2-12 4 6 6-3s3 4 5 0 3 2 7 1')}</div>`;
  panel += `<h2 class="title gt">Built in</h2>${mvTiles}`;
  panel += `<p class="phint" style="margin-top:10px">A modifier changes this value on top of its keys. Plugins come in as files: Import… takes a .luau file or a plugin folder.</p>`;
  panel += `</section>`;
}

// swap the timeline and drop the properties panel (the curves need the room)
const tlStart = html.indexOf('<section class="tl"');
const end = html.lastIndexOf('</div></body>');
html = html.slice(0, tlStart) + s + menu + html.slice(end);
html = html.replace(/<section class="panel"[\s\S]*?<\/section>/, '');
html = html.replace('</div></body>', panel + '</div></body>');
return html;
}

mkdirSync(join(HERE, 'preview'), { recursive: true });
mkdirSync(join(HERE, 'project'), { recursive: true });
for (const [v, file, dc] of [['graph', 'graph', 'Graph'], ['mods', 'graph-mods', 'GraphModifiers'], ['modsadd', 'graph-mods-add', 'GraphAddModifier']]) {
  const html = board(v);
  writeFileSync(join(HERE, `preview/${file}.html`), html);
  writeFileSync(join(HERE, `project/${dc}.dc.html`), html);
  console.log(`wrote preview/${file}.html`);
}
