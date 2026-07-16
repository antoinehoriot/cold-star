#[derive(Clone, PartialEq)]
pub(crate) struct NavRow {
    pub(crate) id: sim::galaxy::SystemId,
    pub(crate) star: String,
    pub(crate) pos: (f64, f64),
    pub(crate) dist: f64,
    pub(crate) is_current: bool,
    pub(crate) reachable: bool,
    pub(crate) affordable: bool,
    pub(crate) scanned: bool,
    pub(crate) cost: f64,
    // (site_index, kind label, depleted, landable)
    pub(crate) sites: Vec<(usize, String, bool, bool)>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct SurfaceNodeVM {
    pub(crate) id: u32,
    pub(crate) depth: u32,
    pub(crate) kind: String,
    pub(crate) resolved: bool,
    pub(crate) is_current: bool,
    pub(crate) edges: Vec<u32>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct CombatVM {
    pub(crate) enemy_hp: f64,
    pub(crate) stance: String,
    pub(crate) shield_ticks: u32,
}

#[derive(Clone, PartialEq)]
pub(crate) struct NegotiationVM {
    pub(crate) kind: String, // "Fauna" | "Peer"
    pub(crate) is_peer: bool,
    pub(crate) band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    pub(crate) disposition: f64,
    // disable predicates — exact duals of the engine's apply() rejections
    pub(crate) can_parley: bool,
    pub(crate) can_trade: bool,
    pub(crate) can_gift: bool,
    pub(crate) can_observe: bool,
    pub(crate) can_harvest: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct LandingVM {
    pub(crate) biome: String,
    pub(crate) suit: f64,
    pub(crate) suit_max: f64,
    pub(crate) satchel: Vec<(String, f64)>,
    pub(crate) depth: u32,
    pub(crate) encounter: String,
    pub(crate) pending_choice: bool,
    pub(crate) traverse: u32,
    pub(crate) nodes: Vec<SurfaceNodeVM>,
    pub(crate) away_crew: u32,
    pub(crate) squad_hp: f64,
    pub(crate) squad_hp_max: f64,
    pub(crate) morale: f64,
    pub(crate) morale_max: f64,
    pub(crate) combat: Option<CombatVM>,
    pub(crate) negotiation: Option<NegotiationVM>,
    pub(crate) consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct LoadoutVM {
    pub(crate) away_crew: u32,
    pub(crate) idle: u32,
    pub(crate) weapon_tier: u8,
    pub(crate) armor_tier: u8,
    // (kind, label, count) for each consumable kind
    pub(crate) consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct ContactVM {
    pub(crate) kind: String, // "Peer" | "Remnant"
    pub(crate) is_peer: bool,
    pub(crate) in_combat: bool,
    pub(crate) band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    pub(crate) disposition: f64,
    pub(crate) hull: f64,
    pub(crate) hull_max: f64,
    pub(crate) enemy_hp: f64,
    pub(crate) stance: String,
    pub(crate) repair_drones: u32,
    // disable predicates — exact duals of the engine rejections
    pub(crate) can_parley: bool,
    pub(crate) can_trade: bool,
    pub(crate) can_gift: bool,
    pub(crate) can_avoid: bool,
    pub(crate) can_flee: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct ShipMunitionsVM {
    pub(crate) weapon_tier: u8,
    pub(crate) plating_tier: u8,
    pub(crate) hull: f64,
    pub(crate) hull_max: f64,
    pub(crate) repair_drones: u32,
    pub(crate) can_upgrade_weapon: bool,
    pub(crate) can_upgrade_plating: bool,
    pub(crate) can_craft_drone: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct EndgameVM {
    pub(crate) ended: bool,
    // WatchedArrived: the earned choices — (ending, button label, one-line meaning)
    pub(crate) choices: Vec<(sim::Ending, String, String)>,
    // Ended: the chosen ending's per-origin closing note (read from the codex)
    pub(crate) ending_title: String,
    pub(crate) ending_body: String,
}
