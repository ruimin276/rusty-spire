use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::simulator::{Simulator, SimulatorError};
use crate::state::{Action, CombatState, Scenario};

#[derive(Clone, Copy, Debug, Deserialize)]
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
    pub won: bool,
    pub complete: bool,
    pub hp_loss: Option<i32>,
    pub final_hp: Option<i32>,
    pub actions: Vec<TraceStep>,
    pub action_ids: Vec<String>,
    pub explored_states: usize,
    pub cache_hits: usize,
    pub runtime_seconds: f64,
    pub termination_reason: String,
}

#[derive(Clone)]
struct Node {
    state: CombatState,
    hash: String,
    hp_loss: i32,
    parent: Option<usize>,
    action: Option<Action>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Priority {
    hp_loss: i32,
    powers: i32,
    enemy_hp: i32,
    block: i32,
    depth: usize,
    sequence: usize,
    node: usize,
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .hp_loss
            .cmp(&self.hp_loss)
            .then_with(|| self.powers.cmp(&other.powers))
            .then_with(|| other.enemy_hp.cmp(&self.enemy_hp))
            .then_with(|| self.block.cmp(&other.block))
            .then_with(|| other.depth.cmp(&self.depth))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn solve_scenario_json(
    scenario_json: &str,
    limits_json: Option<&str>,
) -> Result<String, SimulatorError> {
    let scenario: Scenario = serde_json::from_str(scenario_json)?;
    Simulator::validate_scenario(&scenario)?;
    let limits = match limits_json {
        Some(value) => serde_json::from_str(value)?,
        None => SolveLimits::default(),
    };
    Ok(serde_json::to_string(&solve(&scenario, limits)?)?)
}

pub fn compare_scenarios_json(
    baseline_json: &str,
    candidate_json: &str,
    limits_json: Option<&str>,
) -> Result<String, SimulatorError> {
    let baseline: Scenario = serde_json::from_str(baseline_json)?;
    let candidate: Scenario = serde_json::from_str(candidate_json)?;
    Simulator::validate_scenario(&baseline)?;
    Simulator::validate_scenario(&candidate)?;
    let limits = match limits_json {
        Some(value) => serde_json::from_str(value)?,
        None => SolveLimits::default(),
    };
    let baseline_result = solve(&baseline, limits)?;
    let candidate_result = solve(&candidate, limits)?;
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
    Ok(serde_json::json!({
        "baseline": baseline_result,
        "candidate": candidate_result,
        "hp_loss_delta": delta,
        "better": better,
    })
    .to_string())
}

fn solve(scenario: &Scenario, limits: SolveLimits) -> Result<SolveResult, SimulatorError> {
    let started = Instant::now();
    let initial = scenario.initial_state.clone();
    let starting_hp = initial.player.hp;
    let initial_hash = Simulator::state_hash(&initial)?;
    let mut nodes = vec![Node {
        state: initial,
        hash: initial_hash.clone(),
        hp_loss: 0,
        parent: None,
        action: None,
    }];
    let mut frontier = BinaryHeap::new();
    frontier.push(priority(&nodes[0], 0, 0, 0));
    let mut best_loss = HashMap::from([(initial_hash, 0)]);
    let mut explored = 0;
    let mut cache_hits = 0;
    let mut sequence = 1;

    while let Some(item) = frontier.pop() {
        let elapsed = started.elapsed().as_secs_f64();
        if elapsed > limits.timeout_seconds {
            return Ok(incomplete(explored, cache_hits, elapsed, "timeout"));
        }
        let node = nodes[item.node].clone();
        if best_loss.get(&node.hash).copied() != Some(node.hp_loss) {
            cache_hits += 1;
            continue;
        }
        if node.state.combat.won {
            let actions = trace(&nodes, item.node)?;
            return Ok(SolveResult {
                won: true,
                complete: true,
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
            return Ok(incomplete(explored, cache_hits, elapsed, "max_states"));
        }
        if node.state.combat.turn > limits.max_turns {
            explored += 1;
            continue;
        }
        explored += 1;
        let depth = depth(&nodes, item.node);
        for action in representative_actions(&node.state)? {
            let next_state = Simulator::step(&node.state, &action)?;
            let hash = Simulator::state_hash(&next_state)?;
            let loss = node.hp_loss.max(starting_hp - next_state.player.hp);
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
                action: Some(action),
            };
            let child_index = nodes.len();
            nodes.push(child);
            frontier.push(priority(
                &nodes[child_index],
                depth + 1,
                sequence,
                child_index,
            ));
            sequence += 1;
        }
    }
    Ok(SolveResult {
        won: false,
        complete: true,
        hp_loss: None,
        final_hp: None,
        actions: Vec::new(),
        action_ids: Vec::new(),
        explored_states: explored,
        cache_hits,
        runtime_seconds: started.elapsed().as_secs_f64(),
        termination_reason: "no_winning_line".into(),
    })
}

fn representative_actions(state: &CombatState) -> Result<Vec<Action>, SimulatorError> {
    // Instance ids are observable and remain in snapshots/traces, but no
    // supported mechanic gives two otherwise-identical starter cards distinct
    // behavior. Search one representative while keeping the public action API
    // and resulting state fully instance-aware.
    let mut seen = HashSet::new();
    Ok(Simulator::legal_actions(state)?
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

fn priority(node: &Node, depth: usize, sequence: usize, index: usize) -> Priority {
    Priority {
        hp_loss: node.hp_loss,
        powers: node.state.metrics.powers_played,
        enemy_hp: node.state.enemies.iter().map(|enemy| enemy.hp.max(0)).sum(),
        block: node.state.player.block,
        depth,
        sequence,
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

fn incomplete(explored: usize, cache_hits: usize, runtime: f64, reason: &str) -> SolveResult {
    SolveResult {
        won: false,
        complete: false,
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
