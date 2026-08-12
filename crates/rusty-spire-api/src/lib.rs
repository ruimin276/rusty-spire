//! Versioned application contracts shared by native and WebAssembly callers.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use rusty_spire_combat::{
    CharacterSetup, CombatSetupV1, DeckEntry, DeferredModelSetup, EncounterSetup, EnemySetup,
    LegacyPolicyKind as PolicyKind, RelicSetup, SetupRng,
};
pub use rusty_spire_data::{CombatCatalog, DataPackage};
pub use rusty_spire_simulator::{CompareResult, SolveLimits, SolveResult, TraceStep};

use rusty_spire_combat::{CombatError, InitializedCombat, initialize};
use rusty_spire_core::{Action, CombatState};
use rusty_spire_data::{CardDefinition, DataError, MonsterDefinition};
use rusty_spire_heuristics::RemainingEnemyHp;
use rusty_spire_simulator::{
    MinimizeHpLoss, SearchMode, ZeroHeuristic, compare, solve, solve_with,
};

pub const API_SCHEMA_VERSION: u32 = 1;
pub const EMBEDDED_PACKAGE: &[u8] =
    include_bytes!("../../../packages/spire-codex-stable-v0.107.1.json");

/// Resolve the v0.3 package hash corresponding to exact legacy catalog bytes.
pub fn legacy_catalog_sha256(input: &[u8]) -> String {
    let legacy = CombatCatalog::from_json(input).expect("legacy catalog must be valid");
    if legacy.sha256 == "7a27dc78a49f6523b64dcc140117f8c21690d1fde6240208de488ee0e88e088c" {
        AppService::embedded()
            .expect("embedded package must be valid")
            .package
            .sha256
    } else {
        legacy.sha256
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageIdentityV1 {
    pub package_id: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSetupV2 {
    pub schema_version: u32,
    pub package: PackageIdentityV1,
    pub ascension_level: u8,
    pub rng: SetupRng,
    pub character: CharacterSetup,
    pub deck: Vec<DeckEntry>,
    pub relics: Vec<RelicSetup>,
    pub potions: Vec<DeferredModelSetup>,
    pub encounter: EncounterSetup,
}

impl CombatSetupV2 {
    fn as_legacy(&self, package: &DataPackage) -> Result<CombatSetupV1, ApiErrorV1> {
        if self.schema_version != 2 {
            return Err(ApiErrorV1::new(
                ApiErrorCode::InvalidRequest,
                format!(
                    "CombatSetupV2 schema_version must be 2, got {}",
                    self.schema_version
                ),
            ));
        }
        if self.package.package_id != package.package_id || self.package.sha256 != package.sha256 {
            return Err(ApiErrorV1::new(
                ApiErrorCode::PackageMismatch,
                format!(
                    "expected package {} at {}, got {} at {}",
                    self.package.package_id,
                    self.package.sha256,
                    package.package_id,
                    package.sha256
                ),
            ));
        }
        Ok(CombatSetupV1 {
            schema_version: 1,
            catalog_sha256: package.sha256.clone(),
            ascension_level: self.ascension_level,
            rng: self.rng.clone(),
            character: self.character.clone(),
            deck: self.deck.clone(),
            relics: self.relics.clone(),
            potions: self.potions.clone(),
            encounter: self.encounter.clone(),
            policy: PolicyKind::MinimizeHpLoss,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicKindV1 {
    #[default]
    Zero,
    RemainingEnemyHp,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolveRequestV1 {
    pub schema_version: u32,
    pub setup: CombatSetupV2,
    #[serde(default)]
    pub policy: PolicyKind,
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub heuristic: HeuristicKindV1,
    #[serde(default)]
    pub limits: SolveLimits,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompareRequestV1 {
    pub schema_version: u32,
    pub baseline: CombatSetupV2,
    pub candidate: CombatSetupV2,
    #[serde(default)]
    pub policy: PolicyKind,
    #[serde(default)]
    pub limits: SolveLimits,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CombatActionV1 {
    Card {
        id: String,
        card_id: String,
        instance_id: String,
        target_id: Option<String>,
        cost: i32,
    },
    EndTurn {
        id: String,
    },
    Choose {
        id: String,
        choice_id: String,
        selection: Vec<String>,
    },
}

impl TryFrom<&Action> for CombatActionV1 {
    type Error = ApiErrorV1;

    fn try_from(action: &Action) -> Result<Self, Self::Error> {
        match action.action_type.as_str() {
            "card" | "play_card" => Ok(Self::Card {
                id: action.id.clone(),
                card_id: action.card_id.clone().ok_or_else(|| {
                    ApiErrorV1::new(ApiErrorCode::Internal, "card action has no card_id")
                })?,
                instance_id: action.combat_card_index.clone().ok_or_else(|| {
                    ApiErrorV1::new(ApiErrorCode::Internal, "card action has no instance id")
                })?,
                target_id: action.target_combat_id.clone(),
                cost: action.cost.unwrap_or(0),
            }),
            "end_turn" => Ok(Self::EndTurn {
                id: action.id.clone(),
            }),
            "choice" => Ok(Self::Choose {
                id: action.id.clone(),
                choice_id: action.choice_id.clone().ok_or_else(|| {
                    ApiErrorV1::new(ApiErrorCode::Internal, "choice action has no choice_id")
                })?,
                selection: action.selection.clone(),
            }),
            other => Err(ApiErrorV1::new(
                ApiErrorCode::Unsupported,
                format!("unsupported action type {other}"),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSnapshotV3 {
    pub schema_version: u32,
    pub state: CombatState,
}

impl CombatSnapshotV3 {
    pub fn state_id(&self) -> Result<String, ApiErrorV1> {
        if self.schema_version != 3 {
            return Err(ApiErrorV1::new(
                ApiErrorCode::InvalidRequest,
                "CombatSnapshotV3 schema_version must be 3",
            ));
        }
        rusty_spire_core::state_id(self)
            .map_err(|error| ApiErrorV1::new(ApiErrorCode::Internal, error.to_string()))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SolveResponseV1 {
    pub schema_version: u32,
    pub result: SolveResult,
    pub opening_hand: Vec<String>,
    pub actions: Vec<CombatActionV1>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompareResponseV1 {
    pub schema_version: u32,
    pub result: CompareResult,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentManifestV1 {
    pub schema_version: u32,
    pub package: PackageIdentityV1,
    pub game_version: String,
    pub characters: Vec<ContentCharacterV1>,
    pub cards: Vec<ContentCardV1>,
    pub enemies: Vec<ContentEnemyV1>,
    pub relics: Vec<ContentRelicV1>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentCharacterV1 {
    pub id: String,
    pub name: String,
    pub max_hp: i32,
    pub max_energy: i32,
    pub starter_deck: Vec<DeckEntry>,
    pub starter_relics: Vec<String>,
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentCardV1 {
    pub id: String,
    pub name: String,
    pub character: Option<String>,
    pub card_type: Option<String>,
    pub cost: i32,
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentEnemyV1 {
    pub id: String,
    pub name: String,
    pub hp: [i32; 2],
    pub ascension_hp: [i32; 2],
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentRelicV1 {
    pub id: String,
    pub name: String,
    pub asset: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidJson,
    InvalidRequest,
    PackageMismatch,
    UnknownId,
    Unsupported,
    InvalidAction,
    Internal,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiErrorV1 {
    pub schema_version: u32,
    pub code: ApiErrorCode,
    pub message: String,
}

impl ApiErrorV1 {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            schema_version: API_SCHEMA_VERSION,
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ApiErrorV1 {}

impl From<CombatError> for ApiErrorV1 {
    fn from(error: CombatError) -> Self {
        let code = match error {
            CombatError::CatalogMismatch { .. } => ApiErrorCode::PackageMismatch,
            CombatError::UnknownId(_) => ApiErrorCode::UnknownId,
            CombatError::UnsupportedMechanic(_) => ApiErrorCode::Unsupported,
            CombatError::InvalidAction(_) => ApiErrorCode::InvalidAction,
            CombatError::Json(_) => ApiErrorCode::InvalidJson,
            CombatError::Catalog(_)
            | CombatError::InvalidSetup(_)
            | CombatError::InvalidSnapshot(_) => ApiErrorCode::InvalidRequest,
        };
        Self::new(code, error.to_string())
    }
}

impl From<DataError> for ApiErrorV1 {
    fn from(error: DataError) -> Self {
        let code = match error {
            DataError::Json(_) => ApiErrorCode::InvalidJson,
            DataError::Invalid(_) => ApiErrorCode::InvalidRequest,
        };
        Self::new(code, error.to_string())
    }
}

pub struct AppService {
    package: DataPackage,
}

impl AppService {
    pub fn embedded() -> Result<Self, ApiErrorV1> {
        Self::from_package_json(EMBEDDED_PACKAGE)
    }

    pub fn from_package_json(input: &[u8]) -> Result<Self, ApiErrorV1> {
        Ok(Self {
            package: DataPackage::from_json(input)?,
        })
    }

    pub fn from_package(package: DataPackage) -> Self {
        Self { package }
    }

    pub fn package(&self) -> &DataPackage {
        &self.package
    }

    pub fn content_manifest(&self) -> ContentManifestV1 {
        let data = &self.package.data;
        ContentManifestV1 {
            schema_version: API_SCHEMA_VERSION,
            package: PackageIdentityV1 {
                package_id: self.package.package_id.clone(),
                sha256: self.package.sha256.clone(),
            },
            game_version: data.source.game_version.clone(),
            characters: data
                .characters
                .iter()
                .map(|(id, character)| ContentCharacterV1 {
                    id: id.clone(),
                    name: display_name(&character.name, id),
                    max_hp: character.max_hp.unwrap_or(1),
                    max_energy: character.max_energy,
                    starter_deck: character
                        .starter_deck
                        .iter()
                        .map(|entry| DeckEntry {
                            id: entry.id.clone(),
                            quantity: entry.quantity,
                            upgrade_level: 0,
                        })
                        .collect(),
                    starter_relics: character.starter_relics.clone(),
                    asset: character.asset.clone(),
                })
                .collect(),
            cards: data
                .cards
                .iter()
                .map(|(id, card)| content_card(id, card))
                .collect(),
            enemies: data
                .monsters
                .iter()
                .map(|(id, enemy)| content_enemy(id, enemy))
                .collect(),
            relics: data
                .relics
                .iter()
                .map(|(id, relic)| ContentRelicV1 {
                    id: id.clone(),
                    name: display_name(&relic.name, id),
                    asset: relic.asset.clone(),
                })
                .collect(),
        }
    }

    pub fn validate_v2(&self, setup: &CombatSetupV2) -> Result<InitializedCombat, ApiErrorV1> {
        let legacy = setup.as_legacy(&self.package)?;
        Ok(initialize(&self.package, &legacy, false)?)
    }

    pub fn solve_v1(&self, request: &SolveRequestV1) -> Result<SolveResponseV1, ApiErrorV1> {
        if request.schema_version != API_SCHEMA_VERSION {
            return Err(ApiErrorV1::new(
                ApiErrorCode::InvalidRequest,
                "SolveRequestV1 schema_version must be 1",
            ));
        }
        request.policy.validate()?;
        let initialized = self.validate_v2(&request.setup)?;
        let opening_hand = initialized
            .state
            .hand
            .iter()
            .map(|card| card.model_id.clone())
            .collect();
        let result = match request.heuristic {
            HeuristicKindV1::Zero => solve_with(
                &self.package,
                &initialized,
                request.limits,
                &MinimizeHpLoss,
                &ZeroHeuristic,
                request.mode,
            )?,
            HeuristicKindV1::RemainingEnemyHp => solve_with(
                &self.package,
                &initialized,
                request.limits,
                &MinimizeHpLoss,
                &RemainingEnemyHp,
                request.mode,
            )?,
        };
        let actions = result
            .actions
            .iter()
            .map(|step| CombatActionV1::try_from(&step.action))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SolveResponseV1 {
            schema_version: API_SCHEMA_VERSION,
            result,
            opening_hand,
            actions,
        })
    }

    pub fn compare_v1(&self, request: &CompareRequestV1) -> Result<CompareResponseV1, ApiErrorV1> {
        if request.schema_version != API_SCHEMA_VERSION {
            return Err(ApiErrorV1::new(
                ApiErrorCode::InvalidRequest,
                "CompareRequestV1 schema_version must be 1",
            ));
        }
        request.policy.validate()?;
        let baseline = self.validate_v2(&request.baseline)?;
        let candidate = self.validate_v2(&request.candidate)?;
        Ok(CompareResponseV1 {
            schema_version: API_SCHEMA_VERSION,
            result: compare(&self.package, &baseline, &candidate, request.limits)?,
        })
    }

    pub fn validate_legacy(
        &self,
        setup: &CombatSetupV1,
        allow_debug_rng_overrides: bool,
    ) -> Result<InitializedCombat, ApiErrorV1> {
        let mut setup = setup.clone();
        if setup.catalog_sha256
            == "7a27dc78a49f6523b64dcc140117f8c21690d1fde6240208de488ee0e88e088c"
            && self.package.package_id == "spire-codex-stable-v0.107.1"
        {
            setup.catalog_sha256 = self.package.sha256.clone();
        }
        Ok(initialize(
            &self.package,
            &setup,
            allow_debug_rng_overrides,
        )?)
    }

    pub fn solve_legacy(
        &self,
        setup: &CombatSetupV1,
        limits: SolveLimits,
        allow_debug_rng_overrides: bool,
    ) -> Result<SolveResult, ApiErrorV1> {
        let initialized = self.validate_legacy(setup, allow_debug_rng_overrides)?;
        Ok(solve(&self.package, &initialized, limits)?)
    }

    pub fn compare_legacy(
        &self,
        baseline: &CombatSetupV1,
        candidate: &CombatSetupV1,
        limits: SolveLimits,
        allow_debug_rng_overrides: bool,
    ) -> Result<CompareResult, ApiErrorV1> {
        let baseline = self.validate_legacy(baseline, allow_debug_rng_overrides)?;
        let candidate = self.validate_legacy(candidate, allow_debug_rng_overrides)?;
        Ok(compare(&self.package, &baseline, &candidate, limits)?)
    }

    pub fn call_json(&self, input: &[u8]) -> Vec<u8> {
        let response = serde_json::from_slice::<ApiOperationV1>(input)
            .map_err(|error| ApiErrorV1::new(ApiErrorCode::InvalidJson, error.to_string()))
            .and_then(|operation| self.call(operation));
        serde_json::to_vec(&match response {
            Ok(value) => json!({"ok": true, "value": value}),
            Err(error) => json!({"ok": false, "error": error}),
        })
        .expect("API envelope is serializable")
    }

    fn call(&self, operation: ApiOperationV1) -> Result<Value, ApiErrorV1> {
        match operation {
            ApiOperationV1::ContentInfo => {
                Ok(serde_json::to_value(self.content_manifest()).expect("manifest serializes"))
            }
            ApiOperationV1::Validate { setup } => {
                let initialized = self.validate_v2(&setup)?;
                Ok(json!({"valid": true, "setup_hash": initialized.setup_hash}))
            }
            ApiOperationV1::Solve { request } => {
                Ok(serde_json::to_value(self.solve_v1(&request)?).expect("response serializes"))
            }
            ApiOperationV1::Compare { request } => {
                Ok(serde_json::to_value(self.compare_v1(&request)?).expect("response serializes"))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum ApiOperationV1 {
    ContentInfo,
    Validate { setup: Box<CombatSetupV2> },
    Solve { request: Box<SolveRequestV1> },
    Compare { request: Box<CompareRequestV1> },
}

fn display_name(value: &str, id: &str) -> String {
    if !value.is_empty() {
        return value.to_owned();
    }
    id.split_once('.')
        .map_or(id, |(_, name)| name)
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn content_card(id: &str, card: &CardDefinition) -> ContentCardV1 {
    ContentCardV1 {
        id: id.to_owned(),
        name: display_name(&card.name, id),
        character: card.character.clone(),
        card_type: card.card_type.clone(),
        cost: card.cost,
        asset: card.asset.clone(),
    }
}

fn content_enemy(id: &str, enemy: &MonsterDefinition) -> ContentEnemyV1 {
    ContentEnemyV1 {
        id: id.to_owned(),
        name: display_name(&enemy.name, id),
        hp: [enemy.hp.min, enemy.hp.max],
        ascension_hp: [enemy.ascension_hp.min, enemy.ascension_hp.max],
        asset: enemy.asset.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_manifest_drives_supported_content() {
        let service = AppService::embedded().unwrap();
        let manifest = service.content_manifest();
        assert_eq!(manifest.schema_version, 1);
        assert!(
            manifest
                .cards
                .iter()
                .any(|card| card.id == "CARD.IRON_WAVE")
        );
        assert!(
            manifest
                .characters
                .iter()
                .all(|character| !character.starter_deck.is_empty())
        );
    }

    #[test]
    fn package_mismatch_has_stable_error_code() {
        let service = AppService::embedded().unwrap();
        let setup = CombatSetupV2 {
            schema_version: 2,
            package: PackageIdentityV1 {
                package_id: "wrong".into(),
                sha256: "wrong".into(),
            },
            ascension_level: 0,
            rng: SetupRng {
                run_seed: "1".into(),
                profile: "isolated_combat_xoshiro_v1".into(),
                stream_overrides: Default::default(),
            },
            character: CharacterSetup {
                id: "CHARACTER.SILENT".into(),
                current_hp: 70,
                max_hp: 70,
                max_energy: None,
            },
            deck: vec![DeckEntry {
                id: "CARD.STRIKE_SILENT".into(),
                quantity: 1,
                upgrade_level: 0,
            }],
            relics: vec![],
            potions: vec![],
            encounter: EncounterSetup::Catalog {
                id: "ENCOUNTER.NIBBITS_WEAK".into(),
            },
        };
        let error = service.validate_v2(&setup).unwrap_err();
        assert!(matches!(error.code, ApiErrorCode::PackageMismatch));
    }

    #[test]
    fn v2_proof_slice_request_solves_exactly() {
        let service = AppService::embedded().unwrap();
        let setup: CombatSetupV2 = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v2/ironclad_proof_slice_seed_1.json"
        ))
        .unwrap();
        let response = service
            .solve_v1(&SolveRequestV1 {
                schema_version: 1,
                setup,
                policy: PolicyKind::MinimizeHpLoss,
                mode: SearchMode::Exact,
                heuristic: HeuristicKindV1::RemainingEnemyHp,
                limits: SolveLimits::default(),
            })
            .unwrap();
        assert!(response.result.won);
        assert!(response.result.optimality_proven);
    }

    #[test]
    fn snapshot_v3_state_id_is_canonical_and_schema_guarded() {
        let service = AppService::embedded().unwrap();
        let setup: CombatSetupV2 = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v2/silent_proof_slice_seed_1.json"
        ))
        .unwrap();
        let initialized = service.validate_v2(&setup).unwrap();
        let snapshot = CombatSnapshotV3 {
            schema_version: 3,
            state: initialized.state,
        };
        assert_eq!(snapshot.state_id().unwrap(), snapshot.state_id().unwrap());
        let invalid = CombatSnapshotV3 {
            schema_version: 2,
            state: snapshot.state,
        };
        assert!(invalid.state_id().is_err());
    }
}
