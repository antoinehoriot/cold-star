use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
use sim::research::{Tech, TechRegistry};
use sim::resources::ResourceKind;
use sim::state::{Action, GameState, World};

#[derive(Clone, PartialEq)]
struct NavRow {
    id: sim::galaxy::SystemId,
    star: String,
    pos: (f64, f64),
    dist: f64,
    is_current: bool,
    reachable: bool,
    affordable: bool,
    scanned: bool,
    cost: f64,
    // (site_index, kind label, depleted, landable)
    sites: Vec<(usize, String, bool, bool)>,
}

#[derive(Clone, PartialEq)]
struct SurfaceNodeVM {
    id: u32,
    depth: u32,
    kind: String,
    resolved: bool,
    is_current: bool,
    edges: Vec<u32>,
}

#[derive(Clone, PartialEq)]
struct CombatVM {
    enemy_hp: f64,
    stance: String,
    shield_ticks: u32,
}

#[derive(Clone, PartialEq)]
struct NegotiationVM {
    kind: String, // "Fauna" | "Peer"
    is_peer: bool,
    band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    disposition: f64,
    // disable predicates — exact duals of the engine's apply() rejections
    can_parley: bool,
    can_trade: bool,
    can_gift: bool,
    can_observe: bool,
    can_harvest: bool,
}

#[derive(Clone, PartialEq)]
struct LandingVM {
    biome: String,
    suit: f64,
    suit_max: f64,
    satchel: Vec<(String, f64)>,
    node: u32,
    depth: u32,
    encounter: String,
    pending_choice: bool,
    traverse: u32,
    neighbors: Vec<(u32, String, bool)>,
    nodes: Vec<SurfaceNodeVM>,
    away_crew: u32,
    squad_hp: f64,
    squad_hp_max: f64,
    morale: f64,
    morale_max: f64,
    combat: Option<CombatVM>,
    negotiation: Option<NegotiationVM>,
    consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
struct LoadoutVM {
    away_crew: u32,
    idle: u32,
    weapon_tier: u8,
    armor_tier: u8,
    // (kind, label, count) for each consumable kind
    consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
struct ContactVM {
    kind: String, // "Peer" | "Remnant"
    is_peer: bool,
    in_combat: bool,
    band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    disposition: f64,
    hull: f64,
    hull_max: f64,
    enemy_hp: f64,
    stance: String,
    repair_drones: u32,
    // disable predicates — exact duals of the engine rejections
    can_parley: bool,
    can_trade: bool,
    can_gift: bool,
    can_avoid: bool,
    can_flee: bool,
}

#[derive(Clone, PartialEq)]
struct ShipMunitionsVM {
    weapon_tier: u8,
    plating_tier: u8,
    hull: f64,
    hull_max: f64,
    repair_drones: u32,
    can_upgrade_weapon: bool,
    can_upgrade_plating: bool,
    can_craft_drone: bool,
}

const CSS: &str = "
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

/* ---- Responsive */
@media (max-width: 680px) {
  .dash { grid-template-columns: 1fr; }
  .core-num { font-size: 40px; }
  .core-cap { font-size: 28px; }
  .content-grid { grid-template-columns: 1fr; }
}

/* ---- Accessibility */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    transition: none !important;
    animation: none !important;
  }
}
";

/// Word-drift slots whose label cools as expansion rises (cold vocab shared across origins).
#[derive(Clone, Copy)]
enum DriftSlot {
    AwayTeam,
    Take,
    Salvage,
    FeedCore,
}

/// The cold-vocabulary table: [T0, T1, T2, T3] per slot.
fn drift_word(slot: DriftSlot, tier: u8) -> &'static str {
    let row: [&str; 4] = match slot {
        DriftSlot::AwayTeam => ["Marines", "the team", "the detail", "the asset"],
        DriftSlot::Take => ["Harvest", "Extract", "Process", "Strip"],
        DriftSlot::Salvage => ["Salvage", "Recover", "Reclaim", "Render"],
        DriftSlot::FeedCore => [
            "Feed the core",
            "Fuel the core",
            "Charge the core",
            "Burn it",
        ],
    };
    row[(tier as usize).min(3)]
}

/// The crew noun: the origin's warm word at T0, then shared cold words.
fn crew_word(tier: u8, origin_noun: &str) -> String {
    match tier {
        0 => origin_noun.to_string(),
        1 => "crew".into(),
        2 => "units".into(),
        _ => "yield".into(),
    }
}

/// Emberfall per-tier palette stops (ember → ice). Returns (accent, text, warm-hue).
fn emberfall_stops(tier: u8) -> (&'static str, &'static str, &'static str) {
    match tier.min(3) {
        0 => ("#e89255", "#d9cdbf", "22"),
        1 => ("#cbb079", "#d2cdc6", "16"),
        2 => ("#9ec3d0", "#c9d1d6", "10"),
        _ => ("#c2e2f2", "#c3d2dd", "6"),
    }
}

#[component]
fn PowerMeter(
    status: String,
    charge: f64,
    capacity: f64,
    fuel: f64,
    generation: f64,
    active_draw: f64,
) -> Element {
    let cap = if capacity > 0.0 { capacity } else { 1.0 };
    let charge_pct = (charge / cap * 100.0).clamp(0.0, 100.0);
    let scale = generation.max(active_draw).max(1.0);
    let gen_pct = (generation / scale * 100.0).clamp(0.0, 100.0);
    let draw_pct = (active_draw / scale * 100.0).clamp(0.0, 100.0);
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Reactor" }
            div { class: "status status-{status}", "{status}" }
            div { class: "res-row",
                span { "Output" }
                span { class: "num", "{generation:.1}" }
            }
            div { class: "bar bar-gen", span { style: "width: {gen_pct}%" } }
            div { class: "res-row",
                span { "Draw" }
                span { class: "num", "{active_draw:.1}" }
            }
            div { class: "bar bar-draw", span { style: "width: {draw_pct}%" } }
            div { class: "res-row",
                span { "Charge" }
                span { class: "num", "{charge:.0} / {capacity:.0}" }
            }
            div { class: "bar bar-charge", span { style: "width: {charge_pct}%" } }
            div { class: "res-row",
                span { "Fuel" }
                span { class: "num", "{fuel:.0}" }
            }
        }
    }
}

#[component]
fn ResourcesPanel(rows: Vec<(String, f64, f64, f64, bool)>) -> Element {
    // rows: (name, amount, cap, net, at_cap)
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Resources \u{00b7} net / tick" }
            for (name, amount, cap, net, at_cap) in rows {
                div { class: "res-row",
                    span {
                        class: if at_cap { "res-full" } else { "" },
                        "{name}"
                        if at_cap { span { class: "res-full", " (full)" } }
                    }
                    span { class: "num",
                        "{amount:.0}"
                        if cap.is_finite() && cap < 1e9 { span { class: "mut", " / {cap:.0}" } }
                        " \u{00a0}"
                        if net > 0.0 {
                            span { class: "mut up", "+{net:.1}" }
                        } else if net < 0.0 {
                            span { class: "mut down", "\u{2212}{(-net):.1}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn JobPanel(
    idle: u32,
    starving: bool,
    starving_label: String,
    rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)>,
    on_assign: EventHandler<(sim::job::JobId, i32)>,
) -> Element {
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Crew" }
            div { class: "res-row",
                span { "Idle" }
                span { class: "num", "{idle}" }
            }
            if starving { div { class: "starving", "{starving_label}" } }
            for (id, name, count, has_crew, dark) in rows {
                div { class: "job-row",
                    span {
                        "{name}"
                        span { class: "mut", " \u{00d7}{count}" }
                        if has_crew && dark == "power" { span { class: "badge badge-power", "\u{23f8} no power" } }
                        if has_crew && dark == "input" { span { class: "badge badge-input", "\u{25bd} no input" } }
                    }
                    span {
                        button {
                            class: "mini",
                            disabled: count == 0,
                            onclick: move |_| on_assign.call((id, -1)),
                            "\u{2212}"
                        }
                        button {
                            class: "mini",
                            disabled: idle == 0,
                            onclick: move |_| on_assign.call((id, 1)),
                            "+"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BuildMenu(
    rows: Vec<(sim::module::ModuleId, String, String, String, u32, bool)>,
    on_build: EventHandler<sim::module::ModuleId>,
) -> Element {
    // Which module's narrative info line is currently expanded (toggled by its
    // (i) button). None = all collapsed.
    let mut info_open = use_signal(|| None::<sim::module::ModuleId>);
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Modules" }
            div { class: "mods",
                for (id, name, description, cost, n, affordable) in rows {
                    div { class: if affordable { "mod" } else { "mod dim" },
                        b { "{name}" if n > 0 { span { class: "mut", " ({n})" } } }
                        span { class: "tag", "{cost}" }
                        if info_open() == Some(id) {
                            div { class: "build-desc", "{description}" }
                        }
                        div { style: "margin-top: 8px; display: flex; gap: 4px; align-items: center;",
                            button {
                                class: "mini info",
                                title: "What is this?",
                                onclick: move |_| {
                                    let next = if info_open() == Some(id) { None } else { Some(id) };
                                    info_open.set(next);
                                },
                                "i"
                            }
                            button {
                                class: "mini",
                                disabled: !affordable,
                                onclick: move |_| on_build.call(id),
                                "build"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PriorityPanel(
    rows: Vec<(sim::priority::Consumer, String, bool, bool)>,
    on_move: EventHandler<(sim::priority::Consumer, bool)>,
) -> Element {
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Power Priority" }
            p { class: "muted", "higher = browns out last" }
            for (c, name, is_first, is_last) in rows {
                div { class: "prio-row",
                    span { "{name}" }
                    span {
                        button { class: "mini", disabled: is_first, onclick: move |_| on_move.call((c, true)), "\u{25b2}" }
                        button { class: "mini", disabled: is_last, onclick: move |_| on_move.call((c, false)), "\u{25bc}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ResearchPanel(
    // active: (name, ticks_remaining, ticks_total) when something is researching
    active: Option<(String, u32, u32)>,
    // rows: (id, name, blurb, data_cost, research_ticks, state, affordable) where state is
    // "done" | "available" | "locked"; available rows are startable iff affordable
    rows: Vec<(
        sim::research::TechId,
        String,
        String,
        f64,
        u32,
        &'static str,
        bool,
    )>,
    on_start: EventHandler<sim::research::TechId>,
) -> Element {
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Research" }
            if let Some((name, rem, total)) = active {
                {
                    let done = total.saturating_sub(rem);
                    let pct = if total > 0 { (done as f64 / total as f64 * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                    rsx! {
                        div { class: "research-active",
                            div { class: "research-name", "Researching: {name}  ({rem}s left)" }
                            div { class: "rbar", span { style: "width: {pct}%" } }
                        }
                    }
                }
            }
            for (id, name, blurb, cost, ticks, st, affordable) in rows {
                div { class: "research-row",
                    span {
                        span {
                            class: match st { "done" => "research-done", "locked" => "research-locked", _ => "research-name" },
                            "{name}"
                        }
                        div { class: "research-blurb", "{blurb}" }
                        if st == "available" {
                            div { class: "research-cost", "{cost:.0} Data  \u{00b7}  {ticks}s" }
                        }
                    }
                    span {
                        if st == "done" {
                            span { class: "research-done", "\u{2713}" }
                        } else if st == "available" {
                            button {
                                class: "mini",
                                disabled: !affordable,
                                onclick: move |_| on_start.call(id),
                                "research"
                            }
                        } else {
                            span { class: "research-locked", "locked" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CodexPanel(entries: Vec<(String, String)>) -> Element {
    // entries: (title, text). Click a title to expand its text inline.
    let mut open = use_signal(|| None::<usize>);
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Codex" }
            for (i, (title, text)) in entries.into_iter().enumerate() {
                div {
                    class: "codex-row",
                    onclick: move |_| {
                        let next = if open() == Some(i) { None } else { Some(i) };
                        open.set(next);
                    },
                    "{title}"
                }
                if open() == Some(i) {
                    div { class: "codex-text", "{text}" }
                }
            }
        }
    }
}

#[component]
fn NavigationPanel(
    drive: f64,
    cargo_used: f64,
    cargo_max: f64,
    fuel: f64,
    isotopes: f64,
    activity_label: String,
    busy: bool,
    away: bool,
    has_sensor: bool,
    stranded: bool,
    can_upgrade_drive: bool,
    current: sim::galaxy::SystemId,
    rows: Vec<NavRow>,
    on_scan: EventHandler<sim::galaxy::SystemId>,
    on_jump: EventHandler<sim::galaxy::SystemId>,
    on_resolve: EventHandler<(sim::galaxy::SystemId, usize)>,
    on_scavenge: EventHandler<()>,
    on_upgrade: EventHandler<()>,
    on_land: EventHandler<usize>,
) -> Element {
    let mut selected = use_signal(|| current);
    // Center on the current system's position.
    let center = rows
        .iter()
        .find(|r| r.is_current)
        .map(|r| r.pos)
        .unwrap_or((0.0, 0.0));
    // viewBox half-extent: fit the drive ring + a margin (clamped so a huge drive still renders).
    let half = (drive * 1.25).clamp(30.0, 140.0);
    let view = format!("{} {} {} {}", -half, -half, 2.0 * half, 2.0 * half);
    let sel = selected();
    let sel_row = rows.iter().find(|r| r.id == sel).cloned();
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Navigation" }
            div { class: "muted",
                "Drive {drive:.0} \u{00b7} Cargo {cargo_used:.0}/{cargo_max:.0} \u{00b7} Fuel {fuel:.0} \u{00b7} Isotopes {isotopes:.0}"
            }
            if !activity_label.is_empty() {
                div { class: "nav-activity", "{activity_label}" }
            }
            if stranded {
                button { class: "act", onclick: move |_| on_scavenge.call(()), "Scavenge fuel" }
            }
            if can_upgrade_drive {
                button { class: "act", onclick: move |_| on_upgrade.call(()), "Recalibrate drive (+range)" }
            }
            // SVG star map — live-tinted: range ring + current dot use var(--accent) via CSS.
            svg {
                class: "navmap",
                view_box: "{view}",
                preserve_aspect_ratio: "xMidYMid meet",
                // range ring: styled via .range-ring (stroke: var(--accent))
                circle { cx: 0, cy: 0, r: "{drive}", class: "range-ring" }
                // edges from current to reachable systems
                for row in rows.iter().filter(|r| r.reachable) {
                    line {
                        x1: 0, y1: 0,
                        x2: "{row.pos.0 - center.0}", y2: "{row.pos.1 - center.1}",
                        class: "nav-edge",
                    }
                }
                // system dots
                for row in rows.clone() {
                    {
                        let id = row.id;
                        let is_home = row.id.0 == 0;
                        let dot_class = {
                            let mut c = format!("sys star-{}", row.star);
                            if row.is_current { c.push_str(" sys-current"); }
                            else if row.reachable { c.push_str(" sys-reachable"); }
                            else { c.push_str(" sys-far"); }
                            if is_home { c.push_str(" sys-home"); }
                            if sel == row.id { c.push_str(" sys-selected"); }
                            c
                        };
                        let r_val = if is_home { "5.5" } else if row.is_current { "4.5" } else { "3.2" };
                        let cx_val = format!("{}", row.pos.0 - center.0);
                        let cy_val = format!("{}", row.pos.1 - center.1);
                        let label_y = format!("{}", row.pos.1 - center.1 + 14.0);
                        rsx! {
                            // The home station gets a halo + label so it always reads as "home".
                            if is_home {
                                circle { cx: "{cx_val}", cy: "{cy_val}", r: "10", class: "sys-home-ring" }
                            }
                            circle {
                                cx: "{cx_val}",
                                cy: "{cy_val}",
                                r: "{r_val}",
                                class: "{dot_class}",
                                onclick: move |_| selected.set(id),
                            }
                            if is_home {
                                text {
                                    x: "{cx_val}",
                                    y: "{label_y}",
                                    text_anchor: "middle",
                                    class: "sys-home-label",
                                    "HOME"
                                }
                            }
                        }
                    }
                }
            }
            // Detail + actions for the selected system.
            if let Some(row) = sel_row {
                div { class: "nav-detail",
                    div { class: "nav-row",
                        span {
                            if row.is_current { span { class: "nav-here", "\u{25c9} " } }
                            span { class: "nav-sys-meta",
                                "Sys {row.id.0} \u{00b7} {row.star} \u{00b7} {row.dist:.0} ly"
                            }
                            if row.scanned { span { class: "muted", " \u{00b7} scanned" } } else { span { class: "muted", " \u{00b7} unscanned" } }
                        }
                        span { class: "nav-sys-actions",
                            if !row.is_current && has_sensor && !row.scanned {
                                button { class: "mini", disabled: busy, onclick: move |_| on_scan.call(row.id), "Scan" }
                            }
                            if row.reachable {
                                button {
                                    class: "act",
                                    disabled: busy || !row.affordable,
                                    onclick: move |_| on_jump.call(row.id),
                                    if row.affordable { "Jump \u{00b7} {row.cost:.0} fuel" } else { "Jump (low fuel)" }
                                }
                            }
                        }
                    }
                    for (idx, kind, depleted, landable) in row.sites.clone() {
                        div { class: "nav-site",
                            span { "{kind}" if depleted { span { class: "muted", " \u{00b7} stripped" } } }
                            if row.is_current && !depleted {
                                if landable {
                                    button { class: "act", disabled: busy, onclick: move |_| on_land.call(idx), "Land" }
                                } else {
                                    button { class: "mini", disabled: busy, onclick: move |_| on_resolve.call((row.id, idx)), "work" }
                                }
                            }
                        }
                    }
                    if !row.scanned && row.sites.is_empty() {
                        div { class: "muted", "Scan to reveal what\u{2019}s here." }
                    }
                }
            }
            if away {
                div { class: "muted", "Return to the station to deposit cargo and resume control." }
            }
        }
    }
}

#[component]
fn ContactPanel(
    vm: ContactVM,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_attack: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_stance: EventHandler<()>,
    on_repair: EventHandler<()>,
) -> Element {
    let disp_pct = ((vm.disposition + 100.0) / 2.0).clamp(0.0, 100.0);
    let hull_pct = if vm.hull_max > 0.0 {
        (vm.hull / vm.hull_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rsx! {
        div { class: "panel contact",
            p { class: "eyebrow", "Contact \u{00b7} {vm.kind}" }
            // Hull readout + bar
            div { class: "contact-stat-row",
                span { class: "mut", "Hull" }
                span { class: "num", "{vm.hull:.0} / {vm.hull_max:.0}" }
            }
            div { class: "bar bar-hull", span { style: "width: {hull_pct}%" } }
            if vm.in_combat {
                // Combat mode: enemy hull + actions
                div { class: "contact-stat-row",
                    span { class: "mut", "Enemy hull" }
                    span { class: "num down", "{vm.enemy_hp:.0}" }
                }
                div { class: "bar bar-enemy", span { style: "width: {(vm.enemy_hp).clamp(0.0,100.0)}%" } }
                div { class: "contact-actions",
                    button { class: "mini", onclick: move |_| on_stance.call(()), "Stance: {vm.stance}" }
                    button { class: "mini", disabled: vm.repair_drones == 0, onclick: move |_| on_repair.call(()), "Repair drone ({vm.repair_drones})" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            } else {
                // Hail mode: disposition band + verbs (peer) or husk note
                if vm.is_peer {
                    div { class: "neg-band band-{vm.band}", "Disposition: {vm.band} ({vm.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted", style: "margin: .35rem 0 .1rem;",
                        "It moves the way you move. It is deciding about you, too."
                    }
                } else {
                    div { class: "muted", "A husk-ship. It does not answer hails." }
                }
                div { class: "contact-actions",
                    if vm.is_peer {
                        button { class: "mini", disabled: !vm.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        button { class: "mini", disabled: !vm.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        button { class: "mini", disabled: !vm.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        button { class: "mini", disabled: !vm.can_avoid, onclick: move |_| on_avoid.call(()), "Avoid" }
                    }
                    button { class: "act", onclick: move |_| on_attack.call(()), "Attack" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            }
        }
    }
}

#[component]
fn EndgamePanel(
    vm: EndgameVM,
    on_choose: EventHandler<sim::Ending>,
    on_restart: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "watched",
            if vm.ended {
                // Ended: per-origin closing note + restart
                p { class: "eyebrow", "The record is closed" }
                h2 { class: "watched-title", "{vm.ending_title}" }
                div { class: "endgame-body", "{vm.ending_body}" }
                div { class: "endgame-complete", "The run is complete." }
                button { class: "act", onclick: move |_| on_restart.call(()), "Begin again" }
            } else {
                // WatchedArrived: takeover panel — cold terminal climax
                p { class: "eyebrow", "Contact \u{00b7} resolved out of the dark" }
                h2 { class: "watched-title", "The Watched" }
                p { class: "watched-body",
                    "It does not attack. It holds position and broadcasts on a frequency your station once used too. It moves the way you move. It has watched you become this."
                    span { class: "mut", " The ruins you stripped were stations like yours. The peers were you, at a different hour." }
                }
                p { class: "eyebrow", style: "margin-top: 1.1rem;", "The record is open. Choose how this ends." }
                div { class: "endgame-choices",
                    for (e, label, desc) in vm.choices.clone() {
                        button {
                            class: "act choice",
                            onclick: move |_| on_choose.call(e),
                            span { class: "c-l", "{label}" }
                            br {}
                            span { class: "c-d", "{desc}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SurfaceView(
    vm: LandingVM,
    tier: u8,
    on_move: EventHandler<u32>,
    on_harvest: EventHandler<()>,
    on_leave: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_use_consumable: EventHandler<sim::gear::ConsumableKind>,
    on_stance: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_observe: EventHandler<()>,
    on_harvest_fauna: EventHandler<()>,
    on_attack_alien: EventHandler<()>,
) -> Element {
    let suit_pct = (vm.suit / vm.suit_max * 100.0).clamp(0.0, 100.0);
    let busy = vm.traverse > 0;

    // Layout nodes by depth (x) and band position (y).
    // Pixel-space layout: 70px per depth step (x), 46px per slot (y), 30px margin.
    const PX_X: f64 = 70.0;
    const PX_Y: f64 = 46.0;
    const MARGIN: f64 = 30.0;
    let max_depth = vm.nodes.iter().map(|n| n.depth).max().unwrap_or(0).max(1);
    let mut by_depth: std::collections::BTreeMap<u32, Vec<&SurfaceNodeVM>> = Default::default();
    for n in &vm.nodes {
        by_depth.entry(n.depth).or_default().push(n);
    }
    let max_band = by_depth.values().map(|v| v.len()).max().unwrap_or(1).max(1);
    let placed: Vec<(SurfaceNodeVM, f64, f64)> = by_depth
        .iter()
        .flat_map(|(d, band)| {
            let n = band.len();
            band.iter()
                .enumerate()
                .map(move |(i, node)| {
                    let x = MARGIN + (*d as f64) * PX_X;
                    let y = MARGIN
                        + (i as f64 + 0.5 - n as f64 / 2.0) * PX_Y
                        + (max_band as f64 / 2.0) * PX_Y;
                    ((*node).clone(), x, y)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // SVG content size derived from the actual node extent.
    let svg_w = (MARGIN + max_depth as f64 * PX_X + MARGIN).max(120.0);
    let svg_h = (MARGIN + max_band as f64 * PX_Y + MARGIN).max(80.0);
    let view = format!("0 0 {svg_w} {svg_h}");

    // Current node's reachable set — bidirectional: cur→n (forward) OR n→cur (backward).
    let cur = vm.nodes.iter().find(|n| n.is_current);
    let cur_id = cur.map(|n| n.id).unwrap_or(u32::MAX);
    let cur_edges: Vec<u32> = vm
        .nodes
        .iter()
        .filter(|n| !n.is_current)
        .filter(|n| {
            // current→n (forward) OR n→current (backward)
            cur.map_or(false, |c| c.edges.contains(&n.id)) || n.edges.contains(&cur_id)
        })
        .map(|n| n.id)
        .collect();

    // Build a lookup from node id to (x, y) for drawing edges.
    let pos_map: std::collections::HashMap<u32, (f64, f64)> =
        placed.iter().map(|(n, x, y)| (n.id, (*x, *y))).collect();

    let squad_hp_pct = if vm.squad_hp_max > 0.0 {
        (vm.squad_hp / vm.squad_hp_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let morale_pct = if vm.morale_max > 0.0 {
        (vm.morale / vm.morale_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let disp_pct = vm
        .negotiation
        .as_ref()
        .map(|neg| ((neg.disposition + 100.0) / 2.0).clamp(0.0, 100.0))
        .unwrap_or(0.0);

    rsx! {
        div { class: "panel surface",
            p { class: "eyebrow", "Surface \u{00b7} {vm.biome}" }
            // Suit integrity gauge
            p { class: "eyebrow", style: "margin-top: .6rem;", "Suit integrity" }
            div { class: "suit-gauge", i { style: "width: {suit_pct}%" } }
            div { class: "muted", style: "font-size: .75rem; margin: .15rem 0 .4rem;",
                "{vm.suit:.0} / {vm.suit_max:.0} \u{00b7} {vm.encounter} (depth {vm.depth})"
            }
            if busy { div { class: "muted", "Crossing the ground\u{2026} {vm.traverse}s" } }
            // Satchel
            p { class: "eyebrow", style: "margin-top: .7rem;", "Satchel" }
            if vm.satchel.is_empty() { div { class: "muted", "empty" } }
            for (name, amt) in vm.satchel.clone() {
                div { class: "satchel-row", span { "{name}" } span { class: "num", "{amt:.0}" } }
            }
            // Sub-states: combat → negotiation → pending-choice → node-graph map (order unchanged)
            if let Some(c) = vm.combat.clone() {
                div { class: "combat",
                    div { class: "combat-title", "Contact \u{2014} Remnant" }
                    div { class: "muted", "Enemy hull {c.enemy_hp:.0}" }
                    div { class: "bar bar-enemy", span { style: "width: {(c.enemy_hp).clamp(0.0,100.0)}%" } }
                    div { class: "muted",
                        "{drift_word(DriftSlot::AwayTeam, tier)} \u{00d7}{vm.away_crew} \u{00b7} HP {vm.squad_hp:.0}/{vm.squad_hp_max:.0} \u{00b7} morale {vm.morale:.0}"
                    }
                    div { class: "bar bar-hp", span { style: "width: {squad_hp_pct}%" } }
                    div { class: "bar bar-morale", span { style: "width: {morale_pct}%" } }
                    if c.shield_ticks > 0 { div { class: "muted", "Shield {c.shield_ticks}s" } }
                    div { class: "combat-actions",
                        button { class: "mini", onclick: move |_| on_stance.call(()), "Stance: {c.stance}" }
                        for (kind, label, count) in vm.consumables.clone() {
                            {
                                let kind = kind;
                                rsx! {
                                    button { class: "mini", disabled: count == 0, onclick: move |_| on_use_consumable.call(kind), "{label} ({count})" }
                                }
                            }
                        }
                        button { class: "act", onclick: move |_| on_flee.call(()), "Flee" }
                    }
                }
            } else if let Some(neg) = vm.negotiation.clone() {
                div { class: "neg",
                    div { class: "neg-title", "Contact \u{2014} {neg.kind}" }
                    div { class: "neg-band band-{neg.band}", "Disposition: {neg.band} ({neg.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted",
                        if neg.is_peer { "It moves the way you move. It is deciding about you, too." }
                        else { "Native life. Wary, watching." }
                    }
                    div { class: "neg-actions",
                        button { class: "mini", disabled: !neg.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        if neg.is_peer {
                            button { class: "mini", disabled: !neg.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        }
                        button { class: "mini", disabled: !neg.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        if !neg.is_peer {
                            button { class: "mini", disabled: !neg.can_observe, onclick: move |_| on_observe.call(()), "Observe" }
                            button { class: "mini", disabled: !neg.can_harvest, onclick: move |_| on_harvest_fauna.call(()), "{drift_word(DriftSlot::Take, tier)}" }
                        }
                        button { class: "act", onclick: move |_| on_avoid.call(()), "Avoid" }
                        button { class: "act", onclick: move |_| on_attack_alien.call(()), "Attack" }
                    }
                }
            } else if vm.pending_choice {
                div { class: "surf-choice",
                    div { class: "muted", "Something living. It has not noticed you yet." }
                    button { class: "act", onclick: move |_| on_harvest.call(()), "{drift_word(DriftSlot::Take, tier)} it" }
                    button { class: "act ghost", onclick: move |_| on_leave.call(()), "Leave it" }
                }
            } else {
                // SVG node-graph map — scrollable, pixel-sized to content.
                // live-tinted: surf-current uses var(--accent) via CSS
                div { class: "surfmap-scroll",
                svg {
                    class: "surfmap",
                    view_box: "{view}",
                    width: "{svg_w}",
                    height: "{svg_h}",
                    // Edges — draw between placed nodes that share an edge.
                    for (node, x1v, y1v) in placed.clone() {
                        for eid in node.edges.clone() {
                            if let Some((x2v, y2v)) = pos_map.get(&eid) {
                                line {
                                    x1: "{x1v}",
                                    y1: "{y1v}",
                                    x2: "{x2v}",
                                    y2: "{y2v}",
                                    class: "surf-edge",
                                }
                            }
                        }
                    }
                    // Node circles.
                    for (node, xv, yv) in placed.clone() {
                        {
                            let id = node.id;
                            let reachable = cur_edges.contains(&id);
                            let node_class = {
                                let mut c = "surf-node".to_string();
                                if node.is_current {
                                    c.push_str(" surf-current");
                                } else if reachable {
                                    c.push_str(" surf-reach");
                                } else if node.resolved {
                                    c.push_str(" surf-done");
                                } else {
                                    c.push_str(" surf-far");
                                }
                                if node.kind == "Hazard" {
                                    c.push_str(" kind-Hazard");
                                }
                                c
                            };
                            let r_val = if node.is_current { "4" } else { "3" };
                            rsx! {
                                circle {
                                    cx: "{xv}",
                                    cy: "{yv}",
                                    r: "{r_val}",
                                    class: "{node_class}",
                                    onclick: move |_| {
                                        if reachable && !busy {
                                            on_move.call(id);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                } // end surfmap-scroll div
                if cur_edges.is_empty() {
                    div { class: "muted", "No reachable nodes. Extract." }
                }
            }
            button { class: "act ghost", onclick: move |_| on_extract.call(()), "Extract to orbit" }
        }
    }
}

#[component]
fn LoadoutPanel(
    vm: LoadoutVM,
    tier: u8,
    can_set_crew: bool,
    on_set_crew: EventHandler<i32>,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_armor: EventHandler<()>,
    on_craft: EventHandler<sim::gear::ConsumableKind>,
) -> Element {
    rsx! {
        div { class: "panel",
            // Away-team section
            p { class: "eyebrow", "Away-team" }
            div { class: "job-row",
                span {
                    span { class: "drift", "{drift_word(DriftSlot::AwayTeam, tier)}" }
                    " \u{00d7}{vm.away_crew}"
                    span { class: "mut", "  ({vm.idle} idle)" }
                }
                span {
                    button { class: "mini", disabled: !can_set_crew || vm.away_crew == 0, onclick: move |_| on_set_crew.call(-1), "\u{2212}" }
                    button { class: "mini", disabled: !can_set_crew || vm.idle == 0, onclick: move |_| on_set_crew.call(1), "+" }
                }
            }
            if !can_set_crew { div { class: "muted", "Dock to change the away-team." } }
            // Armory section
            p { class: "eyebrow", style: "margin-top: .7rem;", "Armory" }
            div { class: "job-row",
                span { "Weapon T{vm.weapon_tier}" }
                button { class: "mini", disabled: vm.weapon_tier >= 2, onclick: move |_| on_upgrade_weapon.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Armor T{vm.armor_tier}" }
                button { class: "mini", disabled: vm.armor_tier >= 2, onclick: move |_| on_upgrade_armor.call(()), "upgrade" }
            }
            for (kind, label, count) in vm.consumables.clone() {
                div { class: "job-row",
                    span { "{label} \u{00d7}{count}" }
                    button { class: "mini", onclick: move |_| on_craft.call(kind), "craft" }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct EndgameVM {
    ended: bool,
    // WatchedArrived: the earned choices — (ending, button label, one-line meaning)
    choices: Vec<(sim::Ending, String, String)>,
    // Ended: the chosen ending's per-origin closing note (read from the codex)
    ending_title: String,
    ending_body: String,
}

#[component]
fn ShipMunitionsPanel(
    vm: ShipMunitionsVM,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_plating: EventHandler<()>,
    on_craft_drone: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "panel munitions",
            p { class: "eyebrow", "Ship munitions" }
            div { class: "job-row",
                span { "Hull" }
                span { class: "num", "{vm.hull:.0} / {vm.hull_max:.0}" }
            }
            div { class: "job-row",
                span { "Guns T{vm.weapon_tier}" }
                button { class: "mini", disabled: !vm.can_upgrade_weapon, onclick: move |_| on_upgrade_weapon.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Plating T{vm.plating_tier}" }
                button { class: "mini", disabled: !vm.can_upgrade_plating, onclick: move |_| on_upgrade_plating.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Repair drones \u{00d7}{vm.repair_drones}" }
                button { class: "mini", disabled: !vm.can_craft_drone, onclick: move |_| on_craft_drone.call(()), "craft" }
            }
        }
    }
}

#[component]
pub fn App() -> Element {
    let tuning = use_signal(|| content::load().unwrap_or_else(|_| Tuning::default()));
    let jobs = use_signal(|| content::load_jobs().map(|r| r.0).unwrap_or_default());
    let modules = use_signal(|| content::load_modules().map(|r| r.0).unwrap_or_default());
    let techs = use_signal(|| content::load_techs().map(|r| r.0).unwrap_or_default());
    let origins = use_signal(|| content::load_origins().unwrap_or_default());
    let state = use_signal(|| {
        crate::persist::load_initial(&tuning.read(), &jobs.read(), &modules.read(), &techs.read())
    });

    crate::tick::use_tick_driver(state, tuning, jobs, modules, techs, 1000.0, 8 * 60 * 60);

    let has_game = state.read().is_some();

    rsx! {
        style { dangerous_inner_html: CSS }
        main { class: "console",
            h1 { class: "title", "COLD STAR" }
            if has_game {
                Console { state, tuning, jobs, modules, techs }
            } else {
                OriginSelect { state, tuning, jobs, modules, techs, origins }
            }
        }
    }
}

#[component]
fn OriginSelect(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<sim::job::Job>>,
    modules: Signal<Vec<sim::module::Module>>,
    techs: Signal<Vec<Tech>>,
    origins: Signal<Vec<OriginDef>>,
) -> Element {
    rsx! {
        div { class: "console-root",
            p { class: "origins-header", "Choose an origin" }
            div { class: "origins",
                for o in origins.read().iter().cloned() {
                    div {
                        class: "origin-card",
                        onclick: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            let seed = crate::persist::seed_from_clock();
                            let gs = GameState::new_with_origin(&o, &world, seed);
                            state.set(Some(gs));
                        },
                        div { class: "origin-name", "{o.name}" }
                        div { class: "origin-intro", "{o.intro}" }
                    }
                }
            }
        }
    }
}

#[component]
fn Console(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<sim::job::Job>>,
    modules: Signal<Vec<sim::module::Module>>,
    techs: Signal<Vec<Tech>>,
) -> Element {
    let snapshot = state.read();
    let Some(gs) = snapshot.as_ref() else {
        return rsx! {
            div { class: "loading-placeholder",
                "COLD STAR"
                span { style: "display:block;margin-top:.5rem;width:40px;height:1px;background:var(--line);" }
            }
        };
    };
    let charge = gs.reactor.charge;
    let capacity = gs.reactor.capacity;
    let status = format!("{:?}", gs.reactor.status);
    let is_cold = gs.reactor.status == sim::reactor::ReactorStatus::Cold;
    let salvage_unlocked = gs.flags.contains(&Flag::SalvageUnlocked);
    let source_available = gs.flags.contains(&Flag::CrewSourceAvailable);
    let source_active = gs.flags.contains(&Flag::CrewSourceActive);
    let crew_arrived = gs.flags.contains(&Flag::CrewArrived);
    let scrap = gs.resources.scrap;
    let crew = gs.crew.total;
    let salvage_cooldown = gs.salvage_cooldown;
    let source_name = gs.origin.source_name.clone();
    let crew_noun = gs.origin.crew_noun.clone();
    let log_lines: Vec<(u64, String)> = gs
        .log
        .entries()
        .iter()
        .rev()
        .map(|e| (e.tick, e.text.clone()))
        .collect();
    let economy_unlocked = gs.flags.contains(&Flag::EconomyUnlocked);
    let power_visible = gs.flags.contains(&Flag::PowerBudgetVisible);
    let build_menu_unlocked = gs.flags.contains(&Flag::BuildMenuUnlocked);
    let fuel = gs.resources.fuel;
    let generation = gs.telemetry.generation;
    let active_draw = gs.telemetry.active_draw;
    let res_rows: Vec<(String, f64, f64, f64, bool)> = ResourceKind::ALL
        .iter()
        .filter(|k| gs.resources.amount(**k) > 0.0 || gs.telemetry.net.contains_key(*k))
        .map(|k| {
            let net = gs.telemetry.net.get(k).copied().unwrap_or(0.0);
            let amount = gs.resources.amount(*k);
            let cap = gs.resources.cap;
            let at_cap = cap.is_finite() && amount >= cap;
            (format!("{k:?}"), amount, cap, net, at_cap)
        })
        .collect();
    let idle = gs.crew.idle();
    let starving = gs.telemetry.starving;
    let job_rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)> = jobs
        .read()
        .iter()
        .filter(|j| j.gated_by.map(|g| gs.built_count(g) > 0).unwrap_or(true))
        .map(|j| {
            let count = gs.crew.assigned(j.id);
            let dark = if count > 0
                && gs
                    .telemetry
                    .paused
                    .contains(&sim::priority::Consumer::Job(j.id))
            {
                "power"
            } else if gs.telemetry.starved_input.contains(&j.id) {
                "input"
            } else {
                ""
            };
            (j.id, j.name.clone(), count, count > 0, dark)
        })
        .collect();
    let build_rows: Vec<(sim::module::ModuleId, String, String, String, u32, bool)> = modules
        .read()
        .iter()
        .filter(|m| {
            let prereqs_ok = m.requires_modules.iter().all(|r| gs.built_count(*r) > 0);
            let buildable_again = m.repeatable || gs.built_count(m.id) == 0;
            let flag_ok = m
                .requires_flag
                .map(|f| gs.flags.contains(&f))
                .unwrap_or(true);
            prereqs_ok && buildable_again && flag_ok
        })
        .map(|m| {
            let n = gs.built_count(m.id);
            let scaled_cost: Vec<(ResourceKind, f64)> = m
                .cost
                .iter()
                .map(|(k, a)| (*k, a * m.cost_scaling.powi(n as i32)))
                .collect();
            let cost = scaled_cost
                .iter()
                .map(|(k, a)| format!("{:.0} {k:?}", a))
                .collect::<Vec<_>>()
                .join(", ");
            let affordable = scaled_cost
                .iter()
                .all(|(k, a)| gs.resources.amount(*k) >= *a);
            (
                m.id,
                m.name.clone(),
                m.description.clone(),
                cost,
                n,
                affordable,
            )
        })
        .collect();
    let prio_rows: Vec<(sim::priority::Consumer, String, bool, bool)> = {
        let list = &gs.priority.0;
        let last = list.len().saturating_sub(1);
        list.iter()
            .enumerate()
            .map(|(i, c)| {
                let name = match c {
                    sim::priority::Consumer::Atmosphere => "Atmosphere".to_string(),
                    sim::priority::Consumer::Thermal => "Thermal".to_string(),
                    sim::priority::Consumer::Job(id) => jobs
                        .read()
                        .iter()
                        .find(|j| j.id == *id)
                        .map(|j| j.name.clone())
                        .unwrap_or_else(|| format!("{id:?}")),
                };
                (*c, name, i == 0, i == last)
            })
            .collect()
    };
    let research_unlocked = gs.flags.contains(&Flag::ResearchUnlocked);
    let data_amt = gs.resources.data;
    let research_active: Option<(String, u32, u32)> = gs.research.active.as_ref().map(|a| {
        let total = techs
            .read()
            .iter()
            .find(|t| t.id == a.tech)
            .map(|t| t.research_ticks)
            .unwrap_or(a.ticks_remaining);
        let name = techs
            .read()
            .iter()
            .find(|t| t.id == a.tech)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("{:?}", a.tech));
        (name, a.ticks_remaining, total)
    });
    let codex_unlocked = gs.flags.contains(&Flag::CodexUnlocked);
    let codex_rows: Vec<(String, String)> = gs
        .codex
        .entries
        .iter()
        .rev()
        .map(|d| (d.title.clone(), d.text.clone()))
        .collect();
    // ---- M4 navigation view-model ----
    let shipyard_complete = gs.flags.contains(&Flag::ShipyardComplete);
    let navigation_known = gs.flags.contains(&Flag::NavigationKnown);
    let has_sensor = gs.built_count(sim::module::ModuleId::SensorArray) > 0;
    let away = gs.away();
    let ship_drive = gs.ship.drive_range;
    let ship_cargo_max = gs.ship.cargo;
    let ship_cargo_used = gs.expedition.cargo_total();
    let isotopes = gs.resources.amount(ResourceKind::Isotopes);
    // A textual label of the current activity (or "Docked"/"Holding at <id>").
    let activity_label: String = match &gs.expedition.activity {
        Some(sim::expedition::Activity::Transit { to, ticks }) => {
            format!("In transit to system {} \u{2014} {ticks}s", to.0)
        }
        Some(sim::expedition::Activity::Scan { target, ticks }) => {
            format!("Scanning system {} \u{2014} {ticks}s", target.0)
        }
        Some(sim::expedition::Activity::ResolveSite { system, ticks, .. }) => {
            format!("Working a site in system {} \u{2014} {ticks}s", system.0)
        }
        Some(sim::expedition::Activity::Scavenge { ticks }) => {
            format!("Scavenging fuel \u{2014} {ticks}s")
        }
        None if gs.expedition.location == sim::galaxy::HOME => "Docked at the station".to_string(),
        None => format!("Holding at system {}", gs.expedition.location.0),
    };
    let busy = gs.expedition.activity.is_some();
    let loc = gs.expedition.location;
    // Build a World to regenerate the galaxy for rendering.
    let nav_rows: Vec<NavRow> = if shipyard_complete {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        let galaxy = gs.galaxy(&world);
        let here = galaxy.get(loc).cloned();
        let mut rows: Vec<NavRow> = Vec::new();
        for &sid in gs.galaxy_state.revealed.iter() {
            let id = sim::galaxy::SystemId(sid);
            let Some(sys) = galaxy.get(id) else { continue };
            let dist = here
                .as_ref()
                .map(|h| sim::galaxy::distance(h, sys))
                .unwrap_or(0.0);
            let reachable = id != loc && dist <= ship_drive;
            let cost = sim::galaxy::jump_cost(dist, &t);
            let affordable = gs.resources.fuel >= cost;
            let scanned = gs.galaxy_state.scanned.contains(&sid);
            let sites: Vec<(usize, String, bool, bool)> = gs
                .galaxy_state
                .sites
                .get(&sid)
                .map(|v| {
                    v.iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let landable = matches!(s.kind, sim::galaxy::SiteKind::Planet(_));
                            (i, format!("{:?}", s.kind), s.depleted, landable)
                        })
                        .collect()
                })
                .unwrap_or_default();
            rows.push(NavRow {
                id,
                star: format!("{:?}", sys.star_type),
                pos: sys.pos,
                dist,
                is_current: id == loc,
                reachable,
                affordable,
                scanned,
                cost,
                sites,
            });
        }
        rows.sort_by(|a, b| {
            a.dist
                .partial_cmp(&b.dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows
    } else {
        Vec::new()
    };
    // Stranded? (mirror engine availability so Scavenge only shows when useful)
    let stranded = away && {
        let cheapest = nav_rows
            .iter()
            .filter(|r| r.reachable)
            .map(|r| r.cost)
            .fold(f64::INFINITY, f64::min);
        cheapest.is_infinite() || gs.resources.fuel < cheapest
    };
    let can_upgrade_drive =
        navigation_known && !away && isotopes >= tuning.read().drive_upgrade_isotopes;
    // Atmosphere warning: the survival clock is running (grace below max) while the
    // Atmosphere module is built and crew exist. Crit when grace is nearly gone.
    let atmo_built = gs
        .built
        .get(&sim::module::ModuleId::Atmosphere)
        .copied()
        .unwrap_or(0)
        > 0;
    let grace = gs.atmosphere_grace;
    let grace_max = tuning.read().atmosphere_grace_max;
    let atmo_warning: Option<(u32, bool)> = if atmo_built && gs.crew.total > 0 && grace < grace_max
    {
        Some((grace, grace * 4 <= grace_max)) // crit in the last quarter of the clock
    } else {
        None
    };
    let research_busy = gs.research.active.is_some();
    let research_rows: Vec<(
        sim::research::TechId,
        String,
        String,
        f64,
        u32,
        &'static str,
        bool,
    )> = techs
        .read()
        .iter()
        .map(|t| {
            let done = gs.research.completed.contains(&t.id);
            let prereqs_met = t.requires.iter().all(|r| gs.research.completed.contains(r));
            let st: &'static str = if done {
                "done"
            } else if prereqs_met && !research_busy {
                "available"
            } else {
                "locked"
            };
            let affordable = data_amt >= t.data_cost;
            (
                t.id,
                t.name.clone(),
                t.blurb.clone(),
                t.data_cost,
                t.research_ticks,
                st,
                affordable,
            )
        })
        .collect();
    let landing_vm: Option<LandingVM> = gs.expedition.landing.as_ref().map(|l| {
        let tn = tuning.read();
        let surface = sim::landing::SurfaceSite::generate(
            sim::landing::surface_seed(gs.world_seed, l.system, l.site_idx),
            l.biome,
            &tn,
        );
        let resolved = gs
            .galaxy_state
            .sites
            .get(&l.system.0)
            .and_then(|v| v.get(l.site_idx))
            .map(|s| s.resolved_nodes.clone())
            .unwrap_or_default();
        let cur = surface.get(l.node).cloned();
        let neighbors = cur
            .as_ref()
            .map(|n| {
                n.edges
                    .iter()
                    .filter_map(|&e| {
                        surface
                            .get(e)
                            .map(|nn| (e, format!("{:?}", nn.kind), resolved.contains(&e)))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let nodes = surface
            .nodes
            .iter()
            .map(|n| SurfaceNodeVM {
                id: n.id,
                depth: n.depth,
                kind: format!("{:?}", n.kind),
                resolved: resolved.contains(&n.id),
                is_current: n.id == l.node,
                edges: n.edges.clone(),
            })
            .collect();
        LandingVM {
            biome: format!("{:?}", l.biome),
            suit: l.suit,
            suit_max: tn.suit_max,
            satchel: l
                .satchel
                .iter()
                .map(|(k, v)| (format!("{k:?}"), *v))
                .collect(),
            node: l.node,
            depth: cur.as_ref().map(|n| n.depth).unwrap_or(0),
            encounter: cur
                .as_ref()
                .map(|n| format!("{:?}", n.kind))
                .unwrap_or_default(),
            pending_choice: l.pending_choice,
            traverse: l.traverse,
            neighbors,
            nodes,
            away_crew: gs.expedition.away_team.crew,
            squad_hp: gs.expedition.away_team.hp,
            squad_hp_max: gs.expedition.away_team.crew as f64
                * tn.marine_hp
                * (1.0 + gs.gear.armor_tier as f64 * tn.armor_hp_step),
            morale: gs.expedition.away_team.morale,
            morale_max: tn.morale_max,
            combat: l.combat.as_ref().map(|c| CombatVM {
                enemy_hp: c.enemy_hp,
                stance: format!("{:?}", c.stance),
                shield_ticks: c.shield_ticks,
            }),
            negotiation: l.alien.as_ref().map(|a| {
                use sim::alien::{AlienKind, Band};
                let band = sim::alien::band(a.disposition, &tn);
                let is_peer = a.kind == AlienKind::Peer;
                let not_hostile = band != Band::Hostile;
                let tradeable_band = matches!(band, Band::Neutral | Band::Curious);
                let components = gs
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                NegotiationVM {
                    kind: format!("{:?}", a.kind),
                    is_peer,
                    band: format!("{band:?}"),
                    disposition: a.disposition,
                    can_parley: not_hostile,
                    can_trade: is_peer && tradeable_band && components >= tn.trade_give,
                    can_gift: components >= tn.gift_cost,
                    can_observe: !is_peer && not_hostile,
                    can_harvest: !is_peer,
                }
            }),
            consumables: {
                use sim::gear::ConsumableKind::*;
                [
                    (MedGel, "Med-gel"),
                    (Grenade, "Grenade"),
                    (Shield, "Shield"),
                ]
                .iter()
                .map(|(k, label)| {
                    (
                        *k,
                        label.to_string(),
                        gs.consumables.get(k).copied().unwrap_or(0),
                    )
                })
                .collect()
            },
        }
    });
    let armory_built = gs.built_count(sim::module::ModuleId::Armory) > 0;
    let loadout_vm = if armory_built {
        use sim::gear::ConsumableKind::*;
        Some(LoadoutVM {
            away_crew: gs.expedition.away_team.crew,
            idle: gs.crew.idle(),
            weapon_tier: gs.gear.weapon_tier,
            armor_tier: gs.gear.armor_tier,
            consumables: [
                (MedGel, "Med-gel"),
                (Grenade, "Grenade"),
                (Shield, "Shield"),
            ]
            .iter()
            .map(|(k, label)| {
                (
                    *k,
                    label.to_string(),
                    gs.consumables.get(k).copied().unwrap_or(0),
                )
            })
            .collect(),
        })
    } else {
        None
    };
    let contact_vm = gs.expedition.contact.as_ref().map(|c| {
        use sim::contact::ShipKind;
        let tn = tuning.read();
        let in_combat = c.combat.is_some();
        let is_peer = c.kind == ShipKind::Peer;
        let band = sim::alien::band(c.disposition, &tn);
        let not_hostile = band != sim::alien::Band::Hostile;
        let tradeable = matches!(band, sim::alien::Band::Neutral | sim::alien::Band::Curious);
        let components = gs
            .expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0);
        // flee cost = jump_cost(distance(here, prev_location)). Galaxy needs only seed + tuning
        // (no full World): `Galaxy::generate(gs.world_seed, tn)`. Reuse an in-scope galaxy binding
        // if Console already built one for the nav rows.
        let flee_cost = {
            let g = sim::galaxy::Galaxy::generate(gs.world_seed, &tn);
            match (
                g.get(gs.expedition.location),
                g.get(gs.expedition.prev_location),
            ) {
                (Some(a), Some(b)) => sim::galaxy::jump_cost(sim::galaxy::distance(a, b), &tn),
                _ => sim::galaxy::jump_cost(1.0, &tn),
            }
        };
        let fuel = gs.resources.amount(ResourceKind::Fuel);
        ContactVM {
            kind: format!("{:?}", c.kind),
            is_peer,
            in_combat,
            band: format!("{band:?}"),
            disposition: c.disposition,
            hull: gs.ship.hull,
            hull_max: gs.ship.hull_max(&tn),
            enemy_hp: c.combat.as_ref().map(|cb| cb.enemy_hp).unwrap_or(0.0),
            stance: c
                .combat
                .as_ref()
                .map(|cb| format!("{:?}", cb.stance))
                .unwrap_or_default(),
            repair_drones: gs.ship.repair_drones,
            can_parley: is_peer && !in_combat && not_hostile,
            can_trade: is_peer && !in_combat && tradeable && components >= tn.trade_give,
            can_gift: is_peer && !in_combat && components >= tn.gift_cost,
            can_avoid: is_peer && !in_combat && not_hostile,
            can_flee: fuel >= flee_cost,
        }
    });
    let munitions_vm = if gs.flags.contains(&sim::flags::Flag::MunitionsKnown)
        && gs.built_count(sim::module::ModuleId::Shipyard) > 0
    {
        let tn = tuning.read();
        // VERIFIED: the three upgrade actions spend via `self.resources.try_spend_all(...)` —
        // BOTH Alloy and Components come from station `gs.resources` (NOT expedition.cargo).
        let alloy = gs.resources.amount(ResourceKind::Alloy);
        let components = gs.resources.amount(ResourceKind::Components);
        Some(ShipMunitionsVM {
            weapon_tier: gs.ship.ship_weapon_tier,
            plating_tier: gs.ship.hull_plating_tier,
            hull: gs.ship.hull,
            hull_max: gs.ship.hull_max(&tn),
            repair_drones: gs.ship.repair_drones,
            can_upgrade_weapon: gs.ship.ship_weapon_tier < 2
                && alloy >= tn.ship_weapon_upgrade_alloy
                && components >= tn.ship_weapon_upgrade_components,
            can_upgrade_plating: gs.ship.hull_plating_tier < 2
                && alloy >= tn.hull_plating_upgrade_alloy
                && components >= tn.hull_plating_upgrade_components,
            can_craft_drone: components >= tn.repair_drone_cost,
        })
    } else {
        None
    };
    let endgame_vm = gs.endgame.as_ref().map(|eg| {
        use sim::Ending::*;
        let copy = |e: sim::Ending| -> (&'static str, &'static str) {
            match e {
                Ascendant => (
                    "Press onward",
                    "consume what remains; become the next thing the dark survives",
                ),
                Communion => (
                    "Stand down",
                    "refuse to consume; the harder, quieter future",
                ),
                PatternBroken => (
                    "Stop here",
                    "leave a living world intact; survival without conquest",
                ),
                Cold => (
                    "Let the fire die",
                    "let the reactor go dark; the cycle pauses here",
                ),
            }
        };
        match eg {
            sim::Endgame::WatchedArrived => {
                let tn = tuning.read();
                EndgameVM {
                    ended: false,
                    choices: gs
                        .available_endings(&tn)
                        .into_iter()
                        .map(|e| {
                            let (l, d) = copy(e);
                            (e, l.to_string(), d.to_string())
                        })
                        .collect(),
                    ending_title: String::new(),
                    ending_body: String::new(),
                }
            }
            sim::Endgame::Ended(e) => {
                use sim::codex::{DiscoveryId, NoteId};
                let nid = match e {
                    Ascendant => NoteId::EndingAscendant,
                    Communion => NoteId::EndingCommunion,
                    PatternBroken => NoteId::EndingPatternBroken,
                    Cold => NoteId::EndingCold,
                };
                let entry = gs
                    .codex
                    .entries
                    .iter()
                    .find(|d| d.id == DiscoveryId::Note(nid));
                EndgameVM {
                    ended: true,
                    choices: Vec::new(),
                    ending_title: entry.map(|d| d.title.clone()).unwrap_or_default(),
                    ending_body: entry.map(|d| d.text.clone()).unwrap_or_default(),
                }
            }
        }
    });
    // Whether away-crew can be changed now (docked only).
    let can_set_crew = !gs.away() && !gs.landed();
    // Word-drift: compute expansion tier from a freshly built World (reads only tuning).
    let tier = {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        gs.expansion_tier(&world)
    };
    let charge_ratio = if capacity > 0.0 {
        (charge / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (accent, text_col, warm) = emberfall_stops(tier);
    let starving_label = format!(
        "{} starving \u{2014} rations short",
        crew_word(tier, &crew_noun)
    );
    drop(snapshot);

    let mut act = move |action: Action| {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        if let Some(gs) = state.write().as_mut() {
            let _ = gs.apply(&world, action);
        }
    };

    rsx! {
        div {
            class: "console-root",
            style: "--charge: {charge_ratio}; --tier: {tier}; --accent: {accent}; --text: {text_col}; --warm: {warm};",
        if let Some(egvm) = endgame_vm.clone() {
            EndgamePanel {
                vm: egvm,
                on_choose: move |ending| {
                    let t = tuning.read().clone();
                    let j = JobRegistry(jobs.read().clone());
                    let m = ModuleRegistry(modules.read().clone());
                    let tc = TechRegistry(techs.read().clone());
                    let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                    if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ChooseEnding(ending)); }
                },
                on_restart: move |_| { state.set(None); },
            }
        } else {
        // Core rail — pinned heartbeat bar. ALWAYS visible (charge + status),
        // independent of disclosure. NOT inside a filter: keeps position:sticky working.
        div { class: "rail",
            div { class: "rail-in",
                div {
                    div { class: "core-num",
                        b { "{charge:.0}" }
                        span { class: "core-cap", " / {capacity:.0}" }
                    }
                    div { class: "core-meta",
                        span { class: "dot" }
                        span { "Reactor core" }
                        if shipyard_complete {
                            span { "\u{00b7} ship \u{00b7} drive {ship_drive:.0} \u{00b7} cargo {ship_cargo_used:.0}/{ship_cargo_max:.0} \u{00b7} iso {isotopes:.0}" }
                        }
                    }
                }
                div { class: "status status-{status}", "{status}" }
                div { class: "core-bar", i {} }
            }
        }

        // Life veil — siblings of .rail, filtered together. .rail stays outside.
        div { class: "life-veil",

        if let Some((secs, crit)) = atmo_warning {
            div {
                class: if crit { "ls-warn crit" } else { "ls-warn" },
                if secs > 0 { "ATMOSPHERE FAILING \u{2014} {secs}s of air" } else { "ATMOSPHERE GONE \u{2014} losing crew" }
            }
        }

        // Core action buttons — always rendered, but dimmed/disabled while away
        // (they manage the station, which the engine refuses while exploring).
        div { class: if away { "dimmed" } else { "" },
            if is_cold {
                button { class: "act", onclick: move |_| act(Action::Ignite), "Ignite the core" }
            } else {
                if salvage_unlocked {
                    button {
                        class: "act",
                        disabled: salvage_cooldown > 0,
                        onclick: move |_| act(Action::Salvage),
                        if salvage_cooldown > 0 { "{drift_word(DriftSlot::Salvage, tier)} ({salvage_cooldown}s)" } else { "{drift_word(DriftSlot::Salvage, tier)}" }
                    }
                    button { class: "act", onclick: move |_| act(Action::FeedCore), "{drift_word(DriftSlot::FeedCore, tier)}" }
                }
                if source_available && !source_active {
                    button {
                        class: "act",
                        onclick: move |_| act(Action::ActivateCrewSource),
                        "Activate {source_name}"
                    }
                }
            }
        }
        button {
            class: "act ghost",
            onclick: move |_| {
                if let Some(gs) = state.read().as_ref() { crate::persist::copy_link(gs); }
            },
            "Copy save link"
        }

        if away {
            div { class: "away-banner", "AWAY \u{2014} STATION UNATTENDED \u{00b7} the core runs on without you" }
        }
        if shipyard_complete {
            div { class: "nav-wrap",
                if let Some(vm) = landing_vm.clone() {
                    SurfaceView {
                        vm,
                        tier,
                        on_move: move |node| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::MoveToNode(node)); }
                        },
                        on_harvest: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HarvestLife); }
                        },
                        on_leave: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::LeaveLife); }
                        },
                        on_extract: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Extract); }
                        },
                        on_use_consumable: move |kind| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UseConsumable(kind)); }
                        },
                        on_stance: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() {
                                let next = match gs.expedition.landing.as_ref().and_then(|l| l.combat.as_ref()).map(|c| c.stance) {
                                    Some(sim::combat::Stance::Aggressive) => sim::combat::Stance::Defensive,
                                    _ => sim::combat::Stance::Aggressive,
                                };
                                let _ = gs.apply(&world, Action::SetStance(next));
                            }
                        },
                        on_flee: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Flee); }
                        },
                        on_parley: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Parley); }
                        },
                        on_trade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Trade); }
                        },
                        on_gift: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Gift); }
                        },
                        on_avoid: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Avoid); }
                        },
                        on_observe: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Observe); }
                        },
                        on_harvest_fauna: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Harvest); }
                        },
                        on_attack_alien: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::AttackAlien); }
                        },
                    }
                } else if let Some(cvm) = contact_vm.clone() {
                    ContactPanel {
                        vm: cvm,
                        on_parley: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailParley); }
                        },
                        on_trade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailTrade); }
                        },
                        on_gift: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailGift); }
                        },
                        on_avoid: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailAvoid); }
                        },
                        on_attack: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::AttackShip); }
                        },
                        on_flee: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ShipFlee); }
                        },
                        on_stance: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() {
                                let next = match gs.expedition.contact.as_ref().and_then(|c| c.combat.as_ref()).map(|c| c.stance) {
                                    Some(sim::combat::Stance::Aggressive) => sim::combat::Stance::Defensive,
                                    _ => sim::combat::Stance::Aggressive,
                                };
                                let _ = gs.apply(&world, Action::SetShipStance(next));
                            }
                        },
                        on_repair: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UseRepairDrone); }
                        },
                    }
                } else {
                    NavigationPanel {
                        drive: ship_drive,
                        cargo_used: ship_cargo_used,
                        cargo_max: ship_cargo_max,
                        fuel,
                        isotopes,
                        activity_label: activity_label.clone(),
                        busy,
                        away,
                        has_sensor,
                        stranded,
                        can_upgrade_drive,
                        current: loc,
                        rows: nav_rows.clone(),
                        on_scan: move |id| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ScanSystem(id)); }
                        },
                        on_jump: move |id| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Jump(id)); }
                        },
                        on_resolve: move |(id, idx)| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ResolveSite(id, idx)); }
                        },
                        on_scavenge: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Scavenge); }
                        },
                        on_upgrade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeDrive); }
                        },
                        on_land: move |idx| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Land(idx)); }
                        },
                    }
                }
            }
        }
        div { class: "content-grid",
        div { class: "content-main",
        if build_menu_unlocked || economy_unlocked || power_visible || salvage_unlocked {
            // Two-column dashboard — panels appear via per-panel disclosure flags (§5).
            div { class: "dash",
                div { class: if away { "col dimmed" } else { "col" },
                    if power_visible {
                        PowerMeter {
                            status: status.clone(),
                            charge,
                            capacity,
                            fuel,
                            generation,
                            active_draw,
                        }
                    }
                    if salvage_unlocked {
                        ResourcesPanel { rows: res_rows.clone() }
                    }
                    if codex_unlocked {
                        CodexPanel { entries: codex_rows.clone() }
                    }
                }
                div { class: if away { "col dimmed" } else { "col" },
                    if economy_unlocked {
                        JobPanel {
                            idle,
                            starving,
                            starving_label: starving_label.clone(),
                            rows: job_rows.clone(),
                            on_assign: move |(job, delta)| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::AssignCrew { job, delta });
                                }
                            },
                        }
                    }
                    if build_menu_unlocked {
                        BuildMenu {
                            rows: build_rows.clone(),
                            on_build: move |id| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::Build(id));
                                }
                            },
                        }
                    }
                    if research_unlocked {
                        ResearchPanel {
                            active: research_active.clone(),
                            rows: research_rows.clone(),
                            on_start: move |id| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::StartResearch(id));
                                }
                            },
                        }
                    }
                    if power_visible {
                        PriorityPanel {
                            rows: prio_rows.clone(),
                            on_move: move |(consumer, up)| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::SetPriority { consumer, up });
                                }
                            },
                        }
                    }
                    if let Some(vm) = loadout_vm.clone() {
                        LoadoutPanel {
                            vm,
                            tier,
                            can_set_crew,
                            on_set_crew: move |delta| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::SetAwayCrew { delta }); }
                            },
                            on_upgrade_weapon: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeWeapon); }
                            },
                            on_upgrade_armor: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeArmor); }
                            },
                            on_craft: move |kind| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::CraftConsumable(kind)); }
                            },
                        }
                    }
                    if let Some(vm) = munitions_vm.clone() {
                        ShipMunitionsPanel {
                            vm,
                            on_upgrade_weapon: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeShipWeapon); }
                            },
                            on_upgrade_plating: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeHullPlating); }
                            },
                            on_craft_drone: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::CraftRepairDrone); }
                            },
                        }
                    }
                }
            }
        } else {
            // Reactor readout + buttons are rendered above; this branch is the
            // bare opening (nothing disclosed yet) and adds the intro res line.
            if salvage_unlocked {
                div { class: "res",
                    "Scrap: {scrap:.0}"
                    if crew_arrived { "   \u{00b7}   Crew: {crew}" }
                }
            }
        }
        } // content-main
        // Log rail — persistent right-hand column on wide screens, below on mobile
        div { class: "log-rail",
            div { class: "panel",
                div { class: "eyebrow", "LOG" }
                div { class: "log",
                    for (tick, text) in log_lines {
                        div { class: "log-line", "[{tick}] {text}" }
                    }
                }
            }
        }
        } // content-grid
        } // life-veil
        } // else (no endgame)
        } // console-root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift_word_per_slot_and_tier() {
        // Take slot
        assert_eq!(drift_word(DriftSlot::Take, 0), "Harvest");
        assert_eq!(drift_word(DriftSlot::Take, 1), "Extract");
        assert_eq!(drift_word(DriftSlot::Take, 2), "Process");
        assert_eq!(drift_word(DriftSlot::Take, 3), "Strip");
        // Clamp at 3
        assert_eq!(drift_word(DriftSlot::Take, 4), "Strip");
        assert_eq!(drift_word(DriftSlot::Take, 255), "Strip");

        // Salvage slot
        assert_eq!(drift_word(DriftSlot::Salvage, 0), "Salvage");
        assert_eq!(drift_word(DriftSlot::Salvage, 3), "Render");

        // FeedCore slot
        assert_eq!(drift_word(DriftSlot::FeedCore, 0), "Feed the core");
        assert_eq!(drift_word(DriftSlot::FeedCore, 3), "Burn it");

        // AwayTeam slot
        assert_eq!(drift_word(DriftSlot::AwayTeam, 0), "Marines");
        assert_eq!(drift_word(DriftSlot::AwayTeam, 3), "the asset");
    }

    #[test]
    fn crew_word_origin_at_t0_cold_at_higher() {
        assert_eq!(crew_word(0, "survivor"), "survivor");
        assert_eq!(crew_word(1, "survivor"), "crew");
        assert_eq!(crew_word(2, "survivor"), "units");
        assert_eq!(crew_word(3, "survivor"), "yield");
        // Clamp beyond 3 still returns "yield"
        assert_eq!(crew_word(4, "survivor"), "yield");
    }
}
