//! Platform-independent combat domain primitives.

mod canonical;
mod id;
mod rng;
mod state;

pub use canonical::{canonical_json, combat_state_id, state_id};
pub use id::{ModelId, ModelIdError};
pub use rng::{Xoshiro256StarStar, domain_seed, next_int};
pub use state::{
    Action, CardInstance, CombatState, CombatStatus, Decision, EnemyAiState, EnemyState, Metrics,
    ModelState, PlayerState, PowerState, Provenance, RngBankState, RngStreamState,
};
