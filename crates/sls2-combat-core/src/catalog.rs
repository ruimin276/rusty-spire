use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::simulator::SimulatorError;

pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct CombatCatalog {
    pub data: CombatCatalogV1,
    pub sha256: String,
}

impl CombatCatalog {
    pub fn from_json(input: &[u8]) -> Result<Self, SimulatorError> {
        let data: CombatCatalogV1 = serde_json::from_slice(input)?;
        if data.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(SimulatorError::Catalog(format!(
                "catalog schema_version must be {CATALOG_SCHEMA_VERSION}, got {}",
                data.schema_version
            )));
        }
        data.validate()?;
        Ok(Self {
            data,
            sha256: hex_sha256(input),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatCatalogV1 {
    pub schema_version: u32,
    pub source: CatalogSource,
    pub rng_profiles: BTreeMap<String, RngProfileDefinition>,
    pub characters: BTreeMap<String, CharacterDefinition>,
    pub cards: BTreeMap<String, CardDefinition>,
    pub relics: BTreeMap<String, RelicDefinition>,
    pub powers: BTreeMap<String, PowerDefinition>,
    pub monsters: BTreeMap<String, MonsterDefinition>,
    pub encounters: BTreeMap<String, EncounterDefinition>,
    pub ascensions: AscensionRules,
    pub combat_modifiers: CombatModifiers,
}

impl CombatCatalogV1 {
    fn validate(&self) -> Result<(), SimulatorError> {
        if self.source.channel != "stable" && self.source.channel != "beta" {
            return Err(SimulatorError::Catalog(format!(
                "unknown source channel {}",
                self.source.channel
            )));
        }
        if self.rng_profiles.is_empty()
            || self.characters.is_empty()
            || self.cards.is_empty()
            || self.powers.is_empty()
            || self.monsters.is_empty()
        {
            return Err(SimulatorError::Catalog(
                "catalog must contain RNG profiles, characters, cards, and monsters".into(),
            ));
        }
        for (id, profile) in &self.rng_profiles {
            if profile.algorithm != "xoshiro256_star_star_v1"
                || profile.stream_derivation != "numeric_seed_domain_v1"
            {
                return Err(SimulatorError::Catalog(format!(
                    "RNG profile {id} uses an unsupported adapter"
                )));
            }
        }
        for (id, power) in &self.powers {
            if !matches!(power.stack_behavior.as_str(), "amount" | "duration") {
                return Err(SimulatorError::Catalog(format!(
                    "power {id} has unsupported stack behavior {}",
                    power.stack_behavior
                )));
            }
        }
        for (id, card) in &self.cards {
            if let Some(power) = &card.power
                && !self.powers.contains_key(&power.id)
            {
                return Err(SimulatorError::Catalog(format!(
                    "card {id} references unknown power {}",
                    power.id
                )));
            }
        }
        for (id, monster) in &self.monsters {
            if monster.hp.min <= 0
                || monster.hp.min > monster.hp.max
                || monster.ascension_hp.min <= 0
                || monster.ascension_hp.min > monster.ascension_hp.max
            {
                return Err(SimulatorError::Catalog(format!(
                    "monster {id} has invalid HP ranges"
                )));
            }
            if !monster.moves.contains_key(&monster.opening_move) {
                return Err(SimulatorError::Catalog(format!(
                    "monster {id} has unknown opening move {}",
                    monster.opening_move
                )));
            }
            for (move_id, movement) in &monster.moves {
                if !monster.moves.contains_key(&movement.next_move) {
                    return Err(SimulatorError::Catalog(format!(
                        "monster {id} move {move_id} references unknown next move {}",
                        movement.next_move
                    )));
                }
                if let Some(power) = &movement.power
                    && !self.powers.contains_key(&power.id)
                {
                    return Err(SimulatorError::Catalog(format!(
                        "monster {id} move {move_id} references unknown power {}",
                        power.id
                    )));
                }
                if let Some(power) = &movement.power
                    && !matches!(power.target.as_str(), "self" | "player")
                {
                    return Err(SimulatorError::Catalog(format!(
                        "monster {id} move {move_id} has unsupported target {}",
                        power.target
                    )));
                }
            }
        }
        self.ascensions.validate()?;
        for (id, encounter) in &self.encounters {
            if encounter.enemies.is_empty() {
                return Err(SimulatorError::Catalog(format!(
                    "encounter {id} must contain at least one enemy"
                )));
            }
            for enemy in &encounter.enemies {
                if !self.monsters.contains_key(enemy) {
                    return Err(SimulatorError::Catalog(format!(
                        "encounter {id} references unknown monster {enemy}"
                    )));
                }
            }
        }
        self.combat_modifiers.weak.validate("weak")?;
        self.combat_modifiers.shrink.validate("shrink")?;
        self.combat_modifiers.vulnerable.validate("vulnerable")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSource {
    pub name: String,
    pub channel: String,
    pub game_version: String,
    pub content_sha256: String,
    pub retrieved_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RngProfileDefinition {
    pub algorithm: String,
    pub stream_derivation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDefinition {
    pub max_energy: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardDefinition {
    pub cost: i32,
    #[serde(default)]
    pub damage: Option<UpgradableValue>,
    #[serde(default)]
    pub block: Option<UpgradableValue>,
    #[serde(default)]
    pub power: Option<CardPowerDefinition>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradableValue {
    pub base: i32,
    pub upgraded: i32,
}

impl UpgradableValue {
    pub fn at(&self, upgrade_level: u8) -> i32 {
        if upgrade_level > 0 {
            self.upgraded
        } else {
            self.base
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardPowerDefinition {
    pub id: String,
    pub amount: UpgradableValue,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelicDefinition {
    pub combat_effect: RelicCombatEffect,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PowerDefinition {
    pub source_id: String,
    pub stack_behavior: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelicCombatEffect {
    Inert,
    AdditionalOpeningDraw { amount: usize },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterDefinition {
    pub hp: RangeDefinition,
    pub ascension_hp: RangeDefinition,
    pub opening_move: String,
    pub moves: BTreeMap<String, MonsterMoveDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RangeDefinition {
    pub min: i32,
    pub max: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterMoveDefinition {
    #[serde(default)]
    pub damage: Option<AscensionValue>,
    #[serde(default)]
    pub block: Option<AscensionValue>,
    #[serde(default)]
    pub power: Option<MonsterPowerDefinition>,
    pub next_move: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AscensionValue {
    pub base: i32,
    pub ascension: i32,
}

impl AscensionValue {
    pub fn at(&self, ascended: bool) -> i32 {
        if ascended { self.ascension } else { self.base }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterPowerDefinition {
    pub id: String,
    pub amount: AscensionValue,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterDefinition {
    pub enemies: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AscensionRules {
    pub max_supported_level: u8,
    pub monster_hp_level: u8,
    pub tough_enemies_level: u8,
    pub deadly_enemies_level: u8,
}

impl AscensionRules {
    fn validate(&self) -> Result<(), SimulatorError> {
        if self.monster_hp_level > self.max_supported_level
            || self.tough_enemies_level > self.max_supported_level
            || self.deadly_enemies_level > self.max_supported_level
        {
            return Err(SimulatorError::Catalog(
                "ascension thresholds exceed max_supported_level".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatModifiers {
    pub weak: RationalMultiplier,
    pub shrink: RationalMultiplier,
    pub vulnerable: RationalMultiplier,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RationalMultiplier {
    pub numerator: i32,
    pub denominator: i32,
}

impl RationalMultiplier {
    fn validate(&self, name: &str) -> Result<(), SimulatorError> {
        if self.numerator < 0 || self.denominator <= 0 {
            return Err(SimulatorError::Catalog(format!(
                "{name} multiplier must have numerator >= 0 and denominator > 0"
            )));
        }
        Ok(())
    }

    pub fn apply_floor(&self, value: i32) -> i32 {
        value * self.numerator / self.denominator
    }
}

pub fn hex_sha256(input: &[u8]) -> String {
    format!("{:x}", Sha256::digest(input))
}
