// Script layer board (SCRIPTING.md "Script layers"): a Script row in the
// timeline (its blocks are runs, each with its own seed), and its page in
// Properties: what it moves, its settings, how the run mixes in, Bake.
// Built on the timeline board (run `node boards.mjs` first); writes
// project/ScriptLayer.dc.html and preview/script.html.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const C = { ink: '#222428', ink2: '#6B7078', ink2b: '#4A4F57', blue: '#1577D6', red: '#CC1736', line: '#DADDE2', field: '#EEF0F3' };
let html = readFileSync(join(HERE, 'preview/timeline.html'), 'utf8');

const NAME_W = 224, CELL = 46;
const r1 = (v) => Math.round(v * 10) / 10;
const svg = (w, h, body, extra = '') => `<svg viewBox="0 0 24 24" width="${w}" height="${h}" fill="none" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"${extra}>${body}</svg>`;

// ---- the names column: the sound row becomes the Script row ----
const scriptIcon = svg(20, 20, '<path d="M8.5 5.5C6.5 5.5 6.5 7 6.5 9s-1.5 3-2.5 3c1 0 2.5 1 2.5 3s0 3.5 2 3.5M15.5 5.5c2 0 2 1.5 2 3.5s1.5 3 2.5 3c-1 0-2.5 1-2.5 3s0 3.5-2 3.5"/>', ` stroke="${C.blue}" stroke-width="2"`);
const eye = svg(18, 18, '<path d="M2.5 12S6 5.8 12 5.8 21.5 12 21.5 12 18 18.2 12 18.2 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>', ` stroke="#9AA0A8" stroke-width="2"`);
const nameRow = `<div class="nrow on" style="height:32px"><div class="nin" style="padding-left:8px"><span class="dis"></span>${scriptIcon}<span>Cape sway</span><span class="sub">Hero</span><button type="button" class="eye" aria-label="Hide Cape sway">${eye}</button></div></div>`;
{
  const i = html.indexOf('<span>Punch</span>');
  const a = html.lastIndexOf('<div class="nrow"', i);
  const b = html.indexOf('</div></div>', i) + '</div></div>'.length;
  html = html.slice(0, a) + nameRow + html.slice(b);
}

// the script is what's selected now, not Hero
html = html.replace('<div class="nrow on"', '<div class="nrow"');
html = html.replace(nameRow.replace('<div class="nrow on"', '<div class="nrow"'), nameRow);

// nor any of Hero's poses (the base board's selected block and its grips)
html = html.replace(/<span class="hd"[^>]*><\/span>/g, '').replaceAll('class="blk sel"', 'class="blk"');

// offstage is a quiet grey since 2026-11-24 (no checkerboard)
html = html.replace(/\.checker\{[^}]*\}/, '.checker{background-color:#E9EBEE}');

// ---- the timeline: two runs, each a block with a trace of what it does ----
function run(a, b, seed, sel) {
  const left = NAME_W + 1 + (a - 1) * CELL, width = (b - a + 1) * CELL - 2;
  // the cape's swing over the run: damped, different for each seed
  let d = '';
  for (let x = 0; x <= width - 16; x += 2) {
    const u = x / (width - 16);
    const y = 10 - 7 * Math.exp(-2.2 * u) * Math.sin(u * (9 + seed * 2) + seed);
    d += `${d ? 'L' : 'M'}${r1(8 + x)} ${r1(y)}`;
  }
  const grips = sel ? `<span class="hd" style="left:3px"></span><span class="hd" style="right:3px"></span>` : '';
  return `<div class="blk${sel ? ' sel' : ''}" style="left:${left}px;width:${width}px;top:3px" aria-label="Run ${seed}, frames ${a} to ${b}"><svg viewBox="0 0 ${width} 20" width="${width}" height="20" aria-hidden="true" style="position:absolute;left:0;top:3px"><path d="${d}" fill="none" stroke="${sel ? C.blue : '#8A9099'}" stroke-width="1.6"/></svg>${grips}</div>`;
}
{
  const k = html.indexOf('<div class="blk snd"');
  const rowStart = html.lastIndexOf('<div class="row"', k);
  const rowEnd = html.indexOf('</div></div>', html.indexOf('</svg>', k)) + '</div></div>'.length;
  html = html.slice(0, rowStart) + `<div class="row" style="top:210px;height:32px">${run(3, 11, 1, true)}${run(15, 22, 2, false)}</div>` + html.slice(rowEnd);
}

// ---- Properties: the Script page ----
const pm = (d) => svg(18, 18, `<path d="${d}"/>`, ` stroke="${C.blue}" stroke-width="2.4"`);
const tick = (label, on) => `<div class="prow"><span>${label}</span><svg viewBox="0 0 30 30" width="26" height="26" aria-hidden="true"><rect x="1" y="1" width="28" height="28" rx="7" fill="${on ? C.blue : '#fff'}" stroke="${on ? C.blue : '#C9CED5'}" stroke-width="1.5"/><path d="M8.5 15.5l4.4 4.4 8.6-9.4" fill="none" stroke="${on ? '#fff' : '#D9DDE2'}" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"/></svg></div>`;
const slider = (label, value, f) => `<div class="prow"><span>${label}</span><b>${value}</b></div><div class="slider"><span style="width:${f * 100}%"></span></div>`;
const link = (label, value) => `<div class="prow"><span>${label}</span><b>${value} ${svg(14, 14, '<path d="m9 6 6 6-6 6"/>', ` stroke="${C.ink2}" stroke-width="2.2"`)}</b></div>`;
const title = (t) => `<div class="sect s2"><h2 class="title">${t}</h2></div>`;
let panel = `<section class="panel" aria-label="Script" style="left:900px;top:44px;width:372px;height:536px;padding-bottom:0">`;
panel += `<div style="position:absolute;inset:10px 14px 52px 14px;overflow:hidden">`;
panel += `<div class="prow" style="height:30px"><b style="font-size:15px">Cape sway</b><span style="font-size:12px;color:${C.ink2}">by yeah</span></div>`;
// a figure target, and a value target: any number that can be keyed (a
// segment's length, an effect's or a plugin effect's setting)
panel += title('Moves') + link('Cape', 'Hero') + link('Glow', 'Hero › Glow strength');
panel += title('Settings') + slider('Stiffness', '0.40', 0.4);
panel += title('This run') + `<p class="phint">Frames 3 to 11, with its own seed. Drag the block's ends to change them.</p>` + slider('Mix', '100%', 1) + tick('Replaces the keys', false) + tick('Keeps the last frame', false);
panel += `<div class="pair" style="margin-top:6px"><button type="button" class="wb">Bake to keys</button><button type="button" class="wb">New seed</button></div>`;
panel += `</div><span class="pfade" style="bottom:52px;height:24px"></span>`;
panel += `<div class="pfoot"><button type="button" class="rall">Delete script</button></div></section>`;
html = html.replace(/<section class="panel"[\s\S]*?<\/section>/, panel);

mkdirSync(join(HERE, 'preview'), { recursive: true });
mkdirSync(join(HERE, 'project'), { recursive: true });
writeFileSync(join(HERE, 'preview/script.html'), html);
writeFileSync(join(HERE, 'project/ScriptLayer.dc.html'), html);
console.log('wrote preview/script.html');
