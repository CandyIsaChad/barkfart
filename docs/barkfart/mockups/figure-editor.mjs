// Figure editor board: shape shelf, Parts list (front to back), shape-coded handles, size reference, Shape page.
// Writes project/FigureEditor.dc.html and preview/figureeditor.html.
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));

const C = {
  ink: '#222428', ink2: '#6B7078', blue: '#1577D6', red: '#CC1736',
  hRed: '#D91A3A', hPurple: '#A534D0', hBlue: '#2D63E0', hRoot: '#2A2B30',
  rail: '#F3F4F6', line: '#DADDE2', field: '#EEF0F3', hint: '#FFD98C',
  tween: '#1E9E55', hold: '#8A9099',
};

// ---- poses (as rounds 3 and 4) ----
const COLS = { A: { skin: '#EDB98A', shirt: '#9B1C2E', pants: '#2A2B30' }, B: { skin: '#C98B62', shirt: '#2160A8', pants: '#2A2B30' } };
const PARENT = { neck: 'hip', head: 'neck', le: 'neck', lh: 'le', re: 'neck', rh: 're', lk: 'hip', lf: 'lk', rk: 'hip', rf: 'rk' };
const ORDER = ['neck', 'head', 'le', 'lh', 're', 'rh', 'lk', 'lf', 'rk', 'rf'];
const BODY_JOINTS = ['hip', 'neck', 'le', 'lh', 're', 'rh', 'lk', 'lf', 'rk', 'rf'];
const POSES = {
  A: [
    { hip: [700, 600], neck: [700, 400], head: [700, 330], le: [650, 480], lh: [640, 560], re: [750, 480], rh: [760, 560], lk: [660, 750], lf: [640, 900], rk: [740, 750], rf: [760, 900] },
    { hip: [690, 610], neck: [670, 410], head: [655, 343], le: [620, 470], lh: [660, 520], re: [610, 460], rh: [560, 420], lk: [720, 760], lf: [760, 900], rk: [640, 760], rf: [600, 900] },
    { hip: [730, 605], neck: [780, 410], head: [805, 345], le: [740, 475], lh: [780, 525], re: [870, 412], rh: [965, 400], lk: [790, 750], lf: [830, 900], rk: [680, 760], rf: [610, 900] },
    { hip: [730, 605], neck: [745, 405], head: [760, 338], le: [705, 478], lh: [735, 540], re: [835, 440], rh: [875, 500], lk: [790, 750], lf: [830, 900], rk: [680, 760], rf: [610, 900] },
  ],
  B: [
    { hip: [1080, 600], neck: [1080, 400], head: [1080, 330], le: [1030, 480], lh: [1020, 560], re: [1130, 480], rh: [1140, 560], lk: [1040, 750], lf: [1020, 900], rk: [1120, 750], rf: [1140, 900] },
    { hip: [1095, 600], neck: [1120, 405], head: [1145, 343], le: [1070, 470], lh: [1050, 540], re: [1170, 460], rh: [1210, 520], lk: [1060, 750], lf: [1030, 900], rk: [1140, 750], rf: [1160, 900] },
    { hip: [1110, 600], neck: [1150, 410], head: [1180, 352], le: [1110, 470], lh: [1090, 550], re: [1200, 470], rh: [1240, 540], lk: [1080, 750], lf: [1050, 900], rk: [1160, 750], rf: [1180, 900] },
  ],
};
const SEGS = [['lk', 'hip', 'pants', 34], ['lf', 'lk', 'skin', 30], ['le', 'neck', 'shirt', 30], ['lh', 'le', 'skin', 28], ['neck', 'hip', 'shirt', 40], ['rk', 'hip', 'pants', 34], ['rf', 'rk', 'skin', 30], ['re', 'neck', 'shirt', 30], ['rh', 're', 'skin', 28]];
const HEAD_R = 56, HEAD_OFF = 70;

// ---- the timeline: one row per layer, keys = [frame, value, 'tween' | 'hold'] ----
const CELLS = 24, CUR = 9;
const LAYERS = {
  hero: [[1, 0, 'tween'], [5, 1, 'tween'], [9, 2, 'hold'], [15, 3, 'tween'], [20, 0, 'hold']], // pose index, A
  head: [[1, 0, 'hold'], [6, -10, 'tween'], [9, 14, 'hold'], [16, 0, 'hold']], // tilt in degrees
  rival: [[1, 0, 'hold'], [10, 1, 'tween'], [12, 2, 'hold'], [18, 2, 'tween'], [22, 0, 'hold']], // pose index, B
};
const SOUND = { from: 9, to: 12, name: 'Punch' };

const r1 = (v) => Math.round(v * 10) / 10;
const ease = (t) => 1 - (1 - t) * (1 - t);
const ang = (p, a, b) => Math.atan2(p[b][1] - p[a][1], p[b][0] - p[a][0]);
const wrap = (d) => { while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI; return d; };
function tween(pa, pb, t) {
  const out = { hip: [pa.hip[0] + (pb.hip[0] - pa.hip[0]) * t, pa.hip[1] + (pb.hip[1] - pa.hip[1]) * t] };
  const absA = {}, absB = {}, absT = {};
  for (const j of ORDER) {
    const par = PARENT[j];
    const aA = ang(pa, par, j), aB = ang(pb, par, j);
    absA[j] = aA; absB[j] = aB;
    let a;
    if (par === 'hip') a = aA + wrap(aB - aA) * t;
    else { const rA = wrap(aA - absA[par]), rB = wrap(aB - absB[par]); a = absT[par] + rA + wrap(rB - rA) * t; }
    absT[j] = a;
    const lA = Math.hypot(pa[j][0] - pa[par][0], pa[j][1] - pa[par][1]), lB = Math.hypot(pb[j][0] - pb[par][0], pb[j][1] - pb[par][1]);
    const l = lA + (lB - lA) * t;
    out[j] = [out[par][0] + Math.cos(a) * l, out[par][1] + Math.sin(a) * l];
  }
  return out;
}
// where a frame sits on a row: the key before it, the next key, and how far along (null = holding)
function locate(keys, f) {
  let i = 0;
  while (i + 1 < keys.length && keys[i + 1][0] <= f) i++;
  const k = keys[i], n = keys[i + 1];
  if (k[2] === 'tween' && n && f > k[0]) return { k, n, t: ease((f - k[0]) / (n[0] - k[0])) };
  return { k, n: null, t: 0 };
}
const heroAt = (f) => { const { k, n, t } = locate(LAYERS.hero, f); return n ? tween(POSES.A[k[1]], POSES.A[n[1]], t) : POSES.A[k[1]]; };
const rivalAt = (f) => { const { k, n, t } = locate(LAYERS.rival, f); return n ? tween(POSES.B[k[1]], POSES.B[n[1]], t) : POSES.B[k[1]]; };
const tiltAt = (f) => { const { k, n, t } = locate(LAYERS.head, f); return n ? k[1] + (n[1] - k[1]) * t : k[1]; };
// the head is its own figure, joined at the neck: it follows the neck and adds its own tilt
function headPos(p, tilt) {
  const a = ang(p, 'hip', 'neck') + tilt * Math.PI / 180;
  return [p.neck[0] + Math.cos(a) * HEAD_OFF, p.neck[1] + Math.sin(a) * HEAD_OFF];
}

function figure(p, cols, { tint, op, head } = {}) {
  let s = `<g${op != null ? ` opacity="${op}"` : ''} stroke-linecap="round">`;
  for (const [c, par, col, t] of SEGS) {
    s += `<line x1="${r1(p[par][0])}" y1="${r1(p[par][1])}" x2="${r1(p[c][0])}" y2="${r1(p[c][1])}" stroke="${tint || cols[col]}" stroke-width="${t}"/>`;
    if (c === 'neck') { const h = head || p.head; s += `<circle cx="${r1(h[0])}" cy="${r1(h[1])}" r="${HEAD_R}" fill="${tint || cols.skin}"/>`; }
  }
  return s + '</g>';
}

C.ink2b = '#4A4F57';
const W = 1344, H = 600, TOP = 44, SHELF = 80, RAIL = 64;
const AREA_H = H - TOP - SHELF;
const PARTS = { x: 72, y: TOP + 8, w: 220 }, PROPS_W = 340;
const PROPS = { x: W - RAIL - 8 - PROPS_W, y: TOP + 8, w: PROPS_W };
const PANEL_H = AREA_H - 16;

// the canvas view: zoomed in on the figure being built
const S = 0.44, VC = [(PARTS.x + PARTS.w + PROPS.x) / 2, TOP + AREA_H / 2 - 8], UC = [960, 615];
const shift = (p, dx) => Object.fromEntries(Object.entries(p).map(([k, v]) => [k, [v[0] + dx, v[1]]]));
const HERO = shift(POSES.A[0], 260);
const REF = shift(POSES.B[0], 250);

const IC = {
  parts: '<path d="M8.5 6.5h11M8.5 12h11M8.5 17.5h11"/><circle cx="4.8" cy="6.5" r="1.2"/><circle cx="4.8" cy="12" r="1.2"/><circle cx="4.8" cy="17.5" r="1.2"/>',
  handles: '<rect x="5" y="5" width="14" height="14" rx="1.5" stroke-dasharray="2.4 2.4"/><circle cx="5" cy="5" r="2"/><circle cx="19" cy="19" r="2"/>',
  undo: '<path d="M8.5 5.5 4.5 9.5l4 4"/><path d="M4.5 9.5H14a5.5 5.5 0 0 1 0 11h-3"/>',
  redo: '<path d="m15.5 5.5 4 4-4 4"/><path d="M19.5 9.5H10a5.5 5.5 0 0 0 0 11h3"/>',
  props: '<path d="M4 8h9M17 8h3M4 16h3M11 16h9"/><circle cx="15" cy="8" r="2.2"/><circle cx="9" cy="16" r="2.2"/>',
  bin: '<path d="M4.5 7h15M9.5 7V4.8h5V7M6.5 7l.9 12.2a1 1 0 0 0 1 .8h7.2a1 1 0 0 0 1-.8L17.5 7M10.2 10.5v6M13.8 10.5v6"/>',
};
const icon = (name, size, colour, sw = 2) => `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="${colour}" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${IC[name]}</svg>`;
const railBtn = (name, label, { pressed = false, y }) => `<button type="button" class="rb${pressed ? ' on' : ''}" aria-label="${label}"${pressed ? ' aria-pressed="true"' : ''} style="top:${y}px">${icon(name, 28, C.blue)}</button>`;

// shapes, drawn small: used on the shelf, in the Parts list and on the Shape page
function shapeGlyph(kind, colour, w = 40, h = 26) {
  const cy = h / 2, x0 = w * 0.18, x1 = w * 0.82, t = h * 0.36;
  const g = {
    select: `<path d="M${w / 2 - 6} ${cy - 10}l12 9-5.2 1.2 3 6-2.6 1.2-3-6-4.2 3.4z" fill="${colour}"/>`,
    segment: `<rect x="${x0}" y="${cy - t / 2}" width="${x1 - x0}" height="${t}" fill="${colour}"/>`,
    rounded: `<line x1="${x0 + t / 2}" y1="${cy}" x2="${x1 - t / 2}" y2="${cy}" stroke="${colour}" stroke-width="${t}" stroke-linecap="round"/>`,
    circle: `<circle cx="${w / 2}" cy="${cy}" r="${h * 0.36}" fill="${colour}"/>`,
    ellipse: `<ellipse cx="${w / 2}" cy="${cy}" rx="${w * 0.3}" ry="${h * 0.26}" fill="${colour}"/>`,
    triangle: `<path d="M${w / 2} ${cy - h * 0.34}L${w / 2 + h * 0.4} ${cy + h * 0.3}H${w / 2 - h * 0.4}z" fill="${colour}"/>`,
    trapezoid: `<path d="M${x0} ${cy - t * 0.7}L${x1} ${cy - t * 0.3}V${cy + t * 0.3}L${x0} ${cy + t * 0.7}z" fill="${colour}"/>`,
    polygon: `<path d="${[0, 1, 2, 3, 4].map((i) => { const a = -Math.PI / 2 + i * 2 * Math.PI / 5; return `${i ? 'L' : 'M'}${r1(w / 2 + Math.cos(a) * h * 0.38)} ${r1(cy + Math.sin(a) * h * 0.38)}`; }).join('')}z" fill="${colour}"/>`,
    polyfill: `<path d="M${w * 0.22} ${cy + h * 0.28}Q${w * 0.3} ${cy - h * 0.4} ${w * 0.55} ${cy - h * 0.3}T${w * 0.8} ${cy + h * 0.2}z" fill="${colour}" fill-opacity=".85"/><circle cx="${w * 0.22}" cy="${cy + h * 0.28}" r="2" fill="#fff" stroke="${colour}"/><circle cx="${w * 0.8}" cy="${cy + h * 0.2}" r="2" fill="#fff" stroke="${colour}"/>`,
    connector: `<path d="M${w / 2} ${cy - 6}l6 6-6 6-6-6z" fill="none" stroke="${colour}" stroke-width="2"/>`,
    mirror: `<line x1="${w / 2}" y1="${cy - h * 0.42}" x2="${w / 2}" y2="${cy + h * 0.42}" stroke="${colour}" stroke-width="1.6" stroke-dasharray="2.5 2.5"/><path d="M${w / 2 - 4} ${cy - 7}L${w / 2 - 14} ${cy + 6}M${w / 2 + 4} ${cy - 7}L${w / 2 + 14} ${cy + 6}" stroke="${colour}" stroke-width="${t * 0.7}" stroke-linecap="round"/>`,
  }[kind];
  return `<svg viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" aria-hidden="true" style="display:block">${g}</svg>`;
}

// ---- the canvas ----
function canvas() {
  const u = (px) => r1(px / S);
  const tf = `translate(${r1(VC[0] - RAIL - UC[0] * S)} ${r1(VC[1] - TOP - UC[1] * S)}) scale(${S})`;
  let s = `<svg class="cv" width="${W - 2 * RAIL}" height="${AREA_H}" viewBox="0 0 ${W - 2 * RAIL} ${AREA_H}" style="left:${RAIL}px;top:${TOP}px" aria-label="Figure being built"><g transform="${tf}">`;
  s += `<rect x="0" y="0" width="1920" height="1080" fill="#fff"/>`;
  // size reference: a stock figure, faint, at the stock size
  s += `<g opacity=".1">${figure(REF, { skin: '#222428', shirt: '#222428', pants: '#222428' })}</g>`;
  s += `<text x="${REF.hip[0]}" y="${r1(REF.lf[1] + u(18))}" text-anchor="middle" font-family="Open Sans, sans-serif" font-size="${u(11)}" fill="#9AA0A8">Stock size</text>`;
  // the scarf: a polyfill hanging off the neck, drawn behind
  const n = HERO.neck, scarf = [[n[0] + 26, n[1] - 10], [n[0] + 26, n[1] + 16], [n[0] - 24, n[1] + 22], [n[0] - 52, n[1] + 128], [n[0] - 82, n[1] + 118], [n[0] - 50, n[1] + 18], [n[0] - 28, n[1] - 10]];
  s += `<path d="M${scarf.map((p) => `${p[0]} ${p[1]}`).join('L')}Z" fill="#CC1736" stroke="#CC1736" stroke-width="10" stroke-linejoin="round"/>`;
  s += figure(HERO, COLS.A);
  // a new segment being drawn from the right hand: a ghost, a finger, a readout
  const a = HERO.rh, b = [a[0] + 150, a[1] - 190];
  s += `<line x1="${a[0]}" y1="${a[1]}" x2="${b[0]}" y2="${b[1]}" stroke="${C.blue}" stroke-opacity=".28" stroke-width="22" stroke-linecap="round"/>`;
  s += `<circle cx="${b[0]}" cy="${b[1]}" r="${u(15)}" fill="${C.blue}" fill-opacity=".14" stroke="${C.blue}" stroke-opacity=".35" stroke-width="${u(1.5)}"/>`;
  const len = Math.round(Math.hypot(b[0] - a[0], b[1] - a[1])), deg = Math.round(-Math.atan2(b[1] - a[1], b[0] - a[0]) * 180 / Math.PI);
  s += `<g transform="translate(${b[0] + u(16)} ${b[1] - u(26)})"><rect x="0" y="0" width="${u(118)}" height="${u(22)}" rx="${u(11)}" fill="#222428" fill-opacity=".86"/><text x="${u(59)}" y="${u(15)}" text-anchor="middle" font-family="Open Sans, sans-serif" font-weight="600" font-size="${u(11.5)}" fill="#fff">Length ${len}, ${deg}°</text></g>`;
  // handles, by kind: joints are circles, the main node a square, connectors diamonds, polyfill anchors triangles
  const sel = 'rh', conn = ['re'];
  const joints = ['neck', 'head', 'le', 'lh', 're', 'rh', 'lk', 'lf', 'rk', 'rf'];
  for (const j of conn) s += `<line x1="${HERO[sel][0]}" y1="${HERO[sel][1]}" x2="${HERO[j][0]}" y2="${HERO[j][1]}" stroke="${C.hPurple}" stroke-width="${u(2)}"/>`;
  for (const j of joints) {
    if (j === sel) continue;
    s += `<circle cx="${HERO[j][0]}" cy="${HERO[j][1]}" r="${u(5.5)}" fill="#fff" fill-opacity=".9" stroke="${conn.includes(j) ? C.hPurple : C.hBlue}" stroke-width="${u(2)}"/>`;
  }
  const hp = HERO.hip, hs = u(10);
  s += `<rect x="${r1(hp[0] - hs / 2)}" y="${r1(hp[1] - hs / 2)}" width="${hs}" height="${hs}" rx="${u(1.5)}" fill="#2A2B30" stroke="#fff" stroke-width="${u(1.5)}"/>`;
  const mid = [(HERO.hip[0] + HERO.neck[0]) / 2, (HERO.hip[1] + HERO.neck[1]) / 2], d = u(6.5);
  s += `<path d="M${mid[0]} ${mid[1] - d}l${d} ${d}-${d} ${d}-${d}-${d}z" fill="#fff" stroke="#2D9A8C" stroke-width="${u(2)}" stroke-linejoin="round"/>`;
  const an = scarf[3], tr = u(7);
  s += `<path d="M${an[0]} ${an[1] - tr}L${an[0] + tr} ${an[1] + tr * 0.75}H${an[0] - tr}z" fill="#fff" stroke="#D9821E" stroke-width="${u(2)}" stroke-linejoin="round"/>`;
  s += `<circle cx="${HERO[sel][0]}" cy="${HERO[sel][1]}" r="${u(7.5)}" fill="${C.hRed}" stroke="#fff" stroke-width="${u(2)}"/>`;
  return s + '</g></svg>';
}

// ---- Parts: the figure's pieces, front to back ----
function parts() {
  const rows = [
    ['rounded', COLS.A.skin, 'Rounded 10', true], ['rounded', COLS.A.shirt, 'Rounded 9'], ['circle', COLS.A.skin, 'Circle 2'],
    ['rounded', COLS.A.shirt, 'Rounded 1'], ['connector', '#2D9A8C', 'Connector 14'], ['rounded', COLS.A.skin, 'Rounded 6'],
    ['rounded', COLS.A.shirt, 'Rounded 5'], ['rounded', COLS.A.skin, 'Rounded 4'], ['rounded', COLS.A.pants, 'Rounded 3'],
    ['rounded', COLS.A.skin, 'Rounded 8'], ['rounded', COLS.A.pants, 'Rounded 7'], ['polyfill', '#CC1736', 'Polyfill 13'],
  ];
  let s = `<section class="panel parts" aria-label="Parts" style="left:${PARTS.x}px;top:${PARTS.y}px;width:${PARTS.w}px;height:${PANEL_H}px">`;
  s += `<div class="sect"><h2 class="title">Parts</h2></div><div class="cap">Front at the top</div>`;
  for (const [k, c, label, on] of rows) s += `<button type="button" class="prt${on ? ' on' : ''}"${on ? ' aria-pressed="true"' : ''}>${shapeGlyph(k, c, 30, 20)}<span>${label}</span></button>`;
  s += `<div class="pfoot2">15 nodes</div>`;
  return s + '</section>';
}

// ---- Properties: the Shape page for the selected part ----
function props() {
  const tiles = [['segment', 'Segment'], ['rounded', 'Rounded', true], ['circle', 'Circle'], ['ellipse', 'Ellipse'], ['triangle', 'Triangle'], ['trapezoid', 'Trapezoid'], ['polygon', 'Polygon']];
  let s = `<section class="panel" aria-label="Properties" style="left:${PROPS.x}px;top:${PROPS.y}px;width:${PROPS.w}px;height:${PANEL_H}px">`;
  s += `<div class="seg" role="tablist"><button type="button" role="tab" class="on" aria-selected="true">Shape</button><button type="button" role="tab">Segment</button><button type="button" role="tab">Figure</button></div>`;
  s += `<div class="sect"><h2 class="title">Shape Tools</h2></div>`;
  s += `<div class="tiles">${tiles.map(([k, l, on]) => `<button type="button" class="tile${on ? ' on' : ''}"${on ? ' aria-pressed="true"' : ''}>${shapeGlyph(k, COLS.A.skin, 92, 46)}<span>${l}</span></button>`).join('')}</div>`;
  s += `<div class="sect s2"><h2 class="title">Rounded</h2></div>`;
  s += `<div class="prow"><span>Curve radius</span><span class="step"><button type="button" class="sb" aria-label="Less curve"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12"/></svg></button><b>0</b><button type="button" class="sb" aria-label="More curve"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 6v12M6 12h12"/></svg></button></span></div>`;
  s += `<span class="pfade"></span>`;
  return s + '</section>';
}

// ---- the shape shelf: what a drag from a joint draws ----
function shelf() {
  const tools = [['select', 'Select'], ['segment', 'Segment'], ['rounded', 'Rounded', true], ['circle', 'Circle'], ['ellipse', 'Ellipse'], ['triangle', 'Triangle'], ['trapezoid', 'Trapezoid'], ['polygon', 'Polygon'], ['polyfill', 'Polyfill']];
  let s = `<nav class="shelf" aria-label="Draw" style="height:${SHELF}px"><div class="tools">`;
  for (const [k, l, on] of tools) s += `<button type="button" class="tool${on ? ' on' : ''}"${on ? ' aria-pressed="true"' : ''}>${shapeGlyph(k, on ? C.blue : C.ink2b)}<span>${l}</span></button>`;
  s += `<span class="vsep"></span><button type="button" class="tool" aria-pressed="false">${shapeGlyph('mirror', C.ink2b)}<span>Mirror</span></button>`;
  return s + '</div></nav>';
}

function board() {
  let s = `<div class="bf" style="position:relative;width:${W}px;height:${H}px;overflow:hidden;background:#fff">`;
  s += `<header class="top" style="height:${TOP}px"><button type="button" class="tw" style="left:12px">Cancel</button>`;
  s += `<div class="name"><b>Hero (copy)</b><span>Hero in your animation stays as it is</span></div>`;
  s += `<button type="button" class="tw strong" style="right:12px">Save as new figure</button></header>`;
  s += `<div class="checker" style="position:absolute;left:${RAIL}px;top:${TOP}px;width:${W - 2 * RAIL}px;height:${AREA_H}px"></div>`;
  s += canvas();
  s += `<nav class="rail" aria-label="View" style="left:0;top:${TOP}px;height:${AREA_H}px;box-shadow:inset -1px 0 0 ${C.line}">${railBtn('parts', 'Parts', { pressed: true, y: 6 })}${railBtn('handles', 'Which handles show', { y: 54 })}</nav>`;
  s += `<nav class="rail" aria-label="Edit" style="right:0;top:${TOP}px;height:${AREA_H}px;box-shadow:inset 1px 0 0 ${C.line}">${railBtn('undo', 'Undo', { y: 6 })}${railBtn('redo', 'Redo', { y: 54 })}${railBtn('props', 'Properties', { pressed: true, y: 108 })}${railBtn('bin', 'Delete', { y: 156 })}</nav>`;
  s += parts() + props() + shelf();
  return s + '</div>';
}

const CSS = `
body{margin:0}
.bf{font-family:"Open Sans",system-ui,sans-serif;font-size:14px;color:${C.ink};-webkit-font-smoothing:antialiased}
.bf button{border:0;margin:0;padding:0;background:none;color:inherit;font:inherit;cursor:pointer}
.bf button:focus-visible{outline:2px solid ${C.blue};outline-offset:2px}
.top{position:absolute;left:0;top:0;right:0;background:#fff;box-shadow:inset 0 -1px 0 ${C.line}}
.tw{position:absolute;top:6px;height:32px;padding:0 12px!important;border-radius:9px;font-size:15px!important;font-weight:600!important;color:${C.blue}!important}
.tw.strong{font-weight:700!important}
.name{position:absolute;left:0;right:0;top:0;height:44px;display:flex;flex-direction:column;align-items:center;justify-content:center;pointer-events:none;line-height:17px}
.name b{font-size:15px;font-weight:600}
.name span{font-size:12px;color:${C.ink2}}
.checker{background-color:#6C6F74;background-image:conic-gradient(#606368 25%,transparent 0 50%,#606368 0 75%,transparent 0);background-size:16px 16px}
.cv{position:absolute;display:block}
.rail{position:absolute;width:64px;background:${C.rail}}
.rb{position:absolute;left:10px;width:44px;height:44px;border-radius:11px;display:flex;align-items:center;justify-content:center}
.rb.on{background:#fff!important;box-shadow:0 0 0 1px ${C.line},0 1px 2px rgba(0,0,0,.06)}
.panel{position:absolute;background:#fff;border-radius:16px;box-shadow:0 0 0 1px rgba(0,0,0,.08),0 10px 28px rgba(16,20,26,.3);padding:10px 14px;box-sizing:border-box;overflow:hidden;z-index:5}
.sect{position:relative;height:30px;display:flex;align-items:center;justify-content:center;margin-top:2px}
.sect.s2{margin-top:10px}
.title{margin:0;font-size:15px;font-weight:600;color:${C.red};text-align:center;line-height:20px}
.parts{padding:8px 8px}
.cap{text-align:center;font-size:11px;color:${C.ink2};margin:-4px 0 6px}
.prt{display:flex;align-items:center;gap:8px;width:100%;height:29px;padding:0 8px!important;border-radius:8px;font-size:13px!important;text-align:left}
.prt.on{background:#E6F0FB!important;color:${C.blue}!important;font-weight:600!important}
.pfoot2{position:absolute;left:0;right:0;bottom:0;height:30px;line-height:30px;text-align:center;font-size:12px;color:${C.ink2};box-shadow:inset 0 1px 0 #EEF0F2}
.seg{display:grid;grid-template-columns:repeat(3,1fr);padding:3px;border-radius:11px;background:${C.field};margin-bottom:6px}
.seg button{height:30px;border-radius:8px;color:${C.blue}!important;font-weight:600!important;font-size:13px!important}
.seg button.on{background:#fff!important;box-shadow:0 1px 3px rgba(0,0,0,.14)}
.tiles{display:grid;grid-template-columns:repeat(3,1fr);gap:10px 8px;margin-top:4px}
.tile{display:flex;flex-direction:column;align-items:center;gap:3px;font-size:12px!important}
.tile > svg{border-radius:8px;box-shadow:0 0 0 1px ${C.line};background:#fff}
.tile.on > svg{box-shadow:0 0 0 2px ${C.blue}}
.tile.on span{color:${C.blue};font-weight:600}
.prow{display:flex;align-items:center;justify-content:space-between;height:38px}
.step{display:flex;align-items:center;gap:6px}
.step b{width:36px;text-align:center;font-size:15px;font-weight:600}
.sb{width:32px;height:32px;border-radius:9px;background:${C.field}!important;display:flex;align-items:center;justify-content:center}
.pfade{position:absolute;left:0;right:0;bottom:0;height:30px;background:linear-gradient(rgba(255,255,255,0),#fff);pointer-events:none}
.shelf{position:absolute;left:0;right:0;bottom:0;background:#fff;box-shadow:inset 0 1px 0 ${C.line};display:flex;align-items:center;justify-content:center}
.tools{display:flex;align-items:center;gap:4px}
.tool{width:74px;height:64px;border-radius:12px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:4px;font-size:12px!important;color:${C.ink2}!important}
.tool.on{background:#E8F0FA!important;color:${C.blue}!important;font-weight:600!important}
.vsep{width:1px;height:40px;background:${C.line};margin:0 8px}
`;

const FONT = '<link href="https://fonts.googleapis.com/css2?family=Open+Sans:wght@400;600;700;800&amp;display=swap" rel="stylesheet">';
const body = board();
const dc = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Barkfart figure editor</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
${FONT}
<style>${CSS}</style>
</helmet>
${body}
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{"$preview":{"width":${W},"height":${H}}}'>
class Component extends DCLogic {
  renderVals() { return {}; }
}
</script>
</body>
</html>
`;
mkdirSync(join(HERE, 'preview'), { recursive: true });
writeFileSync(join(HERE, 'project', 'FigureEditor.dc.html'), dc);
writeFileSync(join(HERE, 'preview', 'figureeditor.html'), `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Preview</title>${FONT}<style>${CSS}</style></head><body style="background:#888">${body}</body></html>`);
console.log('ok', dc.length, 'bytes');
