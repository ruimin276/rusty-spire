use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rusty_spire_core::{
    self as rng, CardInstance, CombatState, CombatStatus, Decision, EnemyAiState, EnemyState,
    Metrics, ModelState, PlayerState, Provenance, RngBankState, RngStreamState,
};
use rusty_spire_data::{CombatCatalog, MonsterDefinition, RelicCombatEffect, hex_sha256};

use crate::{Simulator, SimulatorError};

pub const SETUP_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyPolicyKind {
    #[default]
    MinimizeHpLoss,
}

impl LegacyPolicyKind {
    pub fn validate(self) -> Result<(), SimulatorError> {
        match self {
            Self::MinimizeHpLoss => Ok(()),
        }
    }
}
const STREAM_NAMES: &[&str] = &[
    "up_front",
    "shuffle",
    "unknown_map_point",
    "combat_card_generation",
    "combat_potion_generation",
    "combat_card_selection",
    "combat_energy_costs",
    "combat_targets",
    "monster_ai",
    "niche",
    "combat_orbs",
    "treasure_room_relics",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSetupV1 {
    pub schema_version: u32,
    pub catalog_sha256: String,
    pub ascension_level: u8,
    pub rng: SetupRng,
    pub character: CharacterSetup,
    pub deck: Vec<DeckEntry>,
    pub relics: Vec<RelicSetup>,
    pub potions: Vec<DeferredModelSetup>,
    pub encounter: EncounterSetup,
    #[serde(default)]
    pub policy: LegacyPolicyKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetupRng {
    pub run_seed: String,
    pub profile: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stream_overrides: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterSetup {
    pub id: String,
    pub current_hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub max_energy: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeckEntry {
    pub id: String,
    pub quantity: u16,
    #[serde(default)]
    pub upgrade_level: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelicSetup {
    pub id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub state: BTreeMap<String, i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeferredModelSetup {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EncounterSetup {
    Catalog { id: String },
    Custom { enemies: Vec<EnemySetup> },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnemySetup {
    pub id: String,
    #[serde(default)]
    pub current_hp: Option<i32>,
    #[serde(default)]
    pub max_hp: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct InitializedCombat {
    pub state: CombatState,
    pub setup_hash: String,
    pub policy: LegacyPolicyKind,
}

pub fn initialize(
    catalog: &CombatCatalog,
    setup: &CombatSetupV1,
    allow_debug_rng_overrides: bool,
) -> Result<InitializedCombat, SimulatorError> {
    validate_setup(catalog, setup, allow_debug_rng_overrides)?;
    let profile = &catalog.data.rng_profiles[&setup.rng.profile];
    let mut streams = derive_streams(
        &setup.rng.run_seed,
        &profile.stream_derivation,
        &setup.rng.stream_overrides,
    )?;
    let character = &catalog.data.characters[&setup.character.id];
    let max_energy = setup.character.max_energy.unwrap_or(character.max_energy);
    let relics = setup
        .relics
        .iter()
        .map(|relic| ModelState {
            model_id: relic.id.clone(),
        })
        .collect();
    let mut cards = Vec::new();
    for entry in &setup.deck {
        let definition = &catalog.data.cards[&entry.id];
        for _ in 0..entry.quantity {
            cards.push(CardInstance {
                instance_id: cards.len().to_string(),
                model_id: entry.id.clone(),
                upgrade_level: entry.upgrade_level,
                cost: definition.cost,
                cost_for_turn: None,
                retained: false,
                exhausts: definition.keywords.iter().any(|value| value == "Exhaust"),
                ethereal: definition.keywords.iter().any(|value| value == "Ethereal"),
            });
        }
    }
    let enemy_ids = match &setup.encounter {
        EncounterSetup::Catalog { id } => catalog.data.encounters[id].enemies.clone(),
        EncounterSetup::Custom { enemies } => {
            enemies.iter().map(|enemy| enemy.id.clone()).collect()
        }
    };
    let custom = match &setup.encounter {
        EncounterSetup::Custom { enemies } => Some(enemies.as_slice()),
        EncounterSetup::Catalog { .. } => None,
    };
    let mut enemies = Vec::new();
    for (index, id) in enemy_ids.iter().enumerate() {
        let definition = &catalog.data.monsters[id];
        let overrides = custom.and_then(|values| values.get(index));
        let max_hp = if let Some(value) = overrides.and_then(|enemy| enemy.max_hp) {
            value
        } else {
            roll_enemy_hp(
                definition,
                setup.ascension_level,
                catalog.data.ascensions.monster_hp_level,
                streams
                    .get_mut("monster_ai")
                    .expect("derived profiles contain monster_ai"),
                &profile.algorithm,
            )
        };
        let current_hp = overrides
            .and_then(|enemy| enemy.current_hp)
            .unwrap_or(max_hp);
        if current_hp > max_hp {
            return Err(SimulatorError::InvalidSetup(format!(
                "current HP {current_hp} exceeds maximum HP {max_hp} for {id}"
            )));
        }
        enemies.push(EnemyState {
            combat_id: format!("enemy_{index}"),
            model_id: id.clone(),
            hp: current_hp,
            max_hp,
            block: 0,
            powers: Vec::new(),
            ai: EnemyAiState {
                current_move: definition.opening_move.clone(),
                move_history: Vec::new(),
                is_front: index == 0,
                is_alone: enemy_ids.len() == 1,
                tough_enemies: setup.ascension_level >= catalog.data.ascensions.tough_enemies_level,
                deadly_enemies: setup.ascension_level
                    >= catalog.data.ascensions.deadly_enemies_level,
            },
        });
    }
    let state = CombatState {
        snapshot_schema: 2,
        provenance: Provenance {
            game_version: catalog.data.source.game_version.clone(),
            game_commit: String::new(),
            assembly_sha256: catalog.sha256.clone(),
            content_revision: "base".into(),
            modded_gameplay: false,
        },
        rng: RngBankState {
            algorithm: profile.algorithm.clone(),
            run_seed: setup.rng.run_seed.clone(),
            streams,
        },
        combat: CombatStatus {
            won: false,
            lost: false,
            turn: 1,
            current_side: "Player".into(),
            ascension_level: Some(setup.ascension_level),
        },
        decision: Decision::PlayerAction,
        player: PlayerState {
            combat_id: "player".into(),
            model_id: setup.character.id.clone(),
            hp: setup.character.current_hp,
            max_hp: setup.character.max_hp,
            block: 0,
            energy: max_energy,
            max_energy,
            powers: Vec::new(),
            relics,
            potions: Vec::new(),
        },
        enemies,
        hand: Vec::new(),
        draw_pile: cards,
        discard_pile: Vec::new(),
        exhaust_pile: Vec::new(),
        play_pile: Vec::new(),
        metrics: Metrics::default(),
    };
    let state = Simulator::new(catalog).prepare_combat_start(&state)?;
    Ok(InitializedCombat {
        state,
        setup_hash: hex_sha256(&serde_json::to_vec(setup)?),
        policy: setup.policy,
    })
}

fn validate_setup(
    catalog: &CombatCatalog,
    setup: &CombatSetupV1,
    allow_debug_rng_overrides: bool,
) -> Result<(), SimulatorError> {
    if setup.schema_version != SETUP_SCHEMA_VERSION {
        return Err(SimulatorError::InvalidSetup(format!(
            "schema_version must be {SETUP_SCHEMA_VERSION}, got {}",
            setup.schema_version
        )));
    }
    if setup.catalog_sha256 != catalog.sha256 {
        return Err(SimulatorError::CatalogMismatch {
            expected: setup.catalog_sha256.clone(),
            actual: catalog.sha256.clone(),
        });
    }
    if setup.ascension_level > catalog.data.ascensions.max_supported_level {
        return Err(SimulatorError::InvalidSetup(format!(
            "ascension_level must be between 0 and {}",
            catalog.data.ascensions.max_supported_level
        )));
    }
    if !catalog.data.rng_profiles.contains_key(&setup.rng.profile) {
        return Err(SimulatorError::UnsupportedMechanic(format!(
            "rng profile {}",
            setup.rng.profile
        )));
    }
    if !setup.rng.stream_overrides.is_empty() && !allow_debug_rng_overrides {
        return Err(SimulatorError::InvalidSetup(
            "rng.stream_overrides requires the debug override flag".into(),
        ));
    }
    if setup.character.current_hp <= 0
        || setup.character.max_hp <= 0
        || setup.character.current_hp > setup.character.max_hp
    {
        return Err(SimulatorError::InvalidSetup(
            "character HP must satisfy 0 < current_hp <= max_hp".into(),
        ));
    }
    if !catalog.data.characters.contains_key(&setup.character.id) {
        return Err(SimulatorError::UnknownId(setup.character.id.clone()));
    }
    if setup.character.max_energy.is_some_and(|value| value <= 0) {
        return Err(SimulatorError::InvalidSetup(
            "character max_energy override must be positive".into(),
        ));
    }
    if setup.deck.is_empty() {
        return Err(SimulatorError::InvalidSetup("deck cannot be empty".into()));
    }
    for entry in &setup.deck {
        if entry.quantity == 0 {
            return Err(SimulatorError::InvalidSetup(format!(
                "deck entry {} has zero quantity",
                entry.id
            )));
        }
        if entry.upgrade_level > 1 {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "upgrade level {} on {}",
                entry.upgrade_level, entry.id
            )));
        }
        if !catalog.data.cards.contains_key(&entry.id) {
            return Err(SimulatorError::UnknownId(entry.id.clone()));
        }
    }
    let mut relic_ids = std::collections::HashSet::new();
    for relic in &setup.relics {
        if !relic_ids.insert(relic.id.as_str()) {
            return Err(SimulatorError::InvalidSetup(format!(
                "duplicate relic {}",
                relic.id
            )));
        }
        let Some(definition) = catalog.data.relics.get(&relic.id) else {
            return Err(SimulatorError::UnknownId(relic.id.clone()));
        };
        if !relic.state.is_empty() {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "stateful relic {}",
                relic.id
            )));
        }
        if !matches!(
            definition.combat_effect,
            RelicCombatEffect::Inert | RelicCombatEffect::AdditionalOpeningDraw { .. }
        ) {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "relic {}",
                relic.id
            )));
        }
    }
    if !setup.potions.is_empty() {
        return Err(SimulatorError::UnsupportedMechanic("potions".into()));
    }
    let enemies: &[EnemySetup] = match &setup.encounter {
        EncounterSetup::Catalog { id } => {
            let Some(encounter) = catalog.data.encounters.get(id) else {
                return Err(SimulatorError::UnknownId(id.clone()));
            };
            if encounter.enemies.len() != 1 {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "multi-enemy encounter {id}"
                )));
            }
            &[]
        }
        EncounterSetup::Custom { enemies } => {
            if enemies.len() != 1 {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "multi-enemy custom encounter with {} enemies",
                    enemies.len()
                )));
            }
            enemies
        }
    };
    for enemy in enemies {
        if !catalog.data.monsters.contains_key(&enemy.id) {
            return Err(SimulatorError::UnknownId(enemy.id.clone()));
        }
        if enemy.current_hp.is_some_and(|value| value <= 0)
            || enemy.max_hp.is_some_and(|value| value <= 0)
            || matches!((enemy.current_hp, enemy.max_hp), (Some(a), Some(b)) if a > b)
        {
            return Err(SimulatorError::InvalidSetup(format!(
                "invalid HP override for {}",
                enemy.id
            )));
        }
    }
    setup.policy.validate()?;
    Ok(())
}

fn derive_streams(
    run_seed: &str,
    derivation: &str,
    overrides: &BTreeMap<String, u32>,
) -> Result<BTreeMap<String, RngStreamState>, SimulatorError> {
    let mut streams = BTreeMap::new();
    for name in STREAM_NAMES {
        let seed = match derivation {
            "numeric_seed_domain_v1" => {
                let base = run_seed.parse::<u32>().map_err(|_| {
                    SimulatorError::InvalidSetup(
                        "numeric_seed_domain_v1 requires a decimal u32 run_seed".into(),
                    )
                })?;
                if *name == "shuffle" {
                    base
                } else {
                    rng::domain_seed(base, name)
                }
            }
            other => {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "rng stream derivation {other}"
                )));
            }
        };
        streams.insert(
            (*name).to_owned(),
            RngStreamState {
                seed: overrides.get(*name).copied().unwrap_or(seed),
                counter: 0,
            },
        );
    }
    for name in overrides.keys() {
        if !streams.contains_key(name) {
            return Err(SimulatorError::InvalidSetup(format!(
                "unknown RNG stream override {name}"
            )));
        }
    }
    Ok(streams)
}

fn roll_enemy_hp(
    definition: &MonsterDefinition,
    ascension_level: u8,
    monster_hp_level: u8,
    stream: &mut RngStreamState,
    algorithm: &str,
) -> i32 {
    let range = if ascension_level >= monster_hp_level {
        &definition.ascension_hp
    } else {
        &definition.hp
    };
    let width = (range.max - range.min + 1) as u32;
    range.min + rng::next_int(algorithm, stream, width) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Simulator;

    fn catalog() -> CombatCatalog {
        CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json")).unwrap()
    }

    fn setup(catalog: &CombatCatalog) -> CombatSetupV1 {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json"
        ))
        .unwrap();
        value["catalog_sha256"] = catalog.sha256.clone().into();
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn initializes_stable_instances_and_rng_vectors() {
        let catalog = catalog();
        let combat = initialize(&catalog, &setup(&catalog), false).unwrap();
        let mut cards = combat
            .state
            .hand
            .iter()
            .chain(&combat.state.draw_pile)
            .map(|card| card.instance_id.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        cards.sort_unstable();
        assert_eq!(cards, (0..12).collect::<Vec<_>>());
        assert_eq!(combat.state.hand.len(), 7);
        assert_eq!(combat.state.rng.streams["shuffle"].seed, 1);
        assert_eq!(combat.state.rng.streams["shuffle"].counter, 11);
        assert_eq!(combat.state.rng.streams["monster_ai"].counter, 0);
        assert_eq!(combat.state.rng.streams["combat_targets"].counter, 0);
    }

    #[test]
    fn identical_setup_replays_opening_actions_and_hash() {
        let catalog = catalog();
        let first = initialize(&catalog, &setup(&catalog), false).unwrap();
        let second = initialize(&catalog, &setup(&catalog), false).unwrap();
        let simulator = Simulator::new(&catalog);
        assert_eq!(
            simulator.state_hash(&first.state).unwrap(),
            simulator.state_hash(&second.state).unwrap()
        );
        assert_eq!(
            simulator.legal_actions(&first.state).unwrap(),
            simulator.legal_actions(&second.state).unwrap()
        );
    }

    #[test]
    fn catalog_encounter_rolls_omitted_hp_from_monster_stream() {
        let catalog = catalog();
        let mut value = setup(&catalog);
        value.encounter = EncounterSetup::Catalog {
            id: "ENCOUNTER.NIBBITS_WEAK".into(),
        };
        let combat = initialize(&catalog, &value, false).unwrap();
        assert!((42..=46).contains(&combat.state.enemies[0].max_hp));
        assert_eq!(combat.state.rng.streams["monster_ai"].counter, 1);

        value.ascension_level = catalog.data.ascensions.monster_hp_level - 1;
        let before_tough = initialize(&catalog, &value, false).unwrap();
        assert!((42..=46).contains(&before_tough.state.enemies[0].max_hp));
        value.ascension_level = catalog.data.ascensions.monster_hp_level;
        let tough = initialize(&catalog, &value, false).unwrap();
        assert!((44..=48).contains(&tough.state.enemies[0].max_hp));
        assert_eq!(
            tough.state.enemies[0].max_hp,
            before_tough.state.enemies[0].max_hp + 2
        );
    }

    #[test]
    fn rejects_catalog_mismatch_deferred_fields_and_debug_overrides() {
        let catalog = catalog();
        let mut value = setup(&catalog);
        value.catalog_sha256 = "0".repeat(64);
        assert!(matches!(
            initialize(&catalog, &value, false),
            Err(SimulatorError::CatalogMismatch { .. })
        ));

        let mut value = setup(&catalog);
        value.potions.push(DeferredModelSetup {
            id: "POTION.TEST".into(),
        });
        assert!(matches!(
            initialize(&catalog, &value, false),
            Err(SimulatorError::UnsupportedMechanic(_))
        ));

        let mut value = setup(&catalog);
        value.rng.stream_overrides.insert("shuffle".into(), 2);
        assert!(matches!(
            initialize(&catalog, &value, false),
            Err(SimulatorError::InvalidSetup(_))
        ));
        assert!(initialize(&catalog, &value, true).is_ok());
    }

    #[test]
    fn rejects_unknown_ids_and_multi_enemy_execution() {
        let catalog = catalog();
        let mut value = setup(&catalog);
        value.deck[0].id = "CARD.UNKNOWN".into();
        assert!(matches!(
            initialize(&catalog, &value, false),
            Err(SimulatorError::UnknownId(_))
        ));

        let mut value = setup(&catalog);
        value.encounter = EncounterSetup::Catalog {
            id: "ENCOUNTER.NIBBITS_NORMAL".into(),
        };
        assert!(matches!(
            initialize(&catalog, &value, false),
            Err(SimulatorError::UnsupportedMechanic(_))
        ));
    }

    #[test]
    fn strict_schema_rejects_drift() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json"
        ))
        .unwrap();
        value["unexpected"] = true.into();
        assert!(serde_json::from_value::<CombatSetupV1>(value).is_err());
    }
}
