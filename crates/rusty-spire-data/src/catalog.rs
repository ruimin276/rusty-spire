use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::DataError;

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const DATA_PACKAGE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct CombatCatalog {
    pub data: CombatCatalogV1,
    pub sha256: String,
    pub package_id: String,
}

impl CombatCatalog {
    pub fn from_json(input: &[u8]) -> Result<Self, DataError> {
        let value: serde_json::Value = serde_json::from_slice(input)?;
        let (data, package_id) = if value.get("manifest").is_some() {
            let package: DataPackageV1 = serde_json::from_value(value)?;
            package.validate()?;
            let package_id = package.manifest.package_id.clone();
            (package.into_legacy_catalog(), package_id)
        } else {
            let mut data: CombatCatalogV1 = serde_json::from_value(value)?;
            promote_v02_legacy_effects(&mut data);
            let package_id = format!(
                "spire-codex-{}",
                data.source.game_version.trim_start_matches('v')
            );
            (data, package_id)
        };
        if data.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(DataError::Invalid(format!(
                "catalog schema_version must be {CATALOG_SCHEMA_VERSION}, got {}",
                data.schema_version
            )));
        }
        data.validate()?;
        Ok(Self {
            data,
            sha256: hex_sha256(input),
            package_id,
        })
    }
}

fn promote_v02_legacy_effects(data: &mut CombatCatalogV1) {
    for (id, card) in &mut data.cards {
        if !card.effects.is_empty() || id == "CARD.ASCENDERS_BANE" {
            continue;
        }
        if let Some(damage) = card.damage.clone() {
            card.effects
                .push(CardEffectDefinition::Damage { amount: damage });
        }
        if let Some(block) = card.block.clone() {
            card.effects
                .push(CardEffectDefinition::Block { amount: block });
        }
        if let Some(power) = card.power.clone() {
            card.effects.push(CardEffectDefinition::ApplyPower {
                id: power.id,
                amount: power.amount,
            });
        }
        if id == "CARD.SURVIVOR" {
            card.effects
                .push(CardEffectDefinition::Discard { amount: 1 });
        }
    }
}

/// Runtime package handle. `CombatCatalog` remains as the v0.2 source alias.
pub type DataPackage = CombatCatalog;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataPackageV1 {
    pub manifest: DataPackageManifestV1,
    pub cards: BTreeMap<String, CardDefinition>,
    pub actors: ActorContentV1,
    pub items: ItemContentV1,
    pub encounters: BTreeMap<String, EncounterDefinition>,
    pub rules: RuleContentV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataPackageManifestV1 {
    pub schema_version: u32,
    pub package_id: String,
    pub source: CatalogSource,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActorContentV1 {
    pub characters: BTreeMap<String, CharacterDefinition>,
    pub monsters: BTreeMap<String, MonsterDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemContentV1 {
    pub relics: BTreeMap<String, RelicDefinition>,
    pub powers: BTreeMap<String, PowerDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleContentV1 {
    pub rng_profiles: BTreeMap<String, RngProfileDefinition>,
    pub ascensions: AscensionRules,
    pub combat_modifiers: CombatModifiers,
}

impl DataPackageV1 {
    fn validate(&self) -> Result<(), DataError> {
        if self.manifest.schema_version != DATA_PACKAGE_SCHEMA_VERSION {
            return Err(DataError::Invalid(format!(
                "data package schema_version must be {DATA_PACKAGE_SCHEMA_VERSION}, got {}",
                self.manifest.schema_version
            )));
        }
        if self.manifest.package_id.is_empty() {
            return Err(DataError::Invalid("package_id cannot be empty".into()));
        }
        self.clone().into_legacy_catalog().validate()
    }

    fn into_legacy_catalog(self) -> CombatCatalogV1 {
        CombatCatalogV1 {
            schema_version: CATALOG_SCHEMA_VERSION,
            source: self.manifest.source,
            rng_profiles: self.rules.rng_profiles,
            characters: self.actors.characters,
            cards: self.cards,
            relics: self.items.relics,
            powers: self.items.powers,
            monsters: self.actors.monsters,
            encounters: self.encounters,
            ascensions: self.rules.ascensions,
            combat_modifiers: self.rules.combat_modifiers,
        }
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
    pub fn validate(&self) -> Result<(), DataError> {
        if self.source.channel != "stable" && self.source.channel != "beta" {
            return Err(DataError::Invalid(format!(
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
            return Err(DataError::Invalid(
                "catalog must contain RNG profiles, characters, cards, and monsters".into(),
            ));
        }
        for (id, profile) in &self.rng_profiles {
            if profile.algorithm != "xoshiro256_star_star_v1"
                || profile.stream_derivation != "numeric_seed_domain_v1"
            {
                return Err(DataError::Invalid(format!(
                    "RNG profile {id} uses an unsupported adapter"
                )));
            }
        }
        for (id, power) in &self.powers {
            if !matches!(power.stack_behavior.as_str(), "amount" | "duration") {
                return Err(DataError::Invalid(format!(
                    "power {id} has unsupported stack behavior {}",
                    power.stack_behavior
                )));
            }
        }
        for (id, card) in &self.cards {
            if let Some(power) = &card.power
                && !self.powers.contains_key(&power.id)
            {
                return Err(DataError::Invalid(format!(
                    "card {id} references unknown power {}",
                    power.id
                )));
            }
            for effect in &card.effects {
                match effect {
                    CardEffectDefinition::Damage { amount }
                    | CardEffectDefinition::Block { amount }
                    | CardEffectDefinition::Draw { amount }
                    | CardEffectDefinition::Energy { amount } => {
                        if amount.base < 0 || amount.upgraded < 0 {
                            return Err(DataError::Invalid(format!(
                                "card {id} has a negative effect amount"
                            )));
                        }
                    }
                    CardEffectDefinition::ApplyPower { id: power_id, .. }
                        if !self.powers.contains_key(power_id) =>
                    {
                        return Err(DataError::Invalid(format!(
                            "card {id} effect references unknown power {power_id}"
                        )));
                    }
                    CardEffectDefinition::ApplyPower { .. }
                    | CardEffectDefinition::Discard { .. } => {}
                }
            }
        }
        for (id, monster) in &self.monsters {
            if monster.hp.min <= 0
                || monster.hp.min > monster.hp.max
                || monster.ascension_hp.min <= 0
                || monster.ascension_hp.min > monster.ascension_hp.max
            {
                return Err(DataError::Invalid(format!(
                    "monster {id} has invalid HP ranges"
                )));
            }
            if !monster.moves.contains_key(&monster.opening_move) {
                return Err(DataError::Invalid(format!(
                    "monster {id} has unknown opening move {}",
                    monster.opening_move
                )));
            }
            for (move_id, movement) in &monster.moves {
                if !monster.moves.contains_key(&movement.next_move) {
                    return Err(DataError::Invalid(format!(
                        "monster {id} move {move_id} references unknown next move {}",
                        movement.next_move
                    )));
                }
                if let Some(power) = &movement.power
                    && !self.powers.contains_key(&power.id)
                {
                    return Err(DataError::Invalid(format!(
                        "monster {id} move {move_id} references unknown power {}",
                        power.id
                    )));
                }
                if let Some(power) = &movement.power
                    && !matches!(power.target.as_str(), "self" | "player")
                {
                    return Err(DataError::Invalid(format!(
                        "monster {id} move {move_id} has unsupported target {}",
                        power.target
                    )));
                }
            }
        }
        self.ascensions.validate()?;
        for (id, encounter) in &self.encounters {
            if encounter.enemies.is_empty() {
                return Err(DataError::Invalid(format!(
                    "encounter {id} must contain at least one enemy"
                )));
            }
            for enemy in &encounter.enemies {
                if !self.monsters.contains_key(enemy) {
                    return Err(DataError::Invalid(format!(
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
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub max_hp: Option<i32>,
    #[serde(default)]
    pub starter_deck: Vec<StarterDeckEntry>,
    #[serde(default)]
    pub starter_relics: Vec<String>,
    #[serde(default)]
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StarterDeckEntry {
    pub id: String,
    pub quantity: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardDefinition {
    pub cost: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub damage: Option<UpgradableValue>,
    #[serde(default)]
    pub block: Option<UpgradableValue>,
    #[serde(default)]
    pub power: Option<CardPowerDefinition>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub effects: Vec<CardEffectDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CardEffectDefinition {
    Damage { amount: UpgradableValue },
    Block { amount: UpgradableValue },
    Draw { amount: UpgradableValue },
    Energy { amount: UpgradableValue },
    ApplyPower { id: String, amount: UpgradableValue },
    Discard { amount: usize },
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
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub asset: Option<String>,
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
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub asset: Option<String>,
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
    fn validate(&self) -> Result<(), DataError> {
        if self.monster_hp_level > self.max_supported_level
            || self.tough_enemies_level > self.max_supported_level
            || self.deadly_enemies_level > self.max_supported_level
        {
            return Err(DataError::Invalid(
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
    fn validate(&self, name: &str) -> Result<(), DataError> {
        if self.numerator < 0 || self.denominator <= 0 {
            return Err(DataError::Invalid(format!(
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
