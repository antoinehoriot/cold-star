pub(crate) const CSS: &str = "
@import url('https://fonts.googleapis.com/css2?family=Saira+Condensed:wght@300;500;600;700&family=IBM+Plex+Mono:wght@300;400;500&display=swap');

/* ============================================================
   EMBERFALL — Cold Star design system
   --charge : 0..1  reactor health  → interface LIFE
   --tier   : 0..3  expansion       → interface TEMPERATURE
   ============================================================ */
:root {
  color-scheme: dark;
  --charge: 1;
  --tier: 0;
  --accent: #e89255;
  --accent-soft: #e8925533;
  --text: #d9cdbf;
  --warm: 22;
  --void: #06090d;
  --fill: #0e151c;
  --fill-2: #0a1117;
  --line: #1b2a34;
  --line-soft: #14202a;
  --muted: #5f7585;
  --fail: #d66f6f;
  --good: #6fd6a0;
  --r: 3px;
  --display: 'Saira Condensed', 'IBM Plex Mono', sans-serif;
}
*, *::before, *::after { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body {
  background:
    radial-gradient(120% 80% at 50% -10%, hsl(var(--warm) 30% 9% / .9), transparent 60%),
    var(--void);
  color: var(--text);
  font-family: 'IBM Plex Mono', ui-monospace, Menlo, monospace;
  font-size: 14px;
  line-height: 1.55;
  -webkit-font-smoothing: antialiased;
  transition: background .6s ease;
}

/* ---- Console root (var carrier — NO filter here, keeps sticky rail working) */
.console-root {
  max-width: 1320px;
  margin: 0 auto;
  padding: 0 1.5rem 3rem;
}

/* ---- App shell */
.console {
  max-width: 1320px;
  margin: 0 auto;
  padding: 0;
}

/* ---- Title / Brand */
.title {
  font-family: var(--display);
  font-weight: 700;
  letter-spacing: .34em;
  font-size: 13px;
  color: var(--accent);
  text-transform: uppercase;
  margin: 0;
  padding: 1rem 1.5rem;
  opacity: .85;
}

/* ---- Loading placeholder */
.loading-placeholder {
  padding: 1.5rem;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: .72rem;
  letter-spacing: .25em;
  text-transform: uppercase;
}

/* ---- Core rail — pinned heartbeat bar */
.rail {
  position: sticky;
  top: 0;
  z-index: 20;
  background: linear-gradient(180deg, hsl(var(--warm) 28% 7%), hsl(var(--warm) 26% 6% / .92));
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(6px);
  width: 100vw;
  margin-left: calc(50% - 50vw);
}
.rail-in {
  max-width: 1320px;
  margin: 0 auto;
  padding: 12px 1.5rem 10px;
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 18px;
  align-items: end;
}
.core-num {
  font-family: var(--display);
  font-weight: 300;
  font-variant-numeric: tabular-nums;
  font-size: 56px;
  line-height: .82;
  color: var(--text);
  letter-spacing: .01em;
  text-shadow: 0 0 calc(18px * var(--charge)) var(--accent-soft);
}
.core-num b {
  font-weight: 600;
  color: var(--accent);
}
.core-cap {
  font-size: 38px;
  color: var(--muted);
}
.core-meta {
  display: flex;
  gap: 8px;
  align-items: baseline;
  margin-top: 5px;
  font-size: 11px;
  letter-spacing: .22em;
  text-transform: uppercase;
  color: var(--muted);
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 calc(10px * var(--charge)) var(--accent);
  align-self: center;
  flex-shrink: 0;
}
.core-bar {
  height: 4px;
  background: #0a1016;
  border-radius: 2px;
  overflow: hidden;
  grid-column: 1 / -1;
  margin-top: 10px;
}
.core-bar > i {
  display: block;
  height: 100%;
  width: calc(var(--charge) * 100%);
  background: linear-gradient(90deg, var(--accent), color-mix(in oklab, var(--accent), white 35%));
  box-shadow: 0 0 12px var(--accent);
  transition: width .5s ease;
}

/* ---- Status word (in rail, right column) */
.status {
  font-family: var(--display);
  font-weight: 600;
  letter-spacing: .18em;
  text-transform: uppercase;
  font-size: 14px;
  color: var(--accent);
  text-align: right;
  align-self: center;
}
.status-Stable   { color: var(--good); }
.status-Igniting { color: var(--accent); }
.status-Straining { color: var(--accent); }
.status-Failing  { color: var(--fail); }
.status-Critical { color: var(--fail); font-weight: 700; }
.status-Cold     { color: var(--fail); }

/* ---- Resources line (early game) */
.res {
  margin-top: .8rem;
  font-size: .9rem;
  color: #9fb3c2;
}

/* ---- Action buttons */
.act {
  appearance: none;
  cursor: pointer;
  font-family: 'IBM Plex Mono', ui-monospace, Menlo, monospace;
  font-size: 13px;
  color: var(--text);
  background: #0c1822;
  border: 1px solid var(--line);
  border-radius: var(--r);
  padding: .62rem 1rem;
  margin: .4rem .4rem 0 0;
  letter-spacing: .02em;
  transition: background .18s, border-color .18s, color .18s;
}
.act:hover:not(:disabled) {
  background: #11202c;
  border-color: var(--accent);
  color: #eaf3f8;
}
.act:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.act:disabled { opacity: .35; cursor: not-allowed; }
.act.ghost { opacity: .6; }

/* ---- Origin picker — Emberfall instrument plates */
.origins-header {
  font-family: var(--display);
  font-weight: 600;
  font-size: .68rem;
  letter-spacing: .26em;
  text-transform: uppercase;
  color: var(--muted);
  margin: 0 0 1rem;
}
.origins { display: flex; gap: .9rem; flex-wrap: wrap; }
.origin-card {
  flex: 1 1 175px;
  padding: 1rem 1.1rem;
  border: 1px solid var(--line);
  border-radius: var(--r);
  background: linear-gradient(180deg, var(--fill), var(--fill-2));
  box-shadow: 0 0 0 1px #0006 inset, 0 0 calc(26px * var(--charge)) -16px var(--accent);
  cursor: pointer;
  transition: background .18s, border-color .18s, box-shadow .18s;
}
.origin-card:hover {
  background: linear-gradient(180deg, #11202c, #0d1820);
  border-color: var(--accent);
  box-shadow: 0 0 0 1px #0006 inset, 0 0 18px -8px var(--accent);
}
.origin-name {
  font-family: var(--display);
  font-weight: 600;
  color: var(--accent);
  letter-spacing: .16em;
  margin-bottom: .55rem;
  text-transform: uppercase;
  font-size: 15px;
}
.origin-intro { font-size: .82rem; color: #9fb3c2; line-height: 1.45; }

/* ---- Log */
.log {
  margin-top: 1rem;
  max-height: 220px;
  overflow-y: auto;
  font-size: .82rem;
  line-height: 1.5;
}
.log-line { color: var(--muted); padding: .05rem 0; }

/* ---- Dashboard two-column grid */
.dash { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 14px; margin-top: 1.4rem; }
.col { display: flex; flex-direction: column; gap: 14px; }

/* ---- Content + log-rail grid */
.content-grid { display: grid; grid-template-columns: 1fr; gap: 1.4rem; }
.log-rail .log { max-height: none; }
@media (min-width: 1100px) {
  .content-grid { grid-template-columns: minmax(0, 1fr) 340px; align-items: start; }
  .log-rail { position: sticky; top: 84px; max-height: calc(100vh - 100px); overflow-y: auto; }
}

/* ---- Panel */
.panel {
  border: 1px solid var(--line);
  background: linear-gradient(180deg, var(--fill), var(--fill-2));
  border-radius: var(--r);
  padding: .9rem 1rem;
  box-shadow:
    0 0 0 1px #0006 inset,
    0 0 calc(26px * var(--charge)) -16px var(--accent);
}
.panel + .panel { margin-top: 10px; }
.panel-title {
  font-family: var(--display);
  font-weight: 600;
  font-size: .68rem;
  letter-spacing: .26em;
  text-transform: uppercase;
  color: var(--muted);
  margin-bottom: .5rem;
}

/* ---- Bar gauges */
.bar {
  height: 6px;
  background: #0a1016;
  border-radius: 2px;
  overflow: hidden;
  margin: .3rem 0;
}
.bar > span { display: block; height: 100%; }
.bar-gen   > span { background: linear-gradient(90deg, var(--accent), color-mix(in oklab, var(--accent), white 35%)); box-shadow: 0 0 8px var(--accent); }
.bar-draw  > span { background: var(--fail); }
.bar-charge > span { background: linear-gradient(90deg, var(--accent), color-mix(in oklab, var(--accent), white 35%)); box-shadow: 0 0 10px var(--accent); }
.bar-hp    > span { background: var(--good); }
.bar-morale > span { background: #6f9fd6; }
.bar-enemy > span { background: var(--fail); }
.bar-hull  > span { background: #6fb7d6; }
.bar-disp  > span { background: #9b7fd6; }

/* ---- Resource rows */
.res-row {
  display: flex;
  justify-content: space-between;
  font-size: .85rem;
  padding: .2rem 0;
  border-bottom: 1px solid var(--line-soft);
}
.res-row:last-child { border-bottom: 0; }
.res-full     { color: var(--accent); }

/* ---- Module card grid */
.mods { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; margin-top: .6rem; }
.mod { border: 1px solid var(--line); border-radius: var(--r); padding: 10px 12px; background: #0b121a; }
.mod b { font-family: var(--display); font-weight: 500; font-size: 15px; display: block; letter-spacing: .02em; }
.mod .tag { font-size: 10px; letter-spacing: .18em; text-transform: uppercase; color: var(--muted); margin-top: 3px; display: block; }
.mod.dim { opacity: .45; pointer-events: none; }

/* ---- Eyebrow (panel section header) */
.eyebrow { font-family: var(--display); font-weight: 600; letter-spacing: .26em; text-transform: uppercase; font-size: 11px; color: var(--muted); margin: 0 0 8px; }

/* ---- Net-rate value helpers */
.num { font-variant-numeric: tabular-nums; }
.up { color: var(--good); }
.down { color: var(--fail); }
.mut { color: var(--muted); }

/* ---- Muted label */
.muted { color: var(--muted); font-size: .8rem; }

/* ---- Job / Build / Priority rows */
.job-row, .build-row, .prio-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: .85rem;
  padding: .25rem 0;
  gap: .5rem;
  border-bottom: 1px solid var(--line-soft);
}
.job-row:last-child, .build-row:last-child, .prio-row:last-child { border-bottom: 0; }

/* ---- Mini stepper buttons */
.mini {
  appearance: none;
  padding: .15rem .5rem;
  min-width: 1.6rem;
  background: #0c1822;
  color: var(--text);
  border: 1px solid var(--line);
  border-radius: var(--r);
  font-family: inherit;
  cursor: pointer;
  font-size: .82rem;
  transition: background .14s;
}
.mini:hover:not(:disabled) { background: #11202c; border-color: var(--accent); }
.mini:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.mini:disabled { opacity: .3; cursor: not-allowed; }

/* ---- Badges */
.badge { font-size: .7rem; padding: .05rem .4rem; border-radius: var(--r); margin-left: .4rem; }
.badge-power { color: var(--accent); border: 1px solid color-mix(in oklab, var(--accent), var(--line) 60%); }
.badge-input { color: #6f9fd6; border: 1px solid #264a5a; }

/* ---- Crew state labels */
.idle    { color: var(--good); font-size: .8rem; margin-bottom: .4rem; }
.starving { color: var(--fail); font-size: .8rem; margin-bottom: .4rem; }

/* ---- Misc text helpers */
.cost  { color: var(--muted); font-size: .78rem; }
.info  { font-style: italic; min-width: 1.6rem; color: #6fb7d6; }

/* ---- Build description */
.build-desc {
  font-size: .78rem;
  color: #9fb3c2;
  line-height: 1.45;
  border-left: 2px solid var(--line);
  padding: .1rem 0 .4rem .6rem;
  margin: 0 0 .4rem;
}

/* ---- Research */
.research-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: .85rem;
  padding: .25rem 0;
  gap: .5rem;
}
.research-name   { color: var(--text); }
.research-done   { color: var(--good); }
.research-locked { color: var(--muted); }
.research-blurb  { color: #9fb3c2; font-size: .76rem; }
.research-cost   { color: #9fb3c2; font-size: .76rem; }
.research-active { border-left: 2px solid var(--accent); padding-left: .6rem; margin: .3rem 0; }

/* ---- Research bar */
.rbar { height: 6px; background: #0a1016; border-radius: 2px; overflow: hidden; margin: .3rem 0; }
.rbar > span { display: block; height: 100%; background: var(--accent); box-shadow: 0 0 8px var(--accent); }

/* ---- Codex */
.codex-row { font-size: .84rem; padding: .2rem 0; color: #8aa0b0; cursor: pointer; }
.codex-row:hover { color: var(--text); }
.codex-text {
  font-size: .8rem;
  color: #9fb3c2;
  line-height: 1.5;
  border-left: 2px solid var(--line);
  padding: .2rem 0 .4rem .6rem;
  margin: 0 0 .4rem;
}

/* ---- Atmosphere / life-support warning */
.ls-warn {
  margin: .8rem 0;
  padding: .5rem .8rem;
  border: 1px solid color-mix(in oklab, var(--fail), var(--line) 40%);
  border-radius: var(--r);
  color: var(--accent);
  font-size: .82rem;
  letter-spacing: .1em;
  background: color-mix(in oklab, var(--fail), transparent 92%);
}
.ls-warn.crit { color: var(--fail); border-color: color-mix(in oklab, var(--fail), var(--line) 20%); }

/* ---- Navigation */
.nav-activity { font-size: .82rem; color: var(--accent); letter-spacing: .08em; margin: .3rem 0 .5rem; }
.nav-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: .85rem;
  padding: .2rem 0;
  gap: .5rem;
  border-bottom: 1px solid var(--line-soft);
}
.nav-row:last-child { border-bottom: 0; }
.nav-site {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: .8rem;
  color: #9fb3c2;
  padding: .1rem 0 .1rem 1rem;
}
.nav-here  { color: var(--good); }
.nav-wrap  { margin-bottom: 1.2rem; }
.nav-detail { border-top: 1px solid var(--line); padding-top: .4rem; }

/* ---- Away banner */
.away-banner {
  margin: .8rem 0;
  padding: .5rem .8rem;
  border: 1px solid color-mix(in oklab, var(--accent), var(--line) 40%);
  border-radius: var(--r);
  color: var(--accent);
  font-size: .82rem;
  letter-spacing: .12em;
  background: color-mix(in oklab, var(--accent), transparent 94%);
}

/* ---- Life veil — desaturate/dim everything BELOW the rail as charge falls.
   Sibling of .rail (NOT an ancestor), so sticky is unaffected. */
.life-veil { filter: saturate(calc(.35 + var(--charge) * .65)) brightness(calc(.82 + var(--charge) * .18)); transition: filter .5s ease; }

/* ---- Dimmed state (station locked while away) */
.dimmed {
  opacity: .4;
  pointer-events: none;
  filter: grayscale(.4);
  transition: opacity .3s;
}

/* ---- Navmap SVG */
.navmap {
  width: 100%;
  height: 240px;
  display: block;
  margin: .5rem 0 0;
  background: radial-gradient(80% 120% at 30% 0%, #0c1620, #070d13);
  border: 1px solid var(--line);
  border-radius: var(--r);
}
/* live-tinted: stroke/fill use var(--accent) so the ring cools with expansion */
.range-ring { fill: color-mix(in oklab, var(--accent), transparent 96%); stroke: var(--accent); stroke-width: .5; stroke-dasharray: 2 3; opacity: .45; }
.nav-edge { stroke: var(--line); stroke-width: .8; }
.sys { cursor: pointer; stroke: var(--void); stroke-width: .6; }
/* current system: live-tinted accent (not --good green) so it cools with tier */
.sys-current  { fill: var(--accent); }
.sys-reachable { fill: #9fb3c2; }
.sys-far       { fill: #3a4a55; }
.sys-selected  { stroke: var(--text); stroke-width: 1.2; }
.sys-home      { fill: var(--accent); stroke: color-mix(in oklab, var(--accent), white 20%); stroke-width: 1; }
.sys-home-ring { fill: none; stroke: var(--accent); stroke-width: .8; opacity: .4; }
.sys-home-label { fill: var(--muted); font-size: 9px; letter-spacing: .12em; font-family: 'IBM Plex Mono', monospace; }
.star-Neutron  { }
.star-Dead     { }
.navmap circle.sys:hover { stroke: var(--text); stroke-width: 1.2; }

/* ---- Nav detail panel */
.nav-detail { border-top: 1px solid var(--line); margin-top: .6rem; padding-top: .6rem; }
.nav-sys-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: .82rem;
  padding: .3rem 0;
  gap: .5rem;
  border-bottom: 1px solid var(--line-soft);
}
.nav-sys-meta { color: var(--muted); }
.nav-sys-actions { display: flex; gap: .3rem; align-items: center; }

/* ---- Surface map SVG */
.surface { border-color: color-mix(in oklab, var(--accent), var(--line) 65%); }
.surf-here   { font-size: .82rem; color: var(--muted); margin: .3rem 0; }
/* surf-choice: accent-bordered pending panel */
.surf-choice {
  border-left: 2px solid color-mix(in oklab, var(--accent), var(--line) 40%);
  padding: .5rem .5rem .5rem .7rem;
  margin: .6rem 0;
  background: color-mix(in oklab, var(--accent), transparent 96%);
  border-radius: 0 var(--r) var(--r) 0;
}
.surfmap-scroll {
  width: 100%;
  max-height: 300px;
  overflow: auto;
  margin: .5rem 0;
  background: radial-gradient(80% 120% at 30% 0%, #0c1620, #070d13);
  border: 1px solid var(--line);
  border-radius: var(--r);
}
.surfmap       { display: block; }
/* live-tinted: surf-edge uses var(--line) */
.surf-edge     { stroke: var(--line); stroke-width: .8; }
.surf-node     { cursor: pointer; stroke: var(--void); stroke-width: .6; }
/* surf-current: live-tinted accent so it cools with tier */
.surf-current  { fill: var(--accent); }
.surf-reach    { fill: #9fb3c2; }
.surf-done     { fill: #1e2d28; }
.surf-far      { fill: #2a3840; }
.kind-Hazard   { stroke: var(--fail); stroke-width: 1.2; }

/* ---- Suit gauge */
.suit-gauge {
  height: 6px;
  background: #0a1016;
  border-radius: 3px;
  overflow: hidden;
  margin: .35rem 0 .2rem;
}
.suit-gauge > i {
  display: block;
  height: 100%;
  background: var(--good);
  box-shadow: 0 0 8px var(--good);
  transition: width .3s ease;
}

/* ---- Satchel rows */
.satchel-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  font-size: .82rem;
  padding: .22rem 0;
  border-bottom: 1px solid var(--line-soft);
}
.satchel-row:last-child { border-bottom: 0; }

/* ---- Combat */
.combat {
  border: 1px solid color-mix(in oklab, var(--fail), var(--line) 40%);
  border-radius: var(--r);
  padding: .7rem .9rem;
  margin: .6rem 0;
  background: color-mix(in oklab, var(--fail), transparent 95%);
}
.combat-title {
  font-family: var(--display);
  font-weight: 600;
  font-size: .68rem;
  letter-spacing: .26em;
  text-transform: uppercase;
  color: var(--fail);
  margin-bottom: .4rem;
  opacity: .8;
}
.combat-actions { display: flex; flex-wrap: wrap; gap: .4rem; margin-top: .5rem; align-items: center; }

/* ---- Negotiation */
.neg {
  border: 1px solid var(--line);
  border-radius: var(--r);
  padding: .7rem .9rem;
  margin: .6rem 0;
  background: linear-gradient(180deg, var(--fill), var(--fill-2));
}
.neg-title {
  font-family: var(--display);
  font-weight: 600;
  font-size: .68rem;
  letter-spacing: .26em;
  text-transform: uppercase;
  color: var(--muted);
  margin-bottom: .4rem;
}
.neg-band   { font-size: .8rem; letter-spacing: .14em; font-weight: 500; margin-bottom: .25rem; }
.neg-actions { margin-top: .6rem; display: flex; flex-wrap: wrap; gap: .4rem; }
.band-Hostile { color: var(--fail); }
.band-Fearful { color: var(--accent); }
.band-Neutral { color: var(--text); }
.band-Curious { color: var(--good); }

/* ---- Ship contact */
.contact         { border: 1px solid color-mix(in oklab, var(--fail), var(--line) 60%); }
.contact-actions { margin-top: .6rem; display: flex; flex-wrap: wrap; gap: .4rem; }
.contact-stat-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  font-size: .85rem;
  padding: .18rem 0;
}

/* ---- Ship munitions */
.munitions { margin-top: 1rem; }

/* ---- Word-drift inline marker */
.drift {
  color: var(--accent);
  border-bottom: 1px dotted color-mix(in oklab, var(--accent), transparent 55%);
}

/* ---- Endgame (legacy kept for endgame-body / endgame-choices) */
.endgame-body          { white-space: pre-wrap; line-height: 1.6; color: var(--text); margin: .6rem 0 1rem; }
.endgame-complete      { font-family: var(--display); letter-spacing: .25em; margin-bottom: 1rem; color: var(--accent); font-size: .78rem; }
.endgame-choices       { display: flex; flex-direction: column; gap: .5rem; margin-top: .8rem; }

/* ---- Watched takeover (endgame panel) */
.watched {
  border: 1px solid color-mix(in oklab, var(--accent), var(--line) 40%);
  border-radius: var(--r);
  padding: 1.6rem 1.5rem;
  background: radial-gradient(120% 90% at 50% -10%, color-mix(in oklab, var(--accent), transparent 86%), var(--fill-2));
  box-shadow: 0 0 60px -30px var(--accent);
  margin: .6rem 0 1rem;
}
.watched-title {
  font-family: var(--display);
  font-weight: 500;
  font-size: 26px;
  letter-spacing: .01em;
  margin: .15em 0 .35em;
  color: var(--text);
}
.watched-body {
  color: var(--text);
  max-width: 62ch;
  line-height: 1.6;
  margin: 0 0 .5rem;
  font-size: .9rem;
}
/* ---- Choice buttons (endgame endings) */
.choice {
  display: block;
  width: 100%;
  text-align: left;
  margin: 0;
  padding: .7rem 1rem;
}
.c-l {
  font-family: var(--display);
  font-weight: 600;
  font-size: 16px;
  letter-spacing: .04em;
  color: #eaf3f8;
}
.c-d {
  color: var(--muted);
  font-size: 12.5px;
}

/* ---- Navigation map + detail side-by-side */
.nav-split { display: grid; grid-template-columns: 1fr; gap: 1rem; }
@media (min-width: 900px) {
  .nav-split { grid-template-columns: minmax(0, 1.6fr) minmax(260px, 1fr); align-items: start; }
}

/* ---- Surface map width cap (prevent absurd stretch at 1320px) */
.surfmap-scroll { max-width: 600px; }

/* ---- Endgame / Watched containment (narrative focus, not a data spread) */
.watched { max-width: 620px; margin-left: auto; margin-right: auto; }

/* ---- Contact panel containment */
.contact { max-width: 720px; }

/* ---- Responsive */
@media (max-width: 680px) {
  .dash { grid-template-columns: 1fr; }
  .core-num { font-size: 40px; }
  .core-cap { font-size: 28px; }
}

/* ---- Site footer */
.site-footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin: 28px auto 6px;
  padding: 14px 1.5rem 24px;
  border-top: 1px solid var(--line-soft);
  font-size: 12px;
  letter-spacing: .04em;
  color: var(--muted);
}
.foot-link {
  background: none;
  border: none;
  padding: 0;
  font: inherit;
  letter-spacing: inherit;
  color: var(--muted);
  text-decoration: none;
  cursor: pointer;
  transition: color .2s ease;
}
.foot-link:hover { color: var(--accent); }
.foot-link:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
.foot-sep { color: var(--line); }

/* ---- Legal overlay */
.legal-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(4, 8, 11, .72);
  backdrop-filter: blur(2px);
}
.legal-card {
  position: relative;
  max-width: 560px;
  width: 100%;
  max-height: 80vh;
  overflow-y: auto;
  padding: 30px 32px;
  background: linear-gradient(180deg, hsl(var(--warm) 28% 8%), hsl(var(--warm) 26% 6%));
  border: 1px solid var(--line);
  box-shadow: 0 24px 60px rgba(0, 0, 0, .55);
}
.legal-title {
  margin: 0 0 14px;
  font-family: 'Saira Condensed', sans-serif;
  font-weight: 600;
  font-size: 24px;
  letter-spacing: .03em;
  color: var(--accent);
}
.legal-card p {
  margin: 0 0 14px;
  font-size: 14px;
  line-height: 1.65;
  color: var(--text);
}
.legal-card p:last-child { margin-bottom: 0; }
.legal-card .legal-mut { color: var(--muted); font-size: 13px; }
.legal-card a.foot-link { color: var(--accent); text-decoration: underline; }
.legal-card a.foot-link:hover { color: color-mix(in oklab, var(--accent), white 30%); }
.legal-close {
  position: absolute;
  top: 14px;
  right: 14px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: 1px solid var(--line);
  color: var(--muted);
  font-size: 14px;
  cursor: pointer;
  transition: color .2s ease, border-color .2s ease;
}
.legal-close:hover { color: var(--accent); border-color: var(--accent); }
.legal-close:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

/* ---- Accessibility */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    transition: none !important;
    animation: none !important;
  }
}
";
