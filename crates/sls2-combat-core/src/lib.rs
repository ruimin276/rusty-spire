mod rng;
mod search;
mod simulator;
mod state;

pub use search::{compare_scenarios_json, solve_scenario_json};
pub use simulator::{Simulator, SimulatorError};
pub use state::{Action, CombatState, Scenario, validate_scenario_json};

pub fn step_scenario_json(
    scenario_json: &str,
    action_json: &str,
) -> Result<String, SimulatorError> {
    let scenario: Scenario = serde_json::from_str(scenario_json)?;
    Simulator::validate_scenario(&scenario)?;
    let action: Action = serde_json::from_str(action_json)?;
    let state = Simulator::step(&scenario.initial_state, &action)?;
    let state_hash = Simulator::state_hash(&state)?;
    Ok(serde_json::json!({"state": state, "state_hash": state_hash}).to_string())
}

pub fn prepare_scenario_json(scenario_json: &str) -> Result<String, SimulatorError> {
    let mut scenario: Scenario = serde_json::from_str(scenario_json)?;
    Simulator::validate_scenario(&scenario)?;
    scenario.initial_state = Simulator::prepare_combat_start(&scenario.initial_state)?;
    Ok(serde_json::to_string(&scenario)?)
}
