use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::simulator::SimulatorError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub oracle: OracleConfig,
    pub initial_state: CombatState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OracleConfig {
    #[serde(rename = "type")]
    pub oracle_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatState {
    pub snapshot_schema: u32,
    pub provenance: Provenance,
    pub rng: RngBankState,
    pub combat: CombatStatus,
    pub decision: Decision,
    pub player: PlayerState,
    #[serde(default)]
    pub enemies: Vec<EnemyState>,
    #[serde(default)]
    pub hand: Vec<CardInstance>,
    #[serde(default)]
    pub draw_pile: Vec<CardInstance>,
    #[serde(default)]
    pub discard_pile: Vec<CardInstance>,
    #[serde(default)]
    pub exhaust_pile: Vec<CardInstance>,
    #[serde(default)]
    pub play_pile: Vec<CardInstance>,
    #[serde(default)]
    pub metrics: Metrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub game_version: String,
    #[serde(default)]
    pub game_commit: String,
    #[serde(default)]
    pub assembly_sha256: String,
    #[serde(default = "base_content_revision")]
    pub content_revision: String,
    #[serde(default)]
    pub modded_gameplay: bool,
}

fn base_content_revision() -> String {
    "base".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RngBankState {
    pub algorithm: String,
    #[serde(default)]
    pub run_seed: String,
    #[serde(default)]
    pub streams: BTreeMap<String, RngStreamState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RngStreamState {
    pub seed: u32,
    pub counter: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatStatus {
    #[serde(default)]
    pub won: bool,
    #[serde(default)]
    pub lost: bool,
    pub turn: u32,
    #[serde(default = "player_side")]
    pub current_side: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ascension_level: Option<u8>,
}

fn player_side() -> String {
    "Player".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    PlayerAction,
    CardSelection {
        choice_id: String,
        #[serde(default)]
        candidates: Vec<String>,
        min: usize,
        max: usize,
    },
    Terminal,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerState {
    pub combat_id: String,
    pub model_id: String,
    pub hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub block: i32,
    pub energy: i32,
    pub max_energy: i32,
    #[serde(default)]
    pub powers: Vec<PowerState>,
    #[serde(default)]
    pub relics: Vec<ModelState>,
    #[serde(default)]
    pub potions: Vec<ModelState>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyState {
    pub combat_id: String,
    pub model_id: String,
    pub hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub block: i32,
    #[serde(default)]
    pub powers: Vec<PowerState>,
    pub ai: EnemyAiState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyAiState {
    pub current_move: String,
    #[serde(default)]
    pub move_history: Vec<String>,
    #[serde(default)]
    pub is_front: bool,
    #[serde(default)]
    pub is_alone: bool,
    #[serde(default)]
    pub tough_enemies: bool,
    #[serde(default)]
    pub deadly_enemies: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardInstance {
    pub instance_id: String,
    pub model_id: String,
    #[serde(default)]
    pub upgrade_level: u8,
    pub cost: i32,
    #[serde(default)]
    pub cost_for_turn: Option<i32>,
    #[serde(default)]
    pub retained: bool,
    #[serde(default)]
    pub exhausts: bool,
    #[serde(default)]
    pub ethereal: bool,
}

impl CardInstance {
    pub fn effective_cost(&self) -> i32 {
        self.cost_for_turn.unwrap_or(self.cost)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PowerState {
    pub model_id: String,
    pub amount: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelState {
    pub model_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    #[serde(default)]
    pub powers_played: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub combat_card_index: Option<String>,
    #[serde(default)]
    pub target_combat_id: Option<String>,
    #[serde(default)]
    pub cost: Option<i32>,
    #[serde(default)]
    pub choice_id: Option<String>,
    #[serde(default)]
    pub selection: Vec<String>,
}

pub fn validate_scenario_json(input: &str) -> Result<String, SimulatorError> {
    let scenario: Scenario = serde_json::from_str(input)?;
    crate::Simulator::validate_scenario(&scenario)?;
    Ok(serde_json::json!({"ok": true, "snapshot_schema": 2}).to_string())
}
