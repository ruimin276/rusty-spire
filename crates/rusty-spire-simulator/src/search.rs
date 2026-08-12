use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use rusty_spire_combat::{InitializedCombat, Simulator, SimulatorError};
use rusty_spire_core::{Action, CombatState};
use rusty_spire_data::CombatCatalog;

use crate::clock::SearchTimer;
use crate::objective::{CombatObjective, MinimizeHpLoss, ObjectiveKind};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolveLimits {
    #[serde(default = "default_max_states")]
    pub max_states: usize,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: f64,
}

const fn default_max_states() -> usize {
    100_000
}
const fn default_max_turns() -> u32 {
    50
}
const fn default_timeout() -> f64 {
    60.0
}

impl Default for SolveLimits {
    fn default() -> Self {
        Self {
            max_states: default_max_states(),
            max_turns: default_max_turns(),
            timeout_seconds: default_timeout(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TraceStep {
    pub action: Action,
    pub state_hash: String,
    pub hp_loss: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolveResult {
    pub catalog_sha256: String,
    pub catalog_game_version: String,
    pub setup_hash: String,
    pub policy: ObjectiveKind,
    pub won: bool,
    pub complete: bool,
    pub optimality_proven: bool,
    pub hp_loss: Option<i32>,
    pub final_hp: Option<i32>,
    pub actions: Vec<TraceStep>,
    pub action_ids: Vec<String>,
    pub explored_states: usize,
    pub cache_hits: usize,
    pub runtime_seconds: f64,
    pub termination_reason: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    #[default]
    Exact,
    Approximate,
}

pub trait Heuristic {
    fn estimate(&self, state: &CombatState) -> i32;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ZeroHeuristic;

impl Heuristic for ZeroHeuristic {
    fn estimate(&self, _state: &CombatState) -> i32 {
        0
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CompareResult {
    pub baseline: SolveResult,
    pub candidate: SolveResult,
    pub hp_loss_delta: Option<i32>,
    pub better: String,
}

#[derive(Clone)]
struct Node {
    state: CombatState,
    hash: String,
    hp_loss: i32,
    parent: Option<usize>,
    action: Option<Action>,
    path_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Priority {
    hp_loss: i32,
    heuristic: i32,
    powers: i32,
    enemy_hp: i32,
    block: i32,
    depth: usize,
    path_key: String,
    node: usize,
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .hp_loss
            .cmp(&self.hp_loss)
            .then_with(|| other.heuristic.cmp(&self.heuristic))
            .then_with(|| self.powers.cmp(&other.powers))
            .then_with(|| other.enemy_hp.cmp(&self.enemy_hp))
            .then_with(|| self.block.cmp(&other.block))
            .then_with(|| other.depth.cmp(&self.depth))
            .then_with(|| other.path_key.cmp(&self.path_key))
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn compare(
    catalog: &CombatCatalog,
    baseline: &InitializedCombat,
    candidate: &InitializedCombat,
    limits: SolveLimits,
) -> Result<CompareResult, SimulatorError> {
    let baseline_result = solve(catalog, baseline, limits)?;
    let candidate_result = solve(catalog, candidate, limits)?;
    let delta = match (baseline_result.hp_loss, candidate_result.hp_loss) {
        (Some(a), Some(b)) if baseline_result.won && candidate_result.won => Some(b - a),
        _ => None,
    };
    let better = if baseline_result.won && !candidate_result.won {
        "baseline"
    } else if candidate_result.won && !baseline_result.won {
        "candidate"
    } else if !baseline_result.won && !candidate_result.won {
        "neither"
    } else {
        match delta {
            Some(value) if value < 0 => "candidate",
            Some(value) if value > 0 => "baseline",
            _ => "tie",
        }
    };
    Ok(CompareResult {
        baseline: baseline_result,
        candidate: candidate_result,
        hp_loss_delta: delta,
        better: better.into(),
    })
}

pub fn solve(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
) -> Result<SolveResult, SimulatorError> {
    solve_with(
        catalog,
        combat,
        limits,
        &MinimizeHpLoss,
        &ZeroHeuristic,
        SearchMode::Exact,
    )
}

pub fn solve_with(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
    objective: &dyn CombatObjective,
    heuristic: &dyn Heuristic,
    mode: SearchMode,
) -> Result<SolveResult, SimulatorError> {
    let mut result = solve_internal(catalog, combat, limits, objective, heuristic, false)?;
    if mode == SearchMode::Approximate {
        result.optimality_proven = false;
    }
    Ok(result)
}

fn solve_internal(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
    objective: &dyn CombatObjective,
    heuristic: &dyn Heuristic,
    reverse_action_enumeration: bool,
) -> Result<SolveResult, SimulatorError> {
    combat.policy.validate()?;
    debug_assert_eq!(objective.kind(), combat.policy);
    let simulator = Simulator::new(catalog);
    let started = SearchTimer::start();
    let initial = combat.state.clone();
    let starting_hp = initial.player.hp;
    let initial_hash = simulator.state_hash(&initial)?;
    let mut nodes = vec![Node {
        state: initial,
        hash: initial_hash.clone(),
        hp_loss: 0,
        parent: None,
        action: None,
        path_key: String::new(),
    }];
    let mut frontier = BinaryHeap::new();
    frontier.push(priority(&nodes[0], 0, 0, heuristic));
    let mut best_loss = HashMap::from([(initial_hash, 0)]);
    let mut explored = 0;
    let mut cache_hits = 0;
    let mut turn_limit_hit = false;

    while let Some(item) = frontier.pop() {
        let elapsed = started.elapsed_seconds();
        if elapsed > limits.timeout_seconds {
            return Ok(incomplete(
                catalog, combat, explored, cache_hits, elapsed, "timeout",
            ));
        }
        let node = nodes[item.node].clone();
        if best_loss.get(&node.hash).copied() != Some(node.hp_loss) {
            cache_hits += 1;
            continue;
        }
        if node.state.combat.won {
            let actions = trace(&nodes, item.node)?;
            return Ok(SolveResult {
                catalog_sha256: catalog.sha256.clone(),
                catalog_game_version: catalog.data.source.game_version.clone(),
                setup_hash: combat.setup_hash.clone(),
                policy: combat.policy,
                won: true,
                complete: true,
                optimality_proven: true,
                hp_loss: Some(node.hp_loss),
                final_hp: Some(node.state.player.hp),
                action_ids: actions.iter().map(|step| step.action.id.clone()).collect(),
                actions,
                explored_states: explored,
                cache_hits,
                runtime_seconds: elapsed,
                termination_reason: "optimal_win".into(),
            });
        }
        if node.state.combat.lost {
            explored += 1;
            continue;
        }
        if explored >= limits.max_states {
            return Ok(incomplete(
                catalog,
                combat,
                explored,
                cache_hits,
                elapsed,
                "max_states",
            ));
        }
        if node.state.combat.turn > limits.max_turns {
            turn_limit_hit = true;
            explored += 1;
            continue;
        }
        explored += 1;
        let depth = depth(&nodes, item.node);
        let mut actions = representative_actions(&simulator, &node.state)?;
        if reverse_action_enumeration {
            actions.reverse();
        }
        for action in actions {
            let next_state = simulator.step(&node.state, &action)?;
            let hash = simulator.state_hash(&next_state)?;
            let loss = node
                .hp_loss
                .max(objective.path_cost(starting_hp, next_state.player.hp));
            if best_loss.get(&hash).is_some_and(|best| loss >= *best) {
                cache_hits += 1;
                continue;
            }
            best_loss.insert(hash.clone(), loss);
            let child = Node {
                state: next_state,
                hash,
                hp_loss: loss,
                parent: Some(item.node),
                path_key: format!("{}\0{}", node.path_key, action.id),
                action: Some(action),
            };
            let child_index = nodes.len();
            nodes.push(child);
            frontier.push(priority(
                &nodes[child_index],
                depth + 1,
                child_index,
                heuristic,
            ));
        }
    }
    if turn_limit_hit {
        return Ok(incomplete(
            catalog,
            combat,
            explored,
            cache_hits,
            started.elapsed_seconds(),
            "max_turns",
        ));
    }
    Ok(SolveResult {
        catalog_sha256: catalog.sha256.clone(),
        catalog_game_version: catalog.data.source.game_version.clone(),
        setup_hash: combat.setup_hash.clone(),
        policy: combat.policy,
        won: false,
        complete: true,
        optimality_proven: true,
        hp_loss: None,
        final_hp: None,
        actions: Vec::new(),
        action_ids: Vec::new(),
        explored_states: explored,
        cache_hits,
        runtime_seconds: started.elapsed_seconds(),
        termination_reason: "no_winning_line".into(),
    })
}

fn representative_actions(
    simulator: &Simulator<'_>,
    state: &CombatState,
) -> Result<Vec<Action>, SimulatorError> {
    // Instance ids are observable and remain in snapshots/traces, but no
    // supported mechanic gives two otherwise-identical starter cards distinct
    // behavior. Search one representative while keeping the public action API
    // and resulting state fully instance-aware.
    let mut seen = HashSet::new();
    Ok(simulator
        .legal_actions(state)?
        .into_iter()
        .filter(|action| seen.insert(action_equivalence_key(state, action)))
        .collect())
}

fn action_equivalence_key(state: &CombatState, action: &Action) -> String {
    let selected_card_id = action
        .combat_card_index
        .as_deref()
        .or_else(|| action.selection.first().map(String::as_str));
    let card = selected_card_id.and_then(|instance_id| {
        state
            .hand
            .iter()
            .find(|card| card.instance_id == instance_id)
    });
    match card {
        Some(card) => format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            action.action_type,
            card.model_id,
            card.upgrade_level,
            card.cost,
            card.cost_for_turn.unwrap_or(card.cost),
            card.retained,
            card.exhausts,
            card.ethereal,
            action.target_combat_id.as_deref().unwrap_or_default(),
        ),
        None => action.id.clone(),
    }
}

fn priority(node: &Node, depth: usize, index: usize, heuristic: &dyn Heuristic) -> Priority {
    Priority {
        hp_loss: node.hp_loss,
        heuristic: heuristic.estimate(&node.state),
        powers: node.state.metrics.powers_played,
        enemy_hp: node.state.enemies.iter().map(|enemy| enemy.hp.max(0)).sum(),
        block: node.state.player.block,
        depth,
        path_key: node.path_key.clone(),
        node: index,
    }
}

fn depth(nodes: &[Node], mut index: usize) -> usize {
    let mut value = 0;
    while let Some(parent) = nodes[index].parent {
        value += 1;
        index = parent;
    }
    value
}

fn trace(nodes: &[Node], mut index: usize) -> Result<Vec<TraceStep>, SimulatorError> {
    let mut steps = Vec::new();
    while let Some(parent) = nodes[index].parent {
        let node = &nodes[index];
        steps.push(TraceStep {
            action: node.action.clone().expect("child nodes have actions"),
            state_hash: node.hash.clone(),
            hp_loss: node.hp_loss,
        });
        index = parent;
    }
    steps.reverse();
    Ok(steps)
}

fn incomplete(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    explored: usize,
    cache_hits: usize,
    runtime: f64,
    reason: &str,
) -> SolveResult {
    SolveResult {
        catalog_sha256: catalog.sha256.clone(),
        catalog_game_version: catalog.data.source.game_version.clone(),
        setup_hash: combat.setup_hash.clone(),
        policy: combat.policy,
        won: false,
        complete: false,
        optimality_proven: false,
        hp_loss: None,
        final_hp: None,
        actions: Vec::new(),
        action_ids: Vec::new(),
        explored_states: explored,
        cache_hits,
        runtime_seconds: runtime,
        termination_reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_spire_combat::{CombatSetupV1, initialize};

    struct EnemyHpHeuristic;

    impl Heuristic for EnemyHpHeuristic {
        fn estimate(&self, state: &CombatState) -> i32 {
            state.enemies.iter().map(|enemy| enemy.hp.max(0)).sum()
        }
    }

    #[test]
    fn optimal_trace_is_independent_of_action_enumeration_order() {
        let catalog =
            CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json"))
                .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json"
        ))
        .unwrap();
        value["catalog_sha256"] = catalog.sha256.clone().into();
        let setup: CombatSetupV1 = serde_json::from_value(value).unwrap();
        let combat = initialize(&catalog, &setup, false).unwrap();
        let objective = MinimizeHpLoss;
        let heuristic = ZeroHeuristic;
        let forward = solve_internal(
            &catalog,
            &combat,
            SolveLimits::default(),
            &objective,
            &heuristic,
            false,
        )
        .unwrap();
        let reversed = solve_internal(
            &catalog,
            &combat,
            SolveLimits::default(),
            &objective,
            &heuristic,
            true,
        )
        .unwrap();
        assert_eq!(forward.hp_loss, reversed.hp_loss);
        assert_eq!(forward.action_ids, reversed.action_ids);
        assert_eq!(
            forward
                .actions
                .iter()
                .map(|step| &step.state_hash)
                .collect::<Vec<_>>(),
            reversed
                .actions
                .iter()
                .map(|step| &step.state_hash)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn approximate_mode_never_claims_optimality() {
        let catalog =
            CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json"))
                .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json"
        ))
        .unwrap();
        value["catalog_sha256"] = catalog.sha256.clone().into();
        let setup: CombatSetupV1 = serde_json::from_value(value).unwrap();
        let combat = initialize(&catalog, &setup, false).unwrap();
        let result = solve_with(
            &catalog,
            &combat,
            SolveLimits::default(),
            &MinimizeHpLoss,
            &ZeroHeuristic,
            SearchMode::Approximate,
        )
        .unwrap();
        assert!(!result.optimality_proven);
    }

    #[test]
    fn exact_results_are_heuristic_independent() {
        let catalog =
            CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json"))
                .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json"
        ))
        .unwrap();
        value["catalog_sha256"] = catalog.sha256.clone().into();
        let setup: CombatSetupV1 = serde_json::from_value(value).unwrap();
        let combat = initialize(&catalog, &setup, false).unwrap();
        let zero = solve_with(
            &catalog,
            &combat,
            SolveLimits::default(),
            &MinimizeHpLoss,
            &ZeroHeuristic,
            SearchMode::Exact,
        )
        .unwrap();
        let ordered = solve_with(
            &catalog,
            &combat,
            SolveLimits::default(),
            &MinimizeHpLoss,
            &EnemyHpHeuristic,
            SearchMode::Exact,
        )
        .unwrap();
        assert_eq!(zero.hp_loss, ordered.hp_loss);
        assert_eq!(zero.action_ids, ordered.action_ids);
        assert!(zero.optimality_proven && ordered.optimality_proven);
    }
}
