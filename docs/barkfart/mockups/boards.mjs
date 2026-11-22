// Round 10 boards (all screens). Earlier: a flat layer list with join markers (dynamic parenting), a proper Preview row, Segment page with every control and swipe-to-reset.
// Writes project/{Timeline,AddMenu,Effects}.dc.html and preview/*.html.
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
const CELLS = 24, CUR = 11;
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
const LET_GO = 12;
function hatOn(f) {
  const p = rivalAt(f), a = Math.atan2(p.head[1] - p.neck[1], p.head[0] - p.neck[0]);
  return { x: p.head[0] + Math.cos(a) * (HEAD_R - 18), y: p.head[1] + Math.sin(a) * (HEAD_R - 18), a: a * 180 / Math.PI + 90 };
}
const H0 = hatOn(LET_GO);
const HAT_KEYS = [[LET_GO, H0, 'tween'], [15, { x: H0.x + 150, y: H0.y - 170, a: H0.a + 70 }, 'tween'], [19, { x: H0.x + 320, y: 858, a: H0.a + 150 }, 'hold']];
function hatAt(f) {
  if (f < LET_GO) return hatOn(f);
  const { k, n, t } = locate(HAT_KEYS, f);
  if (!n) return k[1];
  const A = k[1], B = n[1];
  return { x: A.x + (B.x - A.x) * t, y: A.y + (B.y - A.y) * t, a: A.a + (B.a - A.a) * t };
}
const HAT_SHAPE = (fill, band) => `<path d="M-40 -4V-46Q-40 -66 -20 -66H20Q40 -66 40 -46V-4Z" fill="${fill}"/><rect x="-40" y="-24" width="80" height="13" fill="${band}"/><line x1="-64" y1="-2" x2="64" y2="-2" stroke="${fill}" stroke-width="14" stroke-linecap="round"/>`;
const hat = (q, { tint, op } = {}) => `<g transform="translate(${r1(q.x)} ${r1(q.y)}) rotate(${r1(q.a)})"${op != null ? ` opacity="${op}"` : ''}>${HAT_SHAPE(tint || '#3B3F46', tint || '#CC1736')}</g>`;
const scene = (f, opts = {}) => {
  if (BOARD.clipEdit) return birdScene(f, opts);
  const x = BOARD.extras || {}, h = heroAt(f);
  return (x.skyline && !opts.tint ? SKYLINE() : '') + (x.birds && !opts.tint ? birdsAt(f) : '')
    + figure(rivalAt(f), COLS.B, opts) + hat(hatAt(f), opts) + figure(h, COLS.A, { ...opts, head: headPos(h, tiltAt(f)) })
    + (x.pow && !opts.tint && f >= 9 && f <= 13 ? POW() : '');
};


// glow on Hero, keyed: 0 at frame 9, full at frame 12, then held (a held pose with a moving effect)
const GLOW_KEYS = [[9, 0], [12, 1]];
const glowAt = (f) => { if (f <= GLOW_KEYS[0][0]) return GLOW_KEYS[0][1]; if (f >= GLOW_KEYS[1][0]) return GLOW_KEYS[1][1]; return ease((f - GLOW_KEYS[0][0]) / (GLOW_KEYS[1][0] - GLOW_KEYS[0][0])); };

// ---- layout ----
let W = 1344, H = 600; const TOP = 40, RAIL = 64;
const FPS = 12;
let BOARD = {};
const TB = 32, RU = 22, ST = 28, BAR = 32, NAME_W = 224, CELL = 46, PANEL_W = 372;
const DEF_ROWS = [
  { id: 'hero', h: 32 }, { id: 'head', h: 32 }, { id: 'rival', h: 32 }, { id: 'hat', h: 32 }, { id: 'sound', h: 32 },
];
const rowsOf = (st) => (st.rows || DEF_ROWS).map((r) => ({ h: 32, ...r }));
const rowsH = (st) => rowsOf(st).reduce((a, r) => a + r.h, 0);
const cx = (f) => NAME_W + (f - 1) * CELL;

function layout(st) {
  const tl = st.folded ? TB + RU + ST + 4 : TB + RU + ST + rowsH(st) + BAR;
  const areaH = H - TOP - tl;
  const availW = W - 2 * RAIL - (st.panel ? (st.dock ? PANEL_W : PANEL_W + 16) : 0);
  let sh = areaH - 20, sw = Math.round(sh * 16 / 9);
  if (sw > availW - 32) { sw = availW - 32; sh = Math.round(sw * 9 / 16); }
  const stage = { w: sw, h: sh, x: Math.round(RAIL + (availW - sw) / 2), y: TOP + Math.round((areaH - sh) / 2) };
  return { tl, areaH, stage, K: sw / 1920 };
}

// ---- pictograms: 24 grid, one stroke weight ----
const IC = {
  pose: '<circle cx="12" cy="4.6" r="2.4"/><path d="M12 7v7.2M12 9.4 7.2 6.6M12 9.4l4.4 3M12 14.2l-3.6 6.4M12 14.2l3.4 6.4"/>',
  all: '<circle cx="7.6" cy="5.6" r="2.1"/><path d="M7.6 7.9v6M7.6 10.2 4.4 12M7.6 10.2l3.2 1.8M7.6 13.9 5.2 19.4M7.6 13.9l2.4 5.5"/><circle cx="16.4" cy="5.6" r="2.1"/><path d="M16.4 7.9v6M16.4 10.2 13.2 12M16.4 10.2l3.2 1.8M16.4 13.9 14 19.4M16.4 13.9l2.4 5.5"/>',
  camera: '<path d="M3 8V5.5A1.5 1.5 0 0 1 4.5 4H7M17 4h2.5A1.5 1.5 0 0 1 21 5.5V8M21 16v2.5a1.5 1.5 0 0 1-1.5 1.5H17M7 20H4.5A1.5 1.5 0 0 1 3 18.5V16"/><circle cx="12" cy="9.2" r="1.7"/><path d="M12 10.9v3.4M12 12l-2.2 1M12 12l2.2 1M12 14.3l-1.6 2.7M12 14.3l1.6 2.7"/>',
  undo: '<path d="M8.5 5.5 4.5 9.5l4 4"/><path d="M4.5 9.5H14a5.5 5.5 0 0 1 0 11h-3"/>',
  redo: '<path d="m15.5 5.5 4 4-4 4"/><path d="M19.5 9.5H10a5.5 5.5 0 0 0 0 11h3"/>',
  bin: '<path d="M4.5 7h15M9.5 7V4.8h5V7M6.5 7l.9 12.2a1 1 0 0 0 1 .8h7.2a1 1 0 0 0 1-.8L17.5 7M10.2 10.5v6M13.8 10.5v6"/>',
  full: '<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/>',
  props: '<path d="M4 8h9M17 8h3M4 16h3M11 16h9"/><circle cx="15" cy="8" r="2.2"/><circle cx="9" cy="16" r="2.2"/>',
  figure: '<circle cx="12" cy="4.6" r="2.4"/><path d="M12 7v7.2M12 9.4l-4.4 3M12 9.4l4.4 3M12 14.2l-3.4 6.4M12 14.2l3.4 6.4"/>',
  headonly: '<circle cx="12" cy="12" r="5.5"/>',
  sound: '<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>',
  picture: '<rect x="3.5" y="5" width="17" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="m4 17.5 5-4.5 3.5 3 3-2.5 4 3.5"/>',
  text: '<path d="M5.5 6h13M12 6v13M9.5 19h5"/>',
  clip: '<rect x="3.5" y="4" width="17" height="16" rx="2"/><path d="M7.5 4v16M16.5 4v16"/><circle cx="12" cy="8.4" r="1.4"/><path d="M12 9.8v3.6M12 11l-1.7 1M12 11l1.7 1M12 13.4l-1.3 2.6M12 13.4l1.3 2.6"/>',
  hat: '<path d="M7.5 15.5V9a2.5 2.5 0 0 1 2.5-2.5h4A2.5 2.5 0 0 1 16.5 9v6.5"/><path d="M3.5 16h17"/><path d="M7.5 12.5h9"/>',
  chain: '<path d="M10 13.5a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1 1"/><path d="M14 10.5a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1-1"/>',
  broken: '<path d="M10.5 6 12 4.5a3.5 3.5 0 0 1 5 5L15.5 11"/><path d="M13.5 18 12 19.5a3.5 3.5 0 0 1-5-5L8.5 13"/><path d="M4.5 8.5H7M8.5 4.5V7M19.5 15.5H17M15.5 19.5V17"/>',
  folder: '<path d="M3.5 7.5A1.5 1.5 0 0 1 5 6h4l2 2h8a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19 19H5a1.5 1.5 0 0 1-1.5-1.5z"/>',
  eye: '<path d="M2.5 12S6 5.8 12 5.8 21.5 12 21.5 12 18 18.2 12 18.2 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  back: '<path d="m15 5-7 7 7 7"/>',
  next: '<path d="m9 5 7 7-7 7"/>',
  plus: '<path d="M12 5.5v13M5.5 12h13"/>',
  x: '<path d="M7 7l10 10M17 7 7 17"/>',
  key: '<path d="M12 4.8 19.2 12 12 19.2 4.8 12z"/>',
  onion: '<circle cx="9" cy="12" r="5.6" stroke-dasharray="2.2 2.2"/><circle cx="15" cy="12" r="5.6"/>',
  loop: '<path d="M17.5 8A6.5 6.5 0 0 0 6 9"/><path d="M6.5 16a6.5 6.5 0 0 0 11.5-1"/><path d="M18 4.2v4h-4"/><path d="M6 19.8v-4h4"/>',
  search: '<circle cx="10.5" cy="10.5" r="5.5"/><path d="m15 15 4.5 4.5"/>',
  wiggle: '<path d="M3 12l2.2-4 2.6 8 2.6-9 2.6 10 2.6-9 2.6 8L20.5 12"/>',
  swing: '<path d="M12 3.5v0"/><circle cx="12" cy="3.8" r="1"/><path d="M12 4.5 17 15"/><circle cx="17.6" cy="16.4" r="2.4"/><path d="M5.5 14.5a9.5 9.5 0 0 0 14 3.5" stroke-dasharray="1.8 2.2"/>',
  loopmod: '<path d="M8.5 7.5A6 6 0 1 0 12 6"/><path d="m12 2.8 0 6.2-3.4-1.6"/>',
  follow: '<circle cx="17" cy="8" r="2.6"/><circle cx="7" cy="16" r="2.6" stroke-dasharray="1.6 1.6"/><path d="M9.3 14.3 14.6 9.7"/><path d="m11.5 9.5 3.1.2-.2 3.1"/>',
  link: '<path d="M10 14a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1 1"/><path d="M14 10a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1-1"/>',
};
const icon = (name, size, colour, sw = 2) => `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="${colour}" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${IC[name]}</svg>`;
// transport glyphs are filled, the way every player draws them
const TP = {
  stepBack: '<rect x="5" y="5.5" width="2.6" height="13" rx="1"/><path d="M19 6.6v10.8a.8.8 0 0 1-1.25.66l-7.9-5.4a.8.8 0 0 1 0-1.32l7.9-5.4A.8.8 0 0 1 19 6.6z"/>',
  stepFwd: '<rect x="16.4" y="5.5" width="2.6" height="13" rx="1"/><path d="M5 6.6v10.8a.8.8 0 0 0 1.25.66l7.9-5.4a.8.8 0 0 0 0-1.32l-7.9-5.4A.8.8 0 0 0 5 6.6z"/>',
  toStart: '<path d="M12 7.2v9.6a.7.7 0 0 1-1.1.57L4.4 12.6a.7.7 0 0 1 0-1.2l6.5-4.77a.7.7 0 0 1 1.1.57z"/><path d="M20.5 7.2v9.6a.7.7 0 0 1-1.1.57l-6.5-4.77a.7.7 0 0 1 0-1.2l6.5-4.77a.7.7 0 0 1 1.1.57z"/>',
  play: '<path d="M7 5.4v13.2a.8.8 0 0 0 1.23.67l10.3-6.6a.8.8 0 0 0 0-1.34L8.23 4.73A.8.8 0 0 0 7 5.4z"/>',
  toEnd: '<path d="M3.5 7.2v9.6a.7.7 0 0 0 1.1.57l6.5-4.77a.7.7 0 0 0 0-1.2L4.6 6.63a.7.7 0 0 0-1.1.57z"/><path d="M12 7.2v9.6a.7.7 0 0 0 1.1.57l6.5-4.77a.7.7 0 0 0 0-1.2l-6.5-4.77a.7.7 0 0 0-1.1.57z"/>',
};
const tp = (name, size) => `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="${C.ink2b}" aria-hidden="true">${TP[name]}</svg>`;
C.ink2b = '#4A4F57';

const railBtn = (name, label, { active = false, pressed = false, y }) => `<button type="button" class="rb${active || pressed ? ' on' : ''}" aria-label="${label}"${active || pressed ? ' aria-pressed="true"' : ''} style="top:${y}px">${icon(name, 28, active ? C.red : C.blue)}</button>`;

// ---- stage ----
function stageSvg(L, st) {
  const { stage, K } = L;
  const u = (px) => r1(px / K);
  const joint = 'rh';
  const g = glowAt(CUR);
  let s = `<svg viewBox="0 0 1920 1080" width="${stage.w}" height="${stage.h}" style="position:absolute;left:${stage.x}px;top:${stage.y}px;display:block;background:#fff" aria-label="Stage, frame ${CUR}">`;
  s += `<defs><filter id="hglow" x="-40%" y="-40%" width="180%" height="180%"><feGaussianBlur in="SourceAlpha" stdDeviation="26" result="b"/><feFlood flood-color="#FFC83D" flood-opacity="${r1(g)}"/><feComposite in2="b" operator="in" result="gl"/><feMerge><feMergeNode in="gl"/><feMergeNode in="gl"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>`;
  if (BOARD.clipEdit) return s + birdScene(CUR - 1, { tint: '#5B8DEF', op: 0.2 }) + birdScene(CUR) + '</svg>';
  const xt = BOARD.extras || {};
  if (xt.skyline) s += SKYLINE();
  if (xt.birds) s += birdsAt(CUR);
  // onion skin of the frame before
  const ph = heroAt(CUR - 1);
  s += figure(rivalAt(CUR - 1), COLS.B, { tint: '#5B8DEF', op: 0.2 }) + hat(hatAt(CUR - 1), { tint: '#5B8DEF', op: 0.2 }) + figure(ph, COLS.A, { tint: '#5B8DEF', op: 0.2, head: headPos(ph, tiltAt(CUR - 1)) });
  const h = heroAt(CUR);
  s += figure(rivalAt(CUR), COLS.B) + hat(hatAt(CUR));
  s += `<g filter="url(#hglow)">${figure(h, COLS.A, { head: headPos(h, tiltAt(CUR)) })}</g>`;
  if (xt.pow) s += POW();
  if (st.selPic) s += selBox(0, 520, 1920, 560, u);
  if (st.selText) s += `<g transform="rotate(-8 1560 200)">${selBox(1360, 110, 420, 175, u)}</g>`;
  if (st.selClip) s += selBox(180, 60, 470, 230, u);
  if (st.camFrame) s += camOverlay(u);
  if (st.noHandles) return s + '</svg>';
  // motion path: one dot per in-between frame
  const pts = [], dots = [], keys = new Map();
  for (let f = 1; f <= CELLS; f++) {
    const q = heroAt(f)[joint];
    pts.push(q);
    const { n } = locate(LAYERS.hero, f);
    if (LAYERS.hero.some((k) => k[0] === f)) { const kk = q.map(Math.round).join(','); if (!keys.has(kk)) keys.set(kk, { p: q, frames: [] }); keys.get(kk).frames.push(f); }
    else if (n) dots.push(q);
  }
  s += `<polyline points="${pts.map((p) => `${r1(p[0])},${r1(p[1])}`).join(' ')}" fill="none" stroke="${C.hRed}" stroke-opacity=".5" stroke-width="${u(2)}" stroke-linejoin="round" stroke-linecap="round"/>`;
  for (const q of dots) s += `<circle cx="${r1(q[0])}" cy="${r1(q[1])}" r="${u(2.4)}" fill="${C.hRed}" fill-opacity=".85"/>`;
  const cur = h[joint].map(Math.round).join(',');
  const labels = { '760,560': [1, 1], '560,420': [0, -1], '965,400': [1, -1], '875,500': [1, 1] };
  for (const [k, q] of keys) {
    if (k !== cur) s += `<circle cx="${r1(q.p[0])}" cy="${r1(q.p[1])}" r="${u(5)}" fill="#fff" stroke="${C.hRed}" stroke-width="${u(2)}"/>`;
    const [dx, dy] = labels[k] || [1, -1];
    const tx = q.p[0] + dx * u(10), ty = q.p[1] + dy * u(dx === 0 ? 11 : 10) + (dy > 0 ? u(9) : 0);
    s += `<text x="${r1(tx)}" y="${r1(ty)}" text-anchor="${dx > 0 ? 'start' : dx < 0 ? 'end' : 'middle'}" font-family="Open Sans, sans-serif" font-weight="700" font-size="${u(11)}" fill="${C.hRed}" stroke="#fff" stroke-width="${u(3)}" paint-order="stroke" stroke-linejoin="round">${q.frames.join(', ')}</text>`;
  }
  const conn = BODY_JOINTS.filter((j) => PARENT[j] === joint || PARENT[joint] === j);
  for (const j of conn) s += `<line x1="${r1(h[joint][0])}" y1="${r1(h[joint][1])}" x2="${r1(h[j][0])}" y2="${r1(h[j][1])}" stroke="${C.hPurple}" stroke-width="${u(2)}" stroke-linecap="round"/>`;
  for (const j of BODY_JOINTS) {
    if (j === joint) continue;
    const p = h[j];
    if (j === 'hip') { const hs = u(9); s += `<rect x="${r1(p[0] - hs / 2)}" y="${r1(p[1] - hs / 2)}" width="${hs}" height="${hs}" rx="${u(1.5)}" fill="#fff" stroke="${C.hRoot}" stroke-width="${u(2)}"/>`; continue; }
    s += `<circle cx="${r1(p[0])}" cy="${r1(p[1])}" r="${u(5)}" fill="#fff" fill-opacity=".85" stroke="${conn.includes(j) ? C.hPurple : C.hBlue}" stroke-width="${u(2)}"/>`;
  }
  s += `<circle cx="${r1(h[joint][0])}" cy="${r1(h[joint][1])}" r="${u(7)}" fill="${C.hRed}" stroke="#fff" stroke-width="${u(2)}"/>`;
  return s + '</svg>';
}

// ---- timeline ----
function miniPose(p, cols, head) {
  const xs = Object.values(p).map((q) => q[0]), ys = Object.values(p).map((q) => q[1]);
  const x0 = Math.min(...xs) - 70, x1 = Math.max(...xs) + 70, y0 = Math.min(...ys) - 70, y1 = Math.max(...ys) + 30;
  const h = 22, w = r1(h * (x1 - x0) / (y1 - y0));
  return `<svg viewBox="${r1(x0)} ${r1(y0)} ${r1(x1 - x0)} ${r1(y1 - y0)}" width="${w}" height="${h}" aria-hidden="true" style="display:block">${figure(p, cols, { head })}</svg>`;
}
const miniHead = (tilt) => { const a = tilt * Math.PI / 180; return `<svg viewBox="0 0 24 20" width="22" height="19" aria-hidden="true" style="display:block"><line x1="12" y1="19" x2="${r1(12 + Math.sin(a) * 9)}" y2="${r1(19 - Math.cos(a) * 9)}" stroke="${COLS.A.shirt}" stroke-width="3" stroke-linecap="round"/><circle cx="${r1(12 + Math.sin(a) * 10)}" cy="${r1(19 - Math.cos(a) * 10)}" r="6" fill="${COLS.A.skin}"/></svg>`; };
const miniHat = (a) => `<svg viewBox="-80 -80 160 160" width="22" height="22" aria-hidden="true" style="display:block"><g transform="rotate(${r1(a)}) translate(0 30)">${HAT_SHAPE('#3B3F46', '#CC1736')}</g></svg>`;
const cellPic = {
  camera: () => icon('camera', 18, '#6B7078'),
  text: () => `<span class="btxt">POW!</span>`,
  picture: () => `<svg viewBox="0 520 1920 560" width="44" height="13" aria-hidden="true" style="display:block;margin-top:4px">${SKYLINE('#B9C4D2')}</svg>`,
  clip: () => `<svg viewBox="-60 -40 220 80" width="36" height="13" aria-hidden="true" style="display:block;margin-top:4px">${bird(0, 0, 1, 1)}${bird(110, 10, 0.8, 0)}</svg>`,
  bird1: (f) => miniBird(birdV(1, f)), bird2: (f) => miniBird(birdV(2, f)), bird3: (f) => miniBird(birdV(3, f)),
  hat: (f) => miniHat(hatAt(f).a),
  hero: (f) => { const p = heroAt(f); return miniPose(p, COLS.A, headPos(p, tiltAt(f))); },
  head: (f) => miniHead(tiltAt(f)),
  rival: (f) => miniPose(rivalAt(f), COLS.B),
};
let SEL = { row: 'hero', frame: 9 };
function blocksFor(id, keys, label, h) {
  let s = '';
  keys.forEach((k, i) => {
    if (k[2] === 'end') return;
    const end = i + 1 < keys.length ? keys[i + 1][0] - 1 : CELLS;
    const tw = k[2] === 'tween' && i + 1 < keys.length;
    const sel = !!SEL && id === SEL.row && k[0] === SEL.frame;
    const x = cx(k[0]) + 1, w = (end - k[0] + 1) * CELL - 2;
    let inner = '';
    const nudge = k[0] === 1 && (id === 'head' || id === 'hat') ? 9 : 0;
    for (let f = k[0]; f <= (tw ? end : k[0]); f++) inner += `<span class="pic" style="left:${(f - k[0]) * CELL + (f === k[0] ? nudge : 0)}px">${cellPic[id](f)}</span>`;
    if (sel) inner += `<span class="hd" style="left:3px"></span><span class="hd" style="right:3px"></span>`;
    s += `<div class="blk${sel ? ' sel' : ''}" role="button" tabindex="0" aria-label="${label}, frame ${k[0]} to ${end}, ${tw ? 'tweens' : 'holds'}"${sel ? ' aria-pressed="true"' : ''} style="left:${x}px;width:${w}px;top:${(h - 26) / 2}px">${inner}</div>`;
  });
  return s;
}

const ROWDEF = {
  hero: ['figure', 'Hero'], head: ['headonly', 'Head'], rival: ['figure', 'Rival'], hat: ['hat', 'Hat'], sound: ['sound', 'Punch'],
  camera: ['camera', 'Camera'], text: ['text', 'POW!'], picture: ['picture', 'Skyline'], clip: ['clip', 'Bird flock'],
  group: ['folder', 'Fighters'], bird1: ['figure', 'Bird 1'], bird2: ['figure', 'Bird 2'], bird3: ['figure', 'Bird 3'],
};
function nameRow(r, st) {
  const on = (SEL && r.id === SEL.row) || (st.multi || []).includes(r.id);
  const eye = (n) => `<button type="button" class="eye" aria-label="Hide ${n}">${icon('eye', 18, '#9AA0A8')}</button>`;
  let inner;
  if (r.id === 'glow') inner = `<span class="pn">Glow strength</span><button type="button" class="px" aria-label="Stop animating glow strength">${icon('x', 14, '#9AA0A8')}</button>`;
  else {
    const [ic, name] = ROWDEF[r.id];
    const fold = r.id === 'hero' || r.id === 'group'
      ? `<button type="button" class="dis" aria-label="${r.id === 'group' ? 'Fold the group' : "Show Hero's animated properties"}" aria-expanded="${!!r.open}">${icon(r.open ? 'chevron' : 'next', 16, C.ink2)}</button>`
      : `<span class="dis"></span>`;
    inner = `${fold}${icon(ic, 20, on ? C.blue : C.ink2)}<span>${name}</span>${eye(name)}`;
  }
  const pad = r.id === 'glow' ? 44 : 8 + 20 * (r.indent || 0);
  const swiped = st.swipe && r.id === 'rival';
  let s = `<div class="nrow${on ? ' on' : ''}${r.id === 'glow' ? ' prop' : ''}" style="height:${r.h}px">`;
  s += `<div class="nin" style="padding-left:${pad}px${swiped ? ';transform:translateX(-28px)' : ''}">${inner}</div>`;
  if (swiped) s += `<div class="acts"><button type="button" class="dup">Duplicate</button><button type="button" class="del">Delete</button></div>`;
  return s + '</div>';
}

function timeline(st, L) {
  const top = H - L.tl;
  let s = `<section class="tl" aria-label="Timeline" style="top:${top}px;height:${L.tl}px">`;
  // control toolbar: frame steps far left, view toggles, transport in the middle, fps and fold on the right
  s += `<div class="tb" style="height:${TB}px">`;
  s += `<span class="grp" style="left:8px"><button type="button" class="ib" aria-label="Back one frame">${tp('stepBack', 20)}</button><button type="button" class="ib" aria-label="Forward one frame">${tp('stepFwd', 20)}</button></span>`;
  s += `<span class="grp" style="left:112px"><button type="button" class="ib on" aria-label="Onion skin" aria-pressed="true">${icon('onion', 20, C.blue)}</button><button type="button" class="ib" aria-label="Loop" aria-pressed="false">${icon('loop', 20, C.ink2b)}</button><button type="button" class="ib fx on" aria-label="Show effects while editing" aria-pressed="true">FX</button></span>`;
  s += `<span class="grp mid"><button type="button" class="ib" aria-label="Go to the first frame">${tp('toStart', 20)}</button><button type="button" class="ib" aria-label="Play">${tp('play', 22)}</button><button type="button" class="ib" aria-label="Go to the last frame">${tp('toEnd', 20)}</button></span>`;
  s += `<span class="grp" style="right:8px"><button type="button" class="fps" aria-label="Frames per second: ${FPS}"${BOARD.fps ? ' aria-expanded="true"' : ''}>${FPS} fps</button><button type="button" class="ib" aria-label="${st.folded ? 'Show the layers' : 'Fold the timeline'}">${st.folded ? icon('chevron', 20, C.ink2b).replace('<path d="m6 9 6 6 6-6"/>', '<path d="m6 15 6-6 6 6"/>') : icon('chevron', 20, C.ink2b)}</button></span>`;
  s += `</div>`;
  const y0 = TB;
  // names column (below the toolbar)
  if (!st.folded) {
    s += `<div class="names" style="top:${y0}px;width:${NAME_W}px;height:${RU + ST + rowsH(st)}px">`;
    s += `<div class="rh" style="height:${RU}px"></div><div class="pvrow" style="height:${ST}px"><span>Preview</span><button type="button" class="eye" aria-label="Hide the preview">${icon('eye', 16, '#9AA0A8')}</button></div>`;
    for (const r of rowsOf(st)) s += nameRow(r, st);
    s += `</div>`;
  } else {
    s += `<div class="names" style="top:${y0}px;width:${NAME_W}px;height:${RU + ST + 4}px"><div class="rh" style="height:${RU}px"></div><div class="pvrow" style="height:${ST}px"><span>Preview</span><button type="button" class="eye" aria-label="Hide the preview">${icon('eye', 16, '#9AA0A8')}</button></div></div>`;
  }
  // ruler in frames
  s += `<div class="ruler" style="left:${NAME_W}px;top:${y0}px;height:${RU}px">`;
  for (let f = 1; f <= CELLS; f++) {
    const m = (f - 1) * CELL + CELL / 2, sec = f % FPS === 0, big = f === 1 || f % (FPS / 4) === 0;
    s += `<span class="tk${sec ? ' sec' : big ? ' big' : ''}" style="left:${m}px"></span>`;
    if (big && f !== CUR) s += `<span class="rn${sec ? ' sec' : ''}" style="left:${m - 20}px">${f}</span>`;
  }
  s += `</div>`;
  // merged preview, tiled
  {
    const x0 = cx(1), w = CELLS * CELL, th = ST - 6, tw = Math.round(th * 16 / 9);
    s += `<div class="strip" role="button" tabindex="0" aria-label="Preview of every layer, merged" style="left:${x0}px;top:${y0 + RU + 3}px;width:${w}px;height:${th}px">`;
    for (let i = 0; i * tw < w; i++) {
      const f = Math.min(CELLS, 1 + Math.floor((i * tw + tw / 2) / CELL));
      s += `<svg viewBox="0 0 1920 1080" width="${tw - 1}" height="${th}" preserveAspectRatio="xMidYMid slice" aria-hidden="true" style="position:absolute;left:${i * tw}px;top:0">${scene(f)}</svg>`;
    }
    s += `</div>`;
  }
  if (!st.folded) {
    let y = y0 + RU + ST;
    for (const r of rowsOf(st)) {
      let inner = '';
      if (r.id === 'hero') inner = blocksFor('hero', LAYERS.hero, 'Hero', r.h);
      if (r.id === 'head') inner = blocksFor('head', LAYERS.head, 'Head', r.h);
      if (r.id === 'rival') inner = blocksFor('rival', LAYERS.rival, 'Rival', r.h);
      if (r.id === 'camera') inner = blocksFor('camera', CAM_KEYS, 'Camera', r.h);
      if (r.id === 'text') inner = blocksFor('text', TEXT_KEYS, 'POW!', r.h);
      if (r.id === 'picture') inner = blocksFor('picture', [[1, 0, 'hold']], 'Skyline', r.h);
      if (r.id === 'clip') inner = blocksFor('clip', [[1, 0, 'hold']], 'Bird flock', r.h);
      if (r.id.startsWith('bird')) inner = blocksFor(r.id, birdKeys(+r.id.slice(4)), r.id, r.h);
      if (r.id === 'group') {
        const fr = [...new Set([...LAYERS.hero, ...LAYERS.head, ...LAYERS.rival].map((k) => k[0]))];
        inner = `<div class="gbar${SEL && SEL.row === 'group' ? ' sel' : ''}" role="button" tabindex="0" aria-label="Fighters, every pose in the group" style="left:${cx(1) + 1}px;width:${CELLS * CELL - 2}px;top:${(r.h - 12) / 2}px">${fr.map((f) => `<i style="left:${(f - 1) * CELL + CELL / 2 - 1}px"></i>`).join('')}</div>`;
      }
      if (r.id === 'hat') inner = blocksFor('hat', [[1, 0, 'hold'], ...HAT_KEYS.map((k) => [k[0], 0, k[2]])], 'Hat', r.h);
      // joins are moments on the row: where it joins, and where it lets go
      const mark = (f, broken, what) => `<button type="button" class="jm${broken ? ' br' : ''}" aria-label="${what}" style="left:${f === 1 ? cx(1) + 1 : cx(f) - 9}px;top:7px">${icon(broken ? 'broken' : 'chain', 13, broken ? '#D91A3A' : '#5D636B', 2.2)}</button>`;
      if (r.id === 'head') inner += mark(1, false, 'Joins Hero at frame 1');
      if (r.id === 'hat') inner += mark(1, false, 'Joins Rival at frame 1') + mark(LET_GO, true, `Lets go of Rival at frame ${LET_GO}`);
      if (r.id === 'glow') for (const [f] of GLOW_KEYS) inner += `<button type="button" class="kd" aria-label="Glow strength key, frame ${f}" style="left:${cx(f) + CELL / 2 - 6}px;top:${(r.h - 12) / 2}px"></button>`;
      if (r.id === 'sound') {
        const x = cx(SOUND.from) + 1, w = (SOUND.to - SOUND.from + 1) * CELL - 2, n = Math.floor((w - 12) / 4);
        let wave = '';
        for (let i = 0; i < n; i++) { const a = Math.max(0.12, Math.exp(-i / (n * 0.4)) * (0.55 + 0.45 * Math.abs(Math.sin(i * 1.7)))); wave += `<rect x="${i * 4}" y="${r1(10 - a * 9)}" width="2.2" height="${r1(a * 18)}" rx="1" fill="#86AFC8"/>`; }
        const ss = SEL && SEL.row === 'sound';
        inner = `<div class="blk snd${ss ? ' sel' : ''}" style="left:${x}px;width:${w}px;top:3px">${ss ? '<span class="hd" style="left:3px"></span><span class="hd" style="right:3px"></span>' : ''}<svg viewBox="0 0 ${n * 4} 20" width="${n * 4}" height="20" aria-hidden="true" style="position:absolute;left:6px;top:2px">${wave}</svg></div>`;
      }
      s += `<div class="row${r.id === 'glow' ? ' prop' : ''}" style="top:${y}px;height:${r.h}px">${inner}</div>`;
      y += r.h;
    }
  }
  // the frame marker: quiet
  if (st.multi) {
    let y = y0 + RU + ST, top = 0, bot = 0;
    for (const r of rowsOf(st)) { if (r.id === st.multi[0]) top = y; if (r.id === st.multi[st.multi.length - 1]) bot = y + r.h; y += r.h; }
    s += `<span class="lasso" style="left:${cx(5) - 3}px;top:${top + 1}px;width:${8 * CELL + 6}px;height:${bot - top - 2}px"></span>`;
  }
  if (st.frameSel) s += `<span class="fsel" style="left:${cx(CUR)}px;top:${y0 + RU + 1}px;width:${CELL}px;height:${ST - 2}px"></span>`;
  const px = cx(CUR) + CELL / 2, lineTop = y0 + RU - 2, lineH = (st.folded ? RU + ST : RU + ST + rowsH(st)) - RU + 2;
  s += `<span class="ph" style="left:${px - 0.75}px;top:${lineTop}px;height:${lineH}px"></span>`;
  s += `<span class="pill" style="left:${px - 14}px;top:${y0 + 2}px">${CUR}</span>`;
  if (!st.folded) {
    s += `<div class="bar" style="height:${BAR}px">`;
    s += `<button type="button" class="ib add${st.menu ? ' on' : ''}" aria-label="Add" aria-expanded="${!!st.menu}">${icon('plus', 20, C.blue, 2.2)}</button>`;
    s += `<button type="button" class="ib" aria-label="Keying: changes make keys" aria-pressed="false">${icon('key', 18, C.ink2b)}</button>`;
    s += `<nav class="bw" aria-label="Frames">${['Add frame', 'New pose', 'Hold longer'].map((w) => `<button type="button">${w}</button>`).join('')}</nav>`;
    s += `</div>`;
  }
  return s + '</section>';
}

// ---- the "+" menu ----
function addMenu() {
  const items = [['figure', 'Stick figure'], ['picture', 'Picture'], ['text', 'Text'], ['sound', 'Sound'], ['clip', 'Movieclip'], null, ['folder', 'Group']];
  let s = `<section class="menu" aria-label="Add" style="left:8px;bottom:${BAR + 8}px">`;
  for (const it of items) s += it ? `<button type="button" class="mi">${icon(it[0], 22, C.blue)}<span>${it[1]}</span></button>` : '<hr>';
  return s + `<span class="mnub"></span></section>`;
}

const SWATCHES = ['#9B1C2E', '#2160A8', '#2A2B30', '#EDB98A', '#1E9E55', '#F2A93B', '#FFFFFF'];
const swatchRow = (cur) => `<span class="sws">${SWATCHES.map((c) => `<button type="button" class="swb${c === cur ? ' on' : ''}" aria-label="Colour ${c}" style="background:${c}"></button>`).join('')}<button type="button" class="swb more" aria-label="More colours"></button></span>`;
// ---- properties panel ----
function panelProps(L) {
  const x = W - RAIL - 8 - PANEL_W, y = TOP + 8, h = L.areaH - 16;
  let s = `<section class="panel" aria-label="Properties" style="left:${x}px;top:${y}px;width:${PANEL_W}px;height:${h}px">`;
  s += `<div class="seg" role="tablist"><button type="button" role="tab" class="on" aria-selected="true">Stickfigure</button><button type="button" role="tab">Segment</button><button type="button" role="tab">Pose</button></div>`;
  s += `<div class="prow"><b class="fname">Hero</b><button type="button" class="elink">Edit as a new figure…</button></div>`;
  s += `<div class="prow crow"><span>Colour</span>${swatchRow(COLS.A.shirt)}</div>`;
  s += `<div class="sect"><h2 class="title">Effects</h2><button type="button" class="tadd" aria-label="Add an effect">${icon('plus', 20, C.blue, 2.2)}</button></div>`;
  s += `<div class="fxrow"><span class="fxn">Glow</span><span class="fxk">${icon('key', 12, C.blue, 2.4)}strength animated</span><button type="button" class="eye" aria-label="Hide Glow">${icon('eye', 18, '#9AA0A8')}</button></div>`;
  s += `<div class="sect"><h2 class="title">Blending</h2></div>`;
  s += `<div class="prow"><span>Opacity</span><b>100%</b></div><div class="slider"><span style="width:100%"></span><i style="left:100%"></i></div>`;
  s += `<div class="prow link"><span>Blend</span><b>Normal ${icon('next', 14, C.ink2)}</b></div>`;
  s += `<div class="sect"><h2 class="title">Stickfigure Tools</h2></div>`;
  s += `<div class="prow"><span>Draw order: free</span>${'<svg viewBox="0 0 30 30" width="26" height="26" aria-hidden="true"><rect x="1" y="1" width="28" height="28" rx="7" fill="#fff" stroke="#C9CED5" stroke-width="1.5"/><path d="M8.5 15.5l4.4 4.4 8.6-9.4" fill="none" stroke="#D9DDE2" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"/></svg>'}</div>`;
  s += `<span class="pfade"></span>`;
  return s + '</section>';
}

const tick = (on) => `<svg viewBox="0 0 30 30" width="26" height="26" aria-hidden="true"><rect x="1" y="1" width="28" height="28" rx="7" fill="#fff" stroke="${on ? C.blue : '#C9CED5'}" stroke-width="${on ? 2 : 1.5}"/><path d="M8.5 15.5l4.4 4.4 8.6-9.4" fill="none" stroke="${on ? C.blue : '#D9DDE2'}" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const stepper = (label, v) => `<div class="prow"><span>${label}</span><span class="step"><button type="button" class="sb" aria-label="Less ${label.toLowerCase()}"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12"/></svg></button><b>${v}</b><button type="button" class="sb" aria-label="More ${label.toLowerCase()}"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 6v12M6 12h12"/></svg></button></span></div>`;
function panelSegment(L, scrolled) {
  const x = W - RAIL - 8 - PANEL_W, y = TOP + 8, h = L.areaH - 16;
  // a value that differs from the figure file gets a small dot; swiping its row offers the file's value back
  const row = (label, ctrl, { changed = false, swiped = null } = {}) => {
    const lab = `<span class="lab">${changed ? '<i class="chg" title="Changed from the figure file"></i>' : ''}${label}</span>`;
    if (!swiped) return `<div class="prow">${lab}${ctrl}</div>`;
    return `<div class="prow swiped"><div class="swin">${lab}${ctrl}</div><button type="button" class="rst">Reset to ${swiped}</button></div>`;
  };
  const step = (label, v) => `<span class="step"><button type="button" class="sb" aria-label="Less ${label}"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12"/></svg></button><b>${v}</b><button type="button" class="sb" aria-label="More ${label}"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 6v12M6 12h12"/></svg></button></span>`;
  let s = `<section class="panel seg-page" aria-label="Properties" style="left:${x}px;top:${y}px;width:${PANEL_W}px;height:${h}px">`;
  s += `<div class="scroll"><div class="sin"${scrolled ? ` style="transform:translateY(-${scrolled}px)"` : ''}>`;
  s += `<div class="seg" role="tablist"><button type="button" role="tab">Stickfigure</button><button type="button" role="tab" class="on" aria-selected="true">Segment</button><button type="button" role="tab">Pose</button></div>`;
  s += `<div class="sect"><h2 class="title">Segment Tools</h2></div>`;
  s += `<div class="pair"><button type="button" class="wb">Flip sideways</button><button type="button" class="wb">Flip over</button></div>`;
  s += `<div class="sect s2"><h2 class="title">Size</h2></div>`;
  s += row('Thickness', step('thickness', 28), { changed: true, swiped: 32 });
  s += row('Length', step('length', 95), { changed: true });
  s += row('Use segment scale', tick(false));
  s += row('Curve radius', step('curve radius', 0));
  s += `<div class="sect s2"><h2 class="title">Colour</h2></div>`;
  s += row('Own colour', tick(true));
  s += `<div class="prow crow sub2">${swatchRow('#EDB98A')}</div>`;
  s += row('Use gradient colour', tick(false));
  s += `<div class="sect s2"><h2 class="title">Movement</h2></div>`;
  s += `<div class="prow"><span class="lab">Angle lock</span></div><div class="seg seg3"><button type="button" class="on">None</button><button type="button">Absolute</button><button type="button">Relative</button></div>`;
  s += row('Show motion path', tick(true)) + row('Is stretchy', tick(false)) + row('Lock node', tick(false)) + row('Drag lock', tick(false));
  s += `</div></div><span class="pfade" style="bottom:52px;height:22px"></span>`;
  s += `<div class="pfoot"><button type="button" class="rall">Reset this segment</button></div>`;
  return s + '</section>';
}

const TW = 106, TH = 58;
// effect previews: Hero's punch, with the real effect drawn on it
function fxThumb(kind, id) {
  const p = POSES.A[2], fig = figure(p, COLS.A, { head: headPos(p, 0) });
  const vb = '470 250 720 405';
  let defs = '', body = `<g filter="url(#${id})">${fig}</g>`;
  if (kind === 'blur') defs = `<filter id="${id}" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="14"/></filter>`;
  if (kind === 'motion') defs = `<filter id="${id}" x="-30%" y="-10%" width="160%" height="120%"><feGaussianBlur stdDeviation="34 0"/></filter>`;
  if (kind === 'glow') defs = `<filter id="${id}" x="-40%" y="-40%" width="180%" height="180%"><feGaussianBlur in="SourceAlpha" stdDeviation="24" result="b"/><feFlood flood-color="#FFC83D"/><feComposite in2="b" operator="in" result="g"/><feMerge><feMergeNode in="g"/><feMergeNode in="g"/><feMergeNode in="SourceGraphic"/></feMerge></filter>`;
  if (kind === 'adjust') defs = `<filter id="${id}"><feColorMatrix type="saturate" values="0.1"/><feComponentTransfer><feFuncR type="linear" slope="1.15"/><feFuncG type="linear" slope="1.15"/><feFuncB type="linear" slope="1.15"/></feComponentTransfer></filter>`;
  if (kind === 'tint') defs = `<filter id="${id}"><feColorMatrix type="matrix" values="0.25 0.5 0.1 0 0.38  0.1 0.25 0.05 0 0.05  0.1 0.25 0.05 0 0.1  0 0 0 1 0"/></filter>`;
  if (kind === 'invert') defs = `<filter id="${id}"><feComponentTransfer><feFuncR type="table" tableValues="1 0"/><feFuncG type="table" tableValues="1 0"/><feFuncB type="table" tableValues="1 0"/></feComponentTransfer></filter>`;
  if (kind === 'outline') defs = `<filter id="${id}" x="-20%" y="-20%" width="140%" height="140%"><feMorphology in="SourceAlpha" operator="dilate" radius="12" result="d"/><feFlood flood-color="#222428"/><feComposite in2="d" operator="in" result="o"/><feMerge><feMergeNode in="o"/><feMergeNode in="SourceGraphic"/></feMerge></filter>`;
  if (kind === 'warp') defs = `<filter id="${id}" x="-10%" y="-10%" width="120%" height="120%"><feTurbulence type="fractalNoise" baseFrequency="0.012" numOctaves="2" seed="3"/><feDisplacementMap in="SourceGraphic" scale="70" xChannelSelector="R" yChannelSelector="G"/></filter>`;
  if (kind === 'shadow') defs = `<filter id="${id}" x="-20%" y="-20%" width="160%" height="160%"><feDropShadow dx="26" dy="24" stdDeviation="10" flood-color="#000" flood-opacity=".45"/></filter>`;
  if (kind === 'pixelate') {
    // sample the figure on a coarse grid
    const cell = 30, segs = SEGS.map(([c, par, col, t]) => [p[par], p[c], COLS.A[col], t]), hd = headPos(p, 0);
    const dist = (q, a, b) => { const dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy; let t = l ? ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l : 0; t = Math.max(0, Math.min(1, t)); return Math.hypot(q[0] - a[0] - t * dx, q[1] - a[1] - t * dy); };
    body = '';
    for (let y = 250; y < 655; y += cell) for (let x = 470; x < 1190; x += cell) {
      const q = [x + cell / 2, y + cell / 2];
      let col = null;
      if (Math.hypot(q[0] - hd[0], q[1] - hd[1]) <= HEAD_R) col = COLS.A.skin;
      for (const [a, b, c, t] of segs) if (dist(q, a, b) <= t / 2 + 4) col = c;
      if (Math.hypot(q[0] - hd[0], q[1] - hd[1]) <= HEAD_R) col = COLS.A.skin;
      if (col) body += `<rect x="${x}" y="${y}" width="${cell}" height="${cell}" fill="${col}"/>`;
    }
  }
  return `<svg viewBox="${vb}" width="${TW}" height="${TH}" aria-hidden="true" style="display:block" preserveAspectRatio="xMidYMid slice">${defs ? `<defs>${defs}</defs>` : ''}${body}</svg>`;
}

function panelEffects(L) {
  const x = W - RAIL - 8 - PANEL_W, y = TOP + 8, h = L.areaH - 16;
  const groups = [
    ['Yours', [['warp', 'Warp']]],
    ['Look', [['blur', 'Blur'], ['motion', 'Motion blur'], ['glow', 'Glow'], ['adjust', 'Colour'], ['tint', 'Tint'], ['invert', 'Invert'], ['pixelate', 'Pixelate']]],
    ['Movement', [['wiggle', 'Wiggle'], ['swing', 'Swing'], ['loopmod', 'Loop'], ['follow', 'Follow'], ['link', 'Link']]],
    ['Edges', [['outline', 'Outline'], ['shadow', 'Shadow']]],
  ];
  let s = `<section class="panel" aria-label="Add an effect" style="left:${x}px;top:${y}px;width:${PANEL_W}px;height:${h}px">`;
  s += `<div class="phead"><button type="button" class="pback">${icon('back', 18, C.blue, 2.4)}Properties</button><button type="button" class="pimp">Import…</button></div>`;
  s += `<label class="srch">${icon('search', 18, '#8A9099')}<span>Search effects</span></label>`;
  let n = 0;
  for (const [g, items] of groups) {
    s += `<h2 class="title gt">${g}</h2><div class="tiles">`;
    for (const [k, label] of items) {
      const thumb = g === 'Movement' ? `<span class="mv">${icon(k, 30, C.blue, 1.8)}</span>` : fxThumb(k, `fx${n++}`);
      const tag = g === 'Yours' ? '<span class="ptag">by yeah</span>' : '';
      s += `<button type="button" class="tile">${thumb}<span>${label}</span>${tag}</button>`;
    }
    s += `</div>`;
  }
  s += `<span class="pfade"></span>`;
  return s + '</section>';
}


// ---- scenery used by some boards ----
const CAM_KEYS = [[1, 0, 'hold'], [9, 0, 'tween'], [13, 0, 'hold']];
const TEXT_KEYS = [[9, 0, 'hold'], [14, 0, 'end']];
function SKYLINE(fill = '#DCE3EC') {
  const b = [[0, 640, 180], [170, 560, 140], [300, 690, 200], [490, 600, 150], [630, 710, 220], [840, 540, 130], [960, 650, 240], [1190, 570, 170], [1350, 700, 200], [1540, 610, 150], [1680, 670, 240]];
  return `<g fill="${fill}">${b.map(([x, y, w]) => `<rect x="${x}" y="${y}" width="${w}" height="${1080 - y}"/>`).join('')}</g>`;
}
const POW = () => `<text x="1380" y="260" font-family="Open Sans, sans-serif" font-weight="800" font-size="150" fill="#F2A93B" stroke="#222428" stroke-width="12" paint-order="stroke" stroke-linejoin="round" transform="rotate(-8 1560 200)">POW!</text>`;
function bird(x, y, s, v, colour = '#3B3F46') {
  const w = 46 * s, h = (14 - 40 * v) * s;
  return `<path d="M${r1(x - w)} ${r1(y + h * 0.4)}Q${r1(x - w / 2)} ${r1(y + h)} ${r1(x)} ${r1(y)}Q${r1(x + w / 2)} ${r1(y + h)} ${r1(x + w)} ${r1(y + h * 0.4)}" fill="none" stroke="${colour}" stroke-width="${r1(9 * s)}" stroke-linecap="round" stroke-linejoin="round"/>`;
}
const birdKeys = (i) => { const k = []; for (let f = 1 + ((i - 1) % 2); f <= CELLS; f += 3) k.push([f, (k.length + i) % 2, 'tween']); k[k.length - 1][2] = 'hold'; if (k[0][0] !== 1) k.unshift([1, (i + 1) % 2, 'tween']); return k; };
function birdV(i, f) { const { k, n, t } = locate(birdKeys(i), f); return n ? k[1] + (n[1] - k[1]) * t : k[1]; }
const miniBird = (v) => `<svg viewBox="-60 -50 120 80" width="30" height="20" aria-hidden="true" style="display:block">${bird(0, 0, 1, v)}</svg>`;
function birdsAt(f) { return [[260, 180], [350, 140], [430, 205], [200, 240], [480, 120]].map(([x, y], i) => bird(x + f * 12, y + Math.sin(f * 0.5 + i) * 8, 0.9, 0.5 + 0.5 * Math.sin(f * 1.1 + i * 1.7))).join(''); }
function birdScene(f, opts = {}) {
  const pos = [[640, 430], [980, 330], [1260, 480]];
  return `<g${opts.op != null ? ` opacity="${opts.op}"` : ''}>${pos.map(([x, y], i) => bird(x + f * 6, y, 3.6, birdV(i + 1, f), opts.tint || '#3B3F46')).join('')}</g>`;
}
function selBox(x, y, w, h, u) {
  const hs = u(9);
  const corners = [[x, y], [x + w, y], [x, y + h], [x + w, y + h]].map(([a, b]) => `<rect x="${r1(a - hs / 2)}" y="${r1(b - hs / 2)}" width="${hs}" height="${hs}" rx="${u(2)}" fill="#fff" stroke="${C.blue}" stroke-width="${u(1.5)}"/>`).join('');
  return `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="none" stroke="${C.blue}" stroke-width="${u(1.5)}" stroke-dasharray="${u(5)} ${u(4)}"/>${corners}`;
}
function camOverlay(u) {
  const x = 125, y = 70, w = 1670, h = 940;
  let s = `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="none" stroke="${C.blue}" stroke-width="${u(2.5)}"/>`;
  for (const i of [1, 2]) s += `<line x1="${x + w * i / 3}" y1="${y}" x2="${x + w * i / 3}" y2="${y + h}" stroke="${C.blue}" stroke-opacity=".45" stroke-width="${u(1)}" stroke-dasharray="${u(6)} ${u(5)}"/><line x1="${x}" y1="${y + h * i / 3}" x2="${x + w}" y2="${y + h * i / 3}" stroke="${C.blue}" stroke-opacity=".45" stroke-width="${u(1)}" stroke-dasharray="${u(6)} ${u(5)}"/>`;
  const hs = u(11);
  for (const [a, b] of [[x, y], [x + w, y], [x, y + h], [x + w, y + h]]) s += `<rect x="${r1(a - hs / 2)}" y="${r1(b - hs / 2)}" width="${hs}" height="${hs}" rx="${u(2)}" fill="#fff" stroke="${C.blue}" stroke-width="${u(2)}"/>`;
  return s;
}


// ---- full screens: projects, first run, settings, and a sheet of edge states ----
function screenTop(left, title, right) {
  return `<header class="top" style="height:${TOP}px"><nav class="words" style="left:12px">${left}</nav><div class="name">${title}</div><nav class="words" style="right:12px">${right}</nav></header>`;
}
const wordBtns = (ws) => ws.map((w) => `<button type="button" class="tw">${w}</button>`).join('');
function thumb(f, opts = {}) {
  const prev = BOARD; BOARD = opts.board || {};
  const svg = `<svg viewBox="0 0 1920 1080" width="100%" height="100%" preserveAspectRatio="xMidYMid slice" aria-hidden="true" style="display:block">${scene(f)}</svg>`;
  BOARD = prev; return svg;
}
const WAVE = { hip: [960, 640], neck: [960, 440], head: [960, 370], le: [900, 520], lh: [880, 600], re: [1040, 380], rh: [1080, 290], lk: [920, 790], lf: [900, 940], rk: [1000, 790], rf: [1020, 940] };
const SCREENS = {
  projects: () => {
    const cards = [
      ['Rooftop fight', '24 frames, 2 seconds. Edited today', thumb(9, { board: { extras: { pow: true } } }), true],
      ['Bird flock', '24 frames. Edited today', thumb(6, { board: { clipEdit: true } })],
      ['City run', '48 frames, 4 seconds. Edited yesterday', thumb(3, { board: { extras: { skyline: true } } })],
      ['Waving test', '12 frames. Edited 3 days ago', `<svg viewBox="0 0 1920 1080" width="100%" height="100%" aria-hidden="true" style="display:block">${figure(WAVE, COLS.B)}</svg>`],
      ['Hat trick', '36 frames, 3 seconds. Edited last week', thumb(15)],
      ['Untitled 2', '1 frame. Edited last week', `<svg viewBox="0 0 1920 1080" width="100%" height="100%" aria-hidden="true" style="display:block">${figure(POSES.A[0], { skin: '#222428', shirt: '#222428', pants: '#222428' })}</svg>`],
      ['Dance', '60 frames, 5 seconds. Edited last month', thumb(19)],
    ];
    let g = `<button type="button" class="pcard new"><span class="pth">${icon('plus', 30, C.blue, 2.2)}</span><b>New animation</b></button>`;
    for (const [n, m, t, unsaved] of cards) g += `<button type="button" class="pcard"><span class="pth">${t}</span><b>${n}${unsaved ? '<i class="unsaved" title="Not saved"></i>' : ''}</b><span>${m}</span></button>`;
    return `<div class="bf scr" style="position:relative;width:${W}px;height:${H}px;overflow:hidden">${screenTop('', 'Your animations', wordBtns(['Open a file…', 'Backups', 'Settings']))}<main class="pgrid">${g}</main></div>`;
  },
  firstrun: () => `<div class="bf scr" style="position:relative;width:${W}px;height:${H}px;overflow:hidden">${screenTop('', 'Your animations', wordBtns(['Open a file…', 'Settings']))}
    <main class="empty"><svg viewBox="700 230 520 760" width="150" height="219" aria-hidden="true">${figure(WAVE, COLS.A)}</svg>
    <h1>No animations yet</h1><p>Start one, then pose your first stick figure.</p>
    <button type="button" class="wb pri big">Make an animation</button>
    <div class="hint2">Stick Nodes projects, figures and movieclips (.stknds, .nodes, .nodemc) open here too. Use "Open a file…".</div></main></div>`,
  settings: () => {
    const left = rowsHtml([
      ['title', 'Screen'], ['slider', 'UI size', '100%', 0.2], ['step', 'Screen padding', '0'],
      ['hint', 'Padding keeps controls clear of notches and rounded corners.'],
      ['title', 'Joints'], ['seg', 'Joint style', ['Legacy', 'Modern', 'Solid'], 1], ['step', 'Line width', '2'],
      ['title', 'Saving'], ['step', 'Save reminder (minutes)', '10'], ['tick', 'Keep backups every 5 minutes', true],
    ]);
    const right = rowsHtml([
      ['title', 'This device'], ['info', 'Graphics', 'Intel Iris Xe, Vulkan'], ['info', 'Sound', 'PipeWire'],
      ['title', 'About'], ['info', 'Version', '0.9 (build 214)'],
      ['hint', 'Barkfart makes no network requests: no ads, no tracking, nothing sent anywhere.'],
      ['link', 'Licences', ''], ['link', 'Backups', '3 projects'],
    ]);
    return `<div class="bf scr" style="position:relative;width:${W}px;height:${H}px;overflow:hidden">${screenTop(`<button type="button" class="tw" style="display:flex;align-items:center;gap:2px">${icon('back', 16, C.blue, 2.4)}Back</button>`, 'Settings', '')}
      <main class="setcols"><section class="scard">${left}</section><section class="scard">${right}</section></main></div>`;
  },
  states: () => {
    const missing = `<section class="scard st1" aria-label="Missing plugin">${rowsHtml([['title', 'Effects', 'Add an effect'], ['fx', 'Missing: Warp 1.0', 'by yeah', true], ['hint', 'Hero draws without it until the plugin is imported again. Its settings and keys are kept.'], ['fx', 'Glow', 'strength animated']])}</section>`;
    const backups = `<section class="scard st2" aria-label="Backups">${rowsHtml([['title', 'Backups of Rooftop fight'], ['hint', 'A copy every 5 minutes while you work, kept for 7 days.']])}${[['Today, 14:05', 'Restore'], ['Today, 14:00', 'Restore'], ['Today, 13:55', 'Restore'], ['Yesterday, 18:32', 'Restore']].map(([t, a]) => `<div class="prow"><span class="lab">${t}</span><button type="button" class="elink">${a}</button></div>`).join('')}</section>`;
    const empty = `<section class="scard st3" aria-label="Empty animation"><div class="etl"><div class="enames"></div><div class="emsg">Nothing here yet. Tap + to add a stick figure.</div></div><div class="ebar"><button type="button" class="ib add on" aria-label="Add">${icon('plus', 20, C.blue, 2.2)}</button><span class="hint3">Start here<span class="tail3"></span></span></div></section>`;
    const toasts = `<div class="toast t1">Couldn't open hero.nodes. It isn't a figure file.<button type="button">OK</button></div><div class="toast t2">Cape sway stopped: it took too long on frame 14.<button type="button">Details</button></div><div class="toast t3 light">${icon('broken', 16, '#D91A3A', 2.2)}Hat lets go of Rival at frame 12<button type="button">Undo</button></div>`;
    return `<div class="bf scr states" style="position:relative;width:${W}px;height:${H}px;overflow:hidden">${missing}${backups}${empty}${toasts}</div>`;
  },
};

// ---- pages are lists of rows: one renderer for every page ----
const stepCtl = (v) => `<span class="step"><button type="button" class="sb" aria-label="Less"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12"/></svg></button><b>${v}</b><button type="button" class="sb" aria-label="More"><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="${C.blue}" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 6v12M6 12h12"/></svg></button></span>`;
const tickBox = (on) => on === 'mixed'
  ? `<svg viewBox="0 0 30 30" width="26" height="26" aria-hidden="true"><rect x="1" y="1" width="28" height="28" rx="7" fill="#fff" stroke="${C.blue}" stroke-width="2"/><path d="M9 15h12" stroke="${C.blue}" stroke-width="3.2" stroke-linecap="round"/></svg>`
  : on === 'off' ? tick(false).replace('<svg', '<svg style="opacity:.45"') : tick(on);
const EASES = [['Even', 'M5 27 27 5'], ['Slow end', 'M5 27C10 12 16 5 27 5'], ['Slow both ends', 'M5 27C17 27 15 5 27 5'], ['Bounce', 'M5 27C8 14 12 6 17 5C18.5 9 19.5 9 21 5C22 7 22.8 7 23.6 5H27']];
function rowsHtml(list) {
  return list.map(([k, a, b, c]) => {
    const dot = (x) => (x === 'chg' ? '<i class="chg"></i>' : '');
    if (k === 'title') return `<div class="sect s2"><h2 class="title">${a}</h2>${b ? `<button type="button" class="tadd" aria-label="${b}">${icon('plus', 20, C.blue, 2.2)}</button>` : ''}</div>`;
    if (k === 'tick') return `<div class="prow"><span class="lab">${dot(c)}${a}</span>${tickBox(b)}</div>`;
    if (k === 'step') return `<div class="prow"><span class="lab">${dot(c)}${a}</span>${stepCtl(b)}</div>`;
    if (k === 'seg') return `${a ? `<div class="prow tight"><span class="lab">${a}</span></div>` : ''}<div class="seg segn" style="grid-template-columns:repeat(${b.length},1fr)">${b.map((o, i) => `<button type="button"${i === c ? ' class="on" aria-pressed="true"' : ''}>${o}</button>`).join('')}</div>`;
    if (k === 'pair') return `<div class="pair">${[a, b].map((w) => `<button type="button" class="wb">${w}</button>`).join('')}</div>`;
    if (k === 'one') return `<div class="pair one"><button type="button" class="wb">${a}</button></div>`;
    if (k === 'swatch') return `<div class="prow crow"><span class="lab">${a}${c ? `<span class="mixed">${c}</span>` : ''}</span>${swatchRow(b)}</div>`;
    if (k === 'slider') return `<div class="prow"><span class="lab">${a}</span><b>${b}</b></div><div class="slider"><span style="width:${c * 100}%"></span><i style="left:${c * 100}%"></i></div>`;
    if (k === 'link') return `<div class="prow link"><span class="lab">${a}</span><b>${b} ${icon('next', 14, C.ink2)}</b></div>`;
    if (k === 'text') return `<div class="tfield" role="textbox" aria-label="Text">${a}</div>`;
    if (k === 'hint') return `<p class="phint">${a}</p>`;
    if (k === 'info') return `<div class="prow"><span class="lab">${a}</span><span class="val">${b}</span></div>`;
    if (k === 'name') return `<div class="prow"><b class="fname">${a}</b>${b ? `<button type="button" class="elink">${b}</button>` : ''}</div>`;
    if (k === 'search') return `<label class="srch">${icon('search', 18, '#8A9099')}<span>${a}</span></label>`;
    if (k === 'ease') return `<div class="ease"><span class="lab">Ease</span><span class="et">${EASES.map(([n, d], i) => `<button type="button" class="eb${i === a ? ' on' : ''}" aria-label="${n}"${i === a ? ' aria-pressed="true"' : ''}><svg viewBox="0 0 32 32" width="24" height="24" aria-hidden="true"><path d="${d}" fill="none" stroke="${i === a ? C.blue : C.ink2}" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg></button>`).join('')}</span></div>`;
    if (k === 'fx') return `<div class="fxrow${c ? ' miss' : ''}"><span class="fxn">${a}</span><span class="fxk">${b}</span><button type="button" class="eye" aria-label="Hide ${a}">${icon('eye', 18, '#9AA0A8')}</button></div>`;
    if (k === 'buttons') return `<div class="pair" style="grid-template-columns:repeat(${a.length},1fr)">${a.map((w, i) => `<button type="button" class="wb${i === a.length - 1 ? ' pri' : ''}">${w}</button>`).join('')}</div>`;
    return '';
  }).join('');
}
function page(L, o) {
  const dock = !!o.dock;
  const x = o.x ?? (dock ? W - RAIL - PANEL_W : W - RAIL - 8 - PANEL_W), y = o.y ?? (dock ? TOP : TOP + 8);
  const w = o.w ?? PANEL_W, h = o.h ?? (dock ? L.areaH : L.areaH - 16), fh = o.foot ? 52 : 0;
  let s = `<section class="panel pg${dock ? ' docked' : ''}" aria-label="${o.label || 'Properties'}" style="left:${x}px;top:${y}px;width:${w}px;height:${h}px">`;
  s += `<div class="scroll" style="bottom:${fh}px"><div class="sin"${o.scrolled ? ` style="transform:translateY(-${o.scrolled}px)"` : ''}>`;
  if (o.tabs) s += `<div class="seg" role="tablist" style="grid-template-columns:repeat(${o.tabs.length},1fr)">${o.tabs.map((t, i) => `<button type="button" role="tab"${i === o.active ? ' class="on" aria-selected="true"' : ''}>${t}</button>`).join('')}</div>`;
  s += rowsHtml(o.body) + `</div></div>`;
  if (!o.nofade) s += `<span class="pfade" style="bottom:${fh}px;height:24px"></span>`;
  if (o.foot) s += `<div class="pfoot"><button type="button" class="rall">${o.foot}</button></div>`;
  if (o.nub) s += `<span class="nub2" style="${o.nub}"></span>`;
  return s + '</section>';
}
const FIG_TABS = ['Stickfigure', 'Segment', 'Pose'];
const PAGES = {
  pose: () => ({ tabs: FIG_TABS, active: 2, body: [
    ['title', 'Pose Tools'], ['info', 'Hero', 'frames 5 to 8'],
    ['seg', null, ['Hold until the next pose', 'Tween into it'], 1], ['ease', 1],
    ['step', 'Length in frames', 4], ['pair', 'Copy pose', 'Paste pose'],
  ], foot: 'Delete this pose' }),
  frame: () => ({ body: [
    ['title', `Frame ${CUR}`], ['hint', 'Changes here apply to every layer at this moment.'],
    ['pair', 'Copy frame', 'Paste frame'], ['pair', 'Add time here', 'Remove this frame'],
    ['seg', 'Add an in-between', ['Halfway', 'A third in', 'A tenth in'], 0], ['one', 'Add an in-between frame'],
    ['title', 'Background'], ['swatch', 'Colour', '#FFFFFF'], ['tick', 'Use gradient', false],
    ['title', 'Timing'], ['tick', 'Do not tween this frame', false], ['tick', 'Slow motion', true], ['step', 'In-between pictures', 6],
    ['tick', 'Pause here', false], ['tick', 'Repeat the frames before this', false], ['tick', 'Silence all sounds', false],
  ], foot: 'Clear frame' }),
  camera: () => ({ body: [
    ['title', 'Camera Tools'], ['hint', 'Drag on the stage to move the camera on this frame.'],
    ['pair', 'Copy camera', 'Paste camera'], ['step', 'Zoom', '1.15'], ['step', 'Rotation', '0°'],
    ['pair', 'Center camera', 'Reset'], ['link', 'Auto-camera', 'Off'], ['link', 'Lock a figure to the camera', 'None'],
    ['seg', 'Guide', ['None', 'Thirds', 'Phi', 'Spiral'], 1],
    ['title', 'Effects on this frame'], ['tick', 'Widescreen', false], ['tick', 'Wobble position', false], ['tick', 'Wobble rotation', false],
    ['step', 'Wobble speed', '1.0'], ['tick', 'Scale wobble with zoom', true], ['pair', 'Copy wobble', 'Paste wobble'],
  ] }),
  text: () => ({ body: [
    ['title', 'Text Tools'], ['text', 'POW!'], ['seg', 'Alignment', ['Left', 'Center', 'Right'], 1],
    ['swatch', 'Colour', '#F2A93B'], ['step', 'Scale', '1.50'], ['tick', 'Locked', false], ['tick', 'Locked to camera', false],
    ['hint', 'Locked to camera: it stays put in the shot however the camera moves, for titles and captions.'],
    ['pair', 'Copy text', 'Paste text'],
  ], foot: 'Delete text' }),
  picture: () => ({ body: [
    ['title', 'Picture Tools'], ['link', 'Picture', 'skyline.png'], ['step', 'Scale', '1.00'], ['pair', 'Flip sideways', 'Flip over'],
    ['title', 'Blending'], ['slider', 'Opacity', '100%', 1], ['link', 'Blend', 'Normal'],
    ['tick', 'Locked', true], ['tick', 'Locked to camera', false],
  ], foot: 'Delete picture' }),
  clip: () => ({ body: [
    ['title', 'Movieclip Tools'], ['seg', 'Plays', ['Once', 'Loop', 'Ping-pong'], 1], ['step', 'Speed', '1.00'], ['step', 'Start offset', '0'],
    ['hint', 'Drag the box on the stage to move it.'], ['pair', 'Edit movieclip', 'Break apart'],
    ['title', 'Blending'], ['slider', 'Opacity', '100%', 1],
  ], foot: 'Delete movieclip' }),
  sound: () => ({ body: [
    ['title', 'Sound Tools'], ['link', 'Sound', 'punch.ogg'], ['slider', 'Volume', '80%', 0.8], ['slider', 'Pan', 'Centre', 0.5],
    ['step', 'Pitch', '1.00'], ['step', 'Fade in', '0.0 s'], ['step', 'Fade out', '0.2 s'], ['tick', 'Loop', false],
    ['pair', 'Copy sound', 'Paste sound'],
  ], foot: 'Delete sound' }),
  multi: () => ({ body: [
    ['title', '3 figures'], ['hint', 'Changes here apply to all three.'], ['swatch', 'Colour', '', 'Mixed'],
    ['title', 'Effects', 'Add an effect to all three'], ['hint', 'Hero has Glow. Head and Rival have none.'],
    ['title', 'Blending'], ['slider', 'Opacity', '100%', 1], ['tick', 'Locked', 'mixed'],
    ['pair', 'Group', 'Make movieclip'], ['pair', 'Copy poses', 'Paste poses'],
  ] }),
  figfull: () => ({ tabs: FIG_TABS, active: 0, body: [
    ['name', 'Hero', 'Edit as a new figure…'], ['swatch', 'Colour', COLS.A.shirt],
    ['title', 'Effects', 'Add an effect'], ['fx', 'Glow', `${icon('key', 12, C.blue, 2.4)}strength animated`],
    ['title', 'Blending'], ['slider', 'Opacity', '100%', 1], ['link', 'Blend', 'Normal'],
    ['title', 'Stickfigure Tools'], ['pair', 'Flip sideways', 'Flip over'], ['pair', 'Send back', 'Bring forward'],
    ['step', 'Scale', '1.00'], ['pair', 'Turn 45°', 'Center on stage'], ['seg', 'Dragging a joint', ['Rotates', 'Stretches'], 0],
    ['link', 'Joined to', 'Nothing'], ['tick', 'Locked', false], ['tick', 'Visible in onion skin', true], ['tick', 'Draw order: free', false],
    ['title', 'Tweening'], ['tick', 'Stay on stage between poses', true], ['tick', 'Blend colours between poses', true],
    ['title', 'Copy and paste'], ['pair', 'Copy figure', 'Paste figure'], ['pair', 'Copy properties', 'Paste properties'],
    ['one', 'Convert to movieclip…'],
  ], foot: 'Delete stickfigure' }),
  group: () => ({ body: [
    ['name', 'Fighters', 'Rename'], ['hint', 'A group keeps layers together in the list. Moving its row moves everything in it.'],
    ['title', 'Effects', 'Add an effect to the group'], ['hint', 'No effects yet.'],
    ['title', 'Blending'], ['slider', 'Opacity', '100%', 1], ['tick', 'Draw order: free', false],
    ['hint', 'Off: the group draws as one stack. On: its figures can sit in front of or behind anything.'],
    ['pair', 'Ungroup', 'Make movieclip'],
  ], foot: 'Delete group' }),
};
const VIEW_BODY = [
  ['title', 'Onion skin'], ['tick', 'Onion skin', true], ['step', 'Frames before', 2], ['step', 'Frames after', 1], ['seg', null, ['Behind', 'In front'], 0],
  ['title', 'Handles'], ['tick', 'Only main joints', false], ['tick', 'Static joints', true], ['tick', 'Draw order numbers', false],
  ['seg', 'Highlight', ['None', 'Polyfills', 'Connectors', 'Locks'], 0],
  ['title', 'While editing'], ['seg', 'Effects', ['None', 'No blur', 'All'], 2], ['tick', 'Preview row', true], ['tick', 'Motion path', true],
  ['title', 'Playback'], ['seg', 'Quality', ['Full', 'Half', 'Fast'], 0],
  ['title', 'Helpers'], ['pair', 'Fit stage', 'Nudge arrows'], ['one', 'Magnifier'],
  ['title', 'Running slow?'], ['hint', '12.4 ms per picture. Lower the playback quality, or turn effects off while editing. Memory: 84 MB.'],
];

// ---- sheets ----
function sheet(title, body, w) {
  return `<div class="scrim"></div><section class="sheet2" role="dialog" aria-label="${title}" style="width:${w}px"><h2 class="title st">${title}</h2>${rowsHtml(body)}</section>`;
}
function figTile(p, cols, opts = {}) {
  const xs = Object.values(p).map((q) => q[0]), ys = Object.values(p).map((q) => q[1]);
  const pad = opts.pad || 90, x0 = Math.min(...xs) - pad, x1 = Math.max(...xs) + pad, y0 = Math.min(...ys) - pad, y1 = Math.max(...ys) + 40;
  const hh = y1 - y0, ww = hh * 1.5, cxm = (x0 + x1) / 2;
  return `<svg viewBox="${r1(cxm - ww / 2)} ${r1(y0)} ${r1(ww)} ${r1(hh)}" width="126" height="84" aria-hidden="true" style="display:block">${figure(p, cols)}${opts.hat ? hat(opts.hat) : ''}</svg>`;
}
function chooser() {
  const k = (c) => ({ skin: c, shirt: c, pants: c });
  const items = [
    [POSES.A[0], k('#222428'), 'Stick figure'], [POSES.A[1], COLS.A, 'Runner'], [POSES.A[2], COLS.A, 'Boxer'], [POSES.B[0], COLS.B, 'Blue'],
    [POSES.B[0], COLS.B, 'Hat', { hat: hatOn(1) }], [POSES.A[3], { skin: '#9AA0A8', shirt: '#6B7078', pants: '#4A4F57' }, 'Robot'],
    [POSES.B[1], k('#2A2B30'), 'Ninja'], [POSES.B[2], { skin: '#F2C9A0', shirt: '#1E9E55', pants: '#2A2B30' }, 'Green'], [POSES.A[0], COLS.A, 'Small', { pad: 260 }],
  ];
  let g = `<button type="button" class="ftile new"><span class="fnew">${icon('plus', 26, C.blue, 2.2)}</span><span>New figure</span></button>`;
  for (const [p, c, n, o] of items) g += `<button type="button" class="ftile">${figTile(p, c, o)}<span>${n}</span></button>`;
  return `<div class="scrim"></div><section class="sheet2 chooser" role="dialog" aria-label="Add a stick figure" style="width:760px">
    <h2 class="title st">Add a stick figure</h2>
    <div class="chead"><div class="seg" style="grid-template-columns:1fr 1fr;width:240px"><button type="button" class="on" aria-selected="true">Stock</button><button type="button">Yours</button></div><label class="srch" style="flex:1;margin:0">${icon('search', 18, '#8A9099')}<span>Search figures</span></label></div>
    <div class="fgrid">${g}</div>
    <div class="cfoot"><button type="button" class="wlink">Import from Files…</button><button type="button" class="wlink dim">Paste figure</button></div></section>`;
}

function board(st) {
  if (st.screen) return SCREENS[st.screen](st);
  BOARD = st; SEL = st.sel === undefined ? { row: 'hero', frame: 9 } : st.sel;
  const L = layout(st);
  let s = `<div class="bf" style="position:relative;width:${W}px;height:${H}px;overflow:hidden;background:#fff">`;
  s += `<header class="top" style="height:${TOP}px">`;
  if (st.clipEdit) {
    s += `<button type="button" class="tw abs" style="left:8px">${icon('back', 16, C.blue, 2.4)}Rooftop fight</button>`;
    s += `<div class="name two"><b>Bird flock</b><span>Editing a movieclip</span></div>`;
    s += `<button type="button" class="tw abs strong" style="right:12px">Finish</button>`;
  } else {
    s += `<nav class="words" aria-label="App" style="left:12px">${['Projects', 'View', 'Export'].map((w) => `<button type="button" class="tw${(w === 'View' && st.view) || (w === 'Export' && st.sheet === 'export') ? ' on' : ''}">${w}</button>`).join('')}</nav>`;
    s += `<div class="name">Rooftop fight<i class="unsaved" title="Not saved"></i></div>`;
  }
  s += `</header>`;
  s += `<div class="checker" style="position:absolute;left:${RAIL}px;top:${TOP}px;width:${W - 2 * RAIL}px;height:${L.areaH}px"></div>`;
  s += stageSvg(L, st);
  const mode = st.mode || 'pose';
  s += `<nav class="rail" aria-label="Modes" style="left:0;top:${TOP}px;height:${L.areaH}px;box-shadow:inset -1px 0 0 ${C.line}">`;
  s += railBtn('pose', 'Pose one figure', { active: mode === 'pose', y: 6 }) + railBtn('all', 'Move everything', { active: mode === 'all', y: 54 }) + railBtn('camera', 'Camera', { active: mode === 'camera', y: 102 });
  s += `</nav>`;
  s += `<nav class="rail" aria-label="Edit" style="right:0;top:${TOP}px;height:${L.areaH}px;box-shadow:inset 1px 0 0 ${C.line}">`;
  s += railBtn('undo', 'Undo', { y: 6 }) + railBtn('redo', 'Redo', { y: 54 });
  s += railBtn('props', 'Properties', { pressed: !!st.panel, y: 108 }) + railBtn('bin', 'Delete', { y: 156 }) + railBtn('full', 'Hide the panels', { y: L.areaH - 52 });
  s += `</nav>`;
  s += timeline(st, L);
  if (st.panel === 'props') s += panelProps(L);
  if (st.panel === 'effects') s += panelEffects(L);
  if (st.panel === 'segment') s += panelSegment(L, st.scrolled);
  if (PAGES[st.panel]) s += page(L, { ...PAGES[st.panel](), dock: st.dock, scrolled: st.scrolled });
  if (st.view) s += page(L, { label: 'View', x: 70, y: TOP + 6, w: 330, h: L.areaH - 10, body: VIEW_BODY, nub: 'left:72px;top:-7px' });
  if (st.fps) s += page(L, { label: 'Animation', x: W - 12 - 300, y: H - L.tl - 262, w: 300, h: 254, nofade: true, nub: 'right:53px;bottom:-7px', body: [
    ['title', 'Animation'], ['step', 'Frames per second', FPS], ['step', 'Extra smoothness', 2],
    ['hint', 'In-between pictures added between frames when playing and exporting. Your poses stay the same.'],
    ['tick', 'Loop back to the start', true], ['tick', 'Fullscreen when playing', false],
  ] });
  if (st.menu) s += addMenu();
  if (st.sheet === 'copy') s += `<div class="scrim"></div><section class="sheet" role="dialog" aria-label="Edit as a new figure"><h2 class="title">Editing makes a new figure</h2><p>Hero in this animation won't change. You'll edit a copy, which you can save to your library and add to any frame.</p><div class="pair"><button type="button" class="wb">Cancel</button><button type="button" class="wb pri">Edit a copy</button></div></section>`;
  if (st.sheet === 'export') s += sheet('Export', [
    ['seg', 'Format', ['Video', 'GIF', 'PNG frames'], 0], ['seg', 'Size', ['640', '1280', '1920'], 1], ['info', 'Picture size', '1280 × 720'],
    ['step', 'From frame', 1], ['step', 'To frame', CELLS], ['tick', 'Include sound', true], ['tick', 'Transparent background', 'off'],
    ['tick', 'Show watermark', false], ['tick', 'Make vertical (phone video)', false], ['hint', `2 seconds at ${FPS} fps. Videos can't be transparent; GIF and PNG frames can.`],
    ['buttons', ['Cancel', 'Export']],
  ], 480);
  if (st.sheet === 'unsaved') s += sheet('Save your work?', [['hint', 'Rooftop fight has changes you haven’t saved.'], ['buttons', ['Later', 'Save as…', 'Save']]], 420);
  if (st.sheet === 'chooser') s += chooser();
  return s + '</div>';
}

const CSS = `
body{margin:0}
.bf{font-family:"Open Sans",system-ui,sans-serif;font-size:14px;color:${C.ink};-webkit-font-smoothing:antialiased}
.bf button{border:0;margin:0;padding:0;background:none;color:inherit;font:inherit;cursor:pointer}
.bf button:focus-visible,.blk:focus-visible,.strip:focus-visible{outline:2px solid ${C.blue};outline-offset:2px}
.top{position:absolute;left:0;top:0;right:0;background:#fff;box-shadow:inset 0 -1px 0 ${C.line}}
.words{position:absolute;top:0;height:40px;display:flex;align-items:center;gap:2px}
.tw{height:32px;padding:0 12px!important;border-radius:9px;font-size:15px!important;font-weight:600!important;color:${C.blue}!important}
.name{position:absolute;left:0;right:0;top:0;height:40px;display:flex;align-items:center;justify-content:center;font-weight:600;font-size:15px;pointer-events:none}
.checker{background-color:#6C6F74;background-image:conic-gradient(#606368 25%,transparent 0 50%,#606368 0 75%,transparent 0);background-size:16px 16px}
.rail{position:absolute;width:64px;background:${C.rail}}
.rb{position:absolute;left:10px;width:44px;height:44px;border-radius:11px;display:flex;align-items:center;justify-content:center}
.rb.on{background:#fff!important;box-shadow:0 0 0 1px ${C.line},0 1px 2px rgba(0,0,0,.06)}
.sw{width:24px;height:24px;border-radius:50%;background:${COLS.A.shirt};box-shadow:0 0 0 2px #fff,0 0 0 3px #C9CED5}
.tl{position:absolute;left:0;right:0;background:#fff;box-shadow:inset 0 1px 0 ${C.line};overflow:hidden}
.tb{position:absolute;left:0;right:0;top:0;box-shadow:inset 0 -1px 0 #E6E8EB;background:#fff;z-index:3}
.grp{position:absolute;top:0;height:${TB}px;display:flex;align-items:center;gap:2px}
.grp.mid{left:50%;transform:translateX(-50%)}
.ib{width:34px;height:28px;border-radius:8px;display:flex;align-items:center;justify-content:center;flex:none}
.ib.on{background:#E8F0FA!important}
.fx{font-size:12px!important;font-weight:800!important;color:${C.blue}!important;letter-spacing:.02em}
.fps{height:28px;padding:0 10px!important;border-radius:8px;font-size:13px!important;color:${C.ink2}!important}
.names{position:absolute;left:0;background:${C.rail};box-shadow:inset -1px 0 0 ${C.line};z-index:2}
.rh{box-shadow:inset 0 -1px 0 #E6E8EB}
.pvrow{display:flex;align-items:center;padding:0 8px 0 36px;box-sizing:border-box;font-size:12px;color:${C.ink2};box-shadow:inset 0 -1px 0 #DADDE2}
.pvrow .eye{margin-left:auto;width:26px;height:22px;display:flex;align-items:center;justify-content:center}
.jm{position:absolute;width:18px;height:18px;border-radius:50%!important;background:rgba(255,255,255,.8)!important;display:flex;align-items:center;justify-content:center;opacity:.75;z-index:1}
.jm.br{background:rgba(255,236,238,.85)!important}
.nrow{position:relative;box-sizing:border-box;box-shadow:inset 0 -1px 0 #E6E8EB;overflow:hidden;font-size:15px}
.nrow.on{background:#E6F0FB;color:${C.blue};font-weight:600}
.nrow.prop{background:#FAFBFC}
.nrow.on + .nrow.prop{background:#F0F6FD}
.nin{position:absolute;left:0;top:0;bottom:0;width:${NAME_W}px;box-sizing:border-box;padding-right:8px;display:flex;align-items:center;gap:7px;background:inherit}
.nin svg{flex:none}
.nrow .sub{font-size:12px;color:${C.ink2};font-weight:400;white-space:nowrap}
.pn{font-size:12px;color:${C.ink2};font-weight:400}
.nrow .eye,.nrow .px{margin-left:auto;width:26px;height:22px;display:flex;align-items:center;justify-content:center}
.dis{width:20px;height:20px;display:flex;align-items:center;justify-content:center;flex:none}
.jn{position:absolute;left:17px;top:-12px;width:10px;height:26px;border-left:1.5px solid #B8BDC4;border-bottom:1.5px solid #B8BDC4;border-bottom-left-radius:6px}
.acts{position:absolute;right:0;top:0;bottom:0;display:flex;box-shadow:-6px 0 10px -6px rgba(0,0,0,.25)}
.acts button{height:100%;padding:0 12px!important;font-size:12px!important;font-weight:600!important}
.dup{background:${C.field}!important;color:${C.blue}!important}
.del{background:#E5484D!important;color:#fff!important}
.ruler{position:absolute;right:0}
.tk{position:absolute;bottom:0;width:1px;height:4px;background:#C4C8CE}
.tk.big{height:6px;background:#9AA0A8}
.rn{position:absolute;top:3px;width:40px;text-align:center;font-size:11px;color:${C.ink2};font-variant-numeric:tabular-nums}
.strip{position:absolute;border-radius:5px;overflow:hidden;background:#fff;box-shadow:inset 0 0 0 1px ${C.line};cursor:pointer}
.strip svg{border-right:1px solid #fff;box-sizing:content-box}
.row{position:absolute;left:0;right:0;box-shadow:inset 0 -1px 0 #EEF0F2}
.row.prop{background:#FAFBFC}
.blk{position:absolute;height:26px;border-radius:5px;background:#EEF0F3;box-shadow:inset 0 0 0 1px #DCDFE4;box-sizing:border-box;cursor:pointer;overflow:hidden}
.blk.sel{background:#E4EEFA;box-shadow:inset 0 0 0 2px ${C.blue}}
.blk.snd{background:#E9F2F7;box-shadow:inset 0 0 0 1px #C9DDE8}
.pic{position:absolute;top:2px;width:${CELL}px;display:flex;justify-content:center}
.hd{position:absolute;top:5px;width:4px;height:14px;border-radius:2px;background:${C.blue}}
.kd{position:absolute;width:12px;height:12px;transform:rotate(45deg) scale(.82);border-radius:2px!important;background:#8A9099!important}
.ph{position:absolute;width:1.5px;background:#8DB8E8;z-index:1;pointer-events:none}
.pill{position:absolute;width:28px;height:18px;border-radius:9px;background:#DCEAF9;color:${C.blue};font-size:11px;font-weight:700;line-height:18px;text-align:center;z-index:2}
.bar{position:absolute;left:0;right:0;bottom:0;background:#fff;box-shadow:inset 0 1px 0 ${C.line};display:flex;align-items:center;gap:2px;padding:0 8px;z-index:3}
.add.on{background:${C.field}!important}
.bw{margin-left:auto;display:flex;gap:2px}
.bw button{height:28px;padding:0 12px!important;border-radius:8px;color:${C.blue}!important;font-weight:600!important;font-size:14px!important}
.menu{position:absolute;width:220px;background:#fff;border-radius:14px;padding:6px;box-sizing:border-box;box-shadow:0 0 0 1px rgba(0,0,0,.08),0 10px 28px rgba(16,20,26,.3);z-index:6}
.mi{display:flex;align-items:center;gap:12px;width:100%;height:40px;padding:0 10px!important;border-radius:9px;font-size:15px!important;text-align:left}
.mi:first-child{background:${C.field}!important}
.menu hr{border:0;height:1px;background:#E6E8EB;margin:5px 8px}
.mnub{position:absolute;left:19px;bottom:-7px;width:14px;height:14px;background:#fff;transform:rotate(45deg);border-radius:2px}
.panel{position:absolute;background:#fff;border-radius:16px;box-shadow:0 0 0 1px rgba(0,0,0,.08),0 10px 28px rgba(16,20,26,.3);padding:10px 14px;box-sizing:border-box;overflow:hidden;z-index:5}
.seg{display:grid;grid-template-columns:repeat(3,1fr);padding:3px;border-radius:11px;background:${C.field}}
.seg button{height:30px;border-radius:8px;color:${C.blue}!important;font-weight:600!important;font-size:13px!important}
.seg button.on{background:#fff!important;box-shadow:0 1px 3px rgba(0,0,0,.14)}
.sect{position:relative;height:34px;display:flex;align-items:center;justify-content:center;margin-top:6px}
.title{margin:0;font-size:15px;font-weight:600;color:${C.red};text-align:center;line-height:20px}
.tadd{position:absolute;right:0;top:3px;width:32px;height:28px;border-radius:8px;display:flex;align-items:center;justify-content:center}
.fxrow{display:flex;align-items:center;gap:10px;height:40px;padding:0 6px 0 12px;border-radius:10px;background:${C.field}}
.fxn{font-weight:600}
.fxk{display:flex;align-items:center;gap:4px;font-size:12px;color:${C.ink2}}
.fxrow .eye{margin-left:auto;width:30px;height:30px;display:flex;align-items:center;justify-content:center}
.prow{display:flex;align-items:center;justify-content:space-between;height:34px}
.prow b{font-weight:600;font-variant-numeric:tabular-nums;display:flex;align-items:center;gap:4px}
.slider{position:relative;height:4px;border-radius:2px;background:#E1E4E8;margin:2px 4px 8px}
.slider span{position:absolute;left:0;top:0;bottom:0;border-radius:2px;background:${C.blue}}
.slider i{position:absolute;top:50%;width:18px;height:18px;margin:-9px 0 0 -9px;border-radius:50%;background:#fff;box-shadow:0 0 0 1px #C9CED5,0 1px 3px rgba(0,0,0,.18)}
.crow{height:40px}
.sws{display:flex;gap:6px}
.swb{width:22px;height:22px;border-radius:50%!important;box-shadow:inset 0 0 0 1px rgba(0,0,0,.18)}
.swb.on{box-shadow:0 0 0 2px #fff,0 0 0 4px ${C.blue}}
.swb.more{background:conic-gradient(#E5484D,#F2A93B,#FFD54A,#1E9E55,#2D63E0,#A534D0,#E5484D)!important}
.sub2{justify-content:flex-end;height:34px;margin-top:-6px}
.pair{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:2px 0 4px}
.wb{height:36px;border-radius:10px;background:${C.field}!important;color:${C.blue}!important;font-weight:600!important}
.step{display:flex;align-items:center;gap:6px}
.step b{width:36px;text-align:center;font-size:15px;font-variant-numeric:tabular-nums;justify-content:center}
.sb{width:32px;height:32px;border-radius:9px;background:${C.field}!important;display:flex;align-items:center;justify-content:center}
.seg-page{padding:0}
.seg-page .scroll{position:absolute;left:0;right:0;top:0;bottom:52px;overflow:hidden;padding:10px 14px;box-sizing:border-box}
.sect.s2{height:30px;margin-top:8px}
.lab{display:flex;align-items:center;gap:6px}
.chg{display:block;width:6px;height:6px;border-radius:50%;background:${C.blue}}
.prow.swiped{position:relative;overflow:hidden;margin:0 -14px;padding:0}
.swin{display:flex;align-items:center;justify-content:space-between;height:34px;padding:0 14px;transform:translateX(-112px);width:100%;box-sizing:border-box;background:#fff}
.rst{position:absolute;right:0;top:0;bottom:0;width:112px;background:${C.field}!important;color:${C.blue}!important;font-weight:600!important;font-size:13px!important;box-shadow:inset 1px 0 0 #E1E4E8}
.seg3{margin:0 0 4px}
.pfoot{position:absolute;left:0;right:0;bottom:0;height:52px;padding:8px 14px;box-sizing:border-box;box-shadow:inset 0 1px 0 #E6E8EB;background:#fff}
.rall{width:100%;height:36px;border-radius:10px;background:#FDECEE!important;color:#C8243A!important;font-weight:700!important}
.fname{font-size:15px}
.elink{height:30px;color:${C.blue}!important;font-weight:600!important;font-size:13px!important}
.scrim{position:absolute;inset:0;background:rgba(20,24,30,.38);z-index:8}
.sheet{position:absolute;left:50%;top:50%;width:400px;transform:translate(-50%,-50%);background:#fff;border-radius:18px;padding:20px 20px 18px;box-sizing:border-box;box-shadow:0 18px 48px rgba(0,0,0,.35);z-index:9;text-align:center}
.sheet .title{font-size:17px;margin-bottom:8px}
.sheet p{margin:0 0 16px;font-size:14px;line-height:20px;color:${C.ink}}
.wb.pri{background:${C.blue}!important;color:#fff!important}
.pg{padding:0}
.pg .scroll{position:absolute;left:0;right:0;top:0;overflow:hidden;padding:10px 14px;box-sizing:border-box}
.panel.docked{border-radius:0;box-shadow:inset 1px 0 0 ${C.line}}
.segn{margin:0 0 6px}
.prow.tight{height:24px}
.val{color:${C.ink2};font-size:13px}
.mixed{font-size:11px;color:${C.ink2};margin-left:6px}
.tfield{height:38px;border-radius:10px;background:${C.field};display:flex;align-items:center;padding:0 12px;font-size:15px;font-weight:600;margin:2px 0 6px}
.phint{margin:2px 0 8px;font-size:12px;line-height:17px;color:${C.ink2}}
.pair.one{grid-template-columns:1fr}
.fxrow.miss{background:#FFF4E5}
.ease{display:flex;align-items:center;justify-content:space-between;height:46px}
.et{display:flex;gap:6px}
.eb{width:38px;height:36px;border-radius:9px;background:${C.field}!important;display:flex;align-items:center;justify-content:center}
.eb.on{background:#fff!important;box-shadow:inset 0 0 0 2px ${C.blue}}
.nub2{position:absolute;width:14px;height:14px;background:#fff;transform:rotate(45deg);border-radius:2px}
.tk.sec{height:9px;background:#4A4F57}
.rn.sec{color:${C.ink};font-weight:700}
.gbar{position:absolute;height:12px;border-radius:6px;background:#E6E9ED;box-shadow:inset 0 0 0 1px #D6DAE0;cursor:pointer}
.gbar.sel{background:#E4EEFA;box-shadow:inset 0 0 0 2px ${C.blue}}
.gbar i{position:absolute;top:3px;width:2px;height:6px;border-radius:1px;background:#8A9099}
.btxt{font-size:11px;font-weight:800;color:#C77A12;margin-top:4px}
.lasso{position:absolute;border:1.5px dashed ${C.blue};border-radius:6px;background:rgba(21,119,214,.06);z-index:2;pointer-events:none}
.fsel{position:absolute;border-radius:4px;box-shadow:inset 0 0 0 2px ${C.blue};z-index:2}
.tw.abs{position:absolute;top:4px;display:flex;align-items:center;gap:2px}
.tw.on{background:${C.field}!important}
.name.two{flex-direction:column;line-height:16px;font-size:13px}
.name.two b{font-size:15px;font-weight:600}
.name.two span{font-size:12px;color:${C.ink2};font-weight:400}
.unsaved{display:inline-block;width:7px;height:7px;border-radius:50%;background:#F2A93B;margin-left:7px}
.sheet2{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);background:#fff;border-radius:18px;padding:16px 20px 18px;box-sizing:border-box;box-shadow:0 18px 48px rgba(0,0,0,.35);z-index:9}
.sheet2 .st{font-size:17px;margin:0 0 8px;text-align:center}
.sheet2 .prow{height:34px}
.chead{display:flex;gap:12px;align-items:center;margin-bottom:12px}
.chead .seg{margin:0}
.fgrid{display:grid;grid-template-columns:repeat(5,1fr);gap:12px 10px}
.ftile{display:flex;flex-direction:column;align-items:center;gap:4px;font-size:12px!important}
.ftile > svg,.fnew{border-radius:10px;box-shadow:0 0 0 1px ${C.line};background:#fff}
.fnew{width:126px;height:84px;display:flex;align-items:center;justify-content:center;box-shadow:inset 0 0 0 1.5px #C9CED5;background:#FAFBFC}
.ftile.new span:last-child{color:${C.blue};font-weight:600}
.cfoot{display:flex;justify-content:space-between;margin-top:14px;padding-top:10px;box-shadow:inset 0 1px 0 #E6E8EB}
.wlink{height:30px;color:${C.blue}!important;font-weight:600!important}
.wlink.dim{color:#AEB3BA!important}
.blk.snd.sel{background:#E4EEFA;box-shadow:inset 0 0 0 2px ${C.blue}}
.scr{background:${C.rail}}
.pgrid{position:absolute;left:0;right:0;top:40px;bottom:0;padding:24px 32px;display:grid;grid-template-columns:repeat(4,1fr);gap:22px 20px;align-content:start;overflow:hidden}
.pcard{display:flex;flex-direction:column;align-items:flex-start;gap:3px;text-align:left}
.pcard .pth{width:100%;aspect-ratio:16/9;border-radius:12px;overflow:hidden;background:#fff;box-shadow:0 0 0 1px ${C.line},0 1px 3px rgba(0,0,0,.06);display:flex;align-items:center;justify-content:center;margin-bottom:6px}
.pcard b{font-size:15px;font-weight:600;display:flex;align-items:center}
.pcard span:last-child{font-size:12px;color:${C.ink2}}
.pcard.new .pth{background:#FAFBFC;box-shadow:inset 0 0 0 1.5px #C9CED5}
.pcard.new b{color:${C.blue}}
.empty{position:absolute;left:0;right:0;top:40px;bottom:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:6px;text-align:center}
.empty h1{margin:8px 0 0;font-size:20px;font-weight:600}
.empty p{margin:0 0 10px;color:${C.ink2}}
.wb.big{height:44px;padding:0 28px!important;border-radius:12px;font-size:16px!important}
.hint2{margin-top:18px;max-width:420px;background:${C.hint};border-radius:10px;padding:8px 12px;font-size:13px;line-height:18px;box-shadow:0 2px 6px rgba(0,0,0,.12)}
.setcols{position:absolute;left:0;right:0;top:40px;bottom:0;padding:20px 32px;display:grid;grid-template-columns:1fr 1fr;gap:20px;align-items:start}
.scard{box-sizing:border-box;background:#fff;border-radius:16px;box-shadow:0 0 0 1px rgba(0,0,0,.06),0 2px 8px rgba(16,20,26,.08);padding:6px 18px 12px}
.states{background:#E9EBEE}
.states .scard{position:absolute}
.st1{left:32px;top:32px;width:360px}
.st2{left:420px;top:32px;width:360px}
.st3{left:808px;top:32px;width:504px;height:250px;padding:0;overflow:hidden}
.etl{position:absolute;left:0;right:0;top:0;bottom:44px;display:flex}
.enames{width:150px;background:${C.rail};box-shadow:inset -1px 0 0 ${C.line}}
.emsg{flex:1;display:flex;align-items:center;justify-content:center;color:${C.ink2};font-size:14px}
.ebar{position:absolute;left:0;right:0;bottom:0;height:44px;box-shadow:inset 0 1px 0 ${C.line};display:flex;align-items:center;padding:0 8px}
.hint3{position:relative;margin-left:16px;background:${C.hint};border-radius:9px;padding:5px 10px;font-size:13px;box-shadow:0 2px 6px rgba(0,0,0,.12)}
.tail3{position:absolute;left:-5px;top:50%;width:10px;height:10px;margin-top:-5px;background:${C.hint};transform:rotate(45deg)}
.toast{position:absolute;display:flex;align-items:center;gap:12px;background:#2A2D33;color:#fff;border-radius:12px;padding:10px 10px 10px 16px;font-size:14px;box-shadow:0 6px 18px rgba(0,0,0,.25)}
.toast button{height:30px;padding:0 12px!important;border-radius:8px;color:#9CC8F5!important;font-weight:700!important}
.toast.light{background:#fff;color:${C.ink};box-shadow:0 0 0 1px rgba(0,0,0,.08),0 6px 18px rgba(0,0,0,.15)}
.toast.light button{color:${C.blue}!important}
.t1{left:808px;top:318px}.t2{left:808px;top:384px}.t3{left:808px;top:450px}
.pfade{position:absolute;left:0;right:0;bottom:0;height:36px;background:linear-gradient(rgba(255,255,255,0),#fff);pointer-events:none}
.phead{height:34px;display:flex;align-items:center;justify-content:space-between}
.pimp{height:30px;padding:0 4px!important;color:${C.blue}!important;font-weight:600!important}
.ptag{font-size:11px;color:${C.ink2};margin-top:-3px}
.pback{display:flex;align-items:center;gap:2px;height:30px;color:${C.blue}!important;font-weight:600!important;margin-left:-6px!important}
.srch{display:flex;align-items:center;gap:8px;height:36px;padding:0 10px;border-radius:10px;background:${C.field};color:#8A9099;font-size:14px;margin-bottom:4px}
.gt{margin:10px 0 6px}
.tiles{display:grid;grid-template-columns:repeat(3,1fr);gap:10px 8px}
.tile{display:flex;flex-direction:column;align-items:center;gap:3px;font-size:12px!important;color:${C.ink}!important}
.tile > svg,.tile > .mv{border-radius:8px;box-shadow:0 0 0 1px ${C.line};background:#fff}
.mv{width:${TW}px;height:${TH}px;display:flex;align-items:center;justify-content:center}
`;

const FONT = '<link href="https://fonts.googleapis.com/css2?family=Open+Sans:wght@400;600;700;800&amp;display=swap" rel="stylesheet">';
function write(st, file, title) {
  const body = board(st);
  const dc = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${title}</title>
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
  const preview = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Preview</title>${FONT}<style>${CSS}</style></head><body style="background:#888">${body}</body></html>`;
  mkdirSync(join(HERE, 'preview'), { recursive: true });
  writeFileSync(join(HERE, 'project', file + '.dc.html'), dc);
  writeFileSync(join(HERE, 'preview', file.toLowerCase() + '.html'), preview);
  console.log('ok', file, dc.length, 'bytes');
}
const SZ = (st) => st.size || [1344, 600];
function out(st, file, title) { [W, H] = SZ(st); write(st, file, title); }
out({ panel: 'props' }, 'Timeline', 'Barkfart editor, timeline and properties');
out({ panel: 'props', sheet: 'copy' }, 'EditCopy', 'Barkfart editor, editing makes a copy');
out({ menu: true }, 'AddMenu', 'Barkfart editor, the add menu');
out({ panel: 'effects', folded: true }, 'Effects', 'Barkfart editor, adding an effect');
out({ panel: 'segment', folded: true }, 'SegmentPage', 'Barkfart editor, segment properties');
out({ panel: 'segment', folded: true, scrolled: 300 }, 'SegmentPage2', 'Barkfart editor, segment properties, scrolled');
out({ panel: 'pose', rows: [{ id: 'hero' }, { id: 'head' }, { id: 'rival' }], sel: { row: 'hero', frame: 5 } }, 'PosePage', 'Pose page');
out({ panel: 'frame', folded: true, sel: null, frameSel: true }, 'FramePage', 'Frame page');
out({ panel: 'figfull', folded: true, scrolled: 520 }, 'FigurePage', 'Stickfigure page, every control');
out({ panel: 'camera', folded: true, mode: 'camera', sel: null, camFrame: true, noHandles: true }, 'CameraPage', 'Camera page');
out({ panel: 'text', rows: [{ id: 'text' }, { id: 'hero' }, { id: 'rival' }], extras: { pow: true }, sel: { row: 'text', frame: 9 }, selText: true, noHandles: true }, 'TextPage', 'Text page');
out({ panel: 'picture', rows: [{ id: 'hero' }, { id: 'rival' }, { id: 'picture' }], extras: { skyline: true }, sel: { row: 'picture', frame: 1 }, selPic: true, noHandles: true }, 'PicturePage', 'Picture page');
out({ panel: 'clip', rows: [{ id: 'clip' }, { id: 'hero' }, { id: 'rival' }], extras: { birds: true }, sel: { row: 'clip', frame: 1 }, selClip: true, noHandles: true }, 'ClipPage', 'Movieclip page');
out({ panel: 'sound', rows: [{ id: 'hero' }, { id: 'rival' }, { id: 'sound' }], sel: { row: 'sound', frame: 9 }, noHandles: true }, 'SoundPage', 'Sound page');
out({ panel: 'multi', multi: ['hero', 'head', 'rival'], rows: [{ id: 'hero' }, { id: 'head' }, { id: 'rival' }, { id: 'hat' }, { id: 'sound' }], sel: null, noHandles: true }, 'MultiPage', 'Two figures selected');
out({ panel: 'group', rows: [{ id: 'group', open: true }, { id: 'hero', indent: 1 }, { id: 'head', indent: 1 }, { id: 'rival', indent: 1 }, { id: 'hat' }], sel: { row: 'group' }, noHandles: true }, 'GroupPage', 'A group');
out({ clipEdit: true, rows: [{ id: 'bird1' }, { id: 'bird2' }, { id: 'bird3' }], sel: { row: 'bird1', frame: 10 } }, 'ClipEdit', 'Editing a movieclip');
out({ view: true }, 'ViewPanel', 'View options');
out({ fps: true }, 'AnimationPopover', 'Animation settings');
out({ sheet: 'export' }, 'ExportSheet', 'Export');
out({ sheet: 'unsaved' }, 'UnsavedSheet', 'Save your work?');
out({ sheet: 'chooser' }, 'FigureChooser', 'Add a stick figure');
out({ size: [1344, 940], dock: true, panel: 'figfull', rows: [{ id: 'camera' }, { id: 'hero', open: true }, { id: 'glow', h: 22 }, { id: 'head' }, { id: 'rival' }, { id: 'hat' }, { id: 'sound' }] }, 'Tablet', 'Tablet layout');
out({ screen: 'projects' }, 'Projects', 'Your animations');
out({ screen: 'firstrun' }, 'FirstRun', 'First run');
out({ screen: 'settings' }, 'Settings', 'Settings');
out({ screen: 'states' }, 'States', 'Edge states');
