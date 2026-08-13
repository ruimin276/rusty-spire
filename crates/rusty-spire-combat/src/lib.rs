//! Deterministic combat initialization and state transitions.

mod engine;
mod setup;

pub use engine::{
    CombatEngine, CombatError, EnemyIntent, EnemyPowerIntent, Simulator, SimulatorError,
};
pub use setup::{
    CharacterSetup, CombatSetupV1, DeckEntry, DeferredModelSetup, EncounterSetup, EnemySetup,
    InitializedCombat, LegacyPolicyKind, RelicSetup, SetupRng, initialize,
};
