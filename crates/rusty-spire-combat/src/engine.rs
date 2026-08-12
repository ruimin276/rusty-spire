use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};

use thiserror::Error;

use rusty_spire_core::{
    self as rng, Action, CardInstance, CombatState, Decision, EnemyState, PlayerState, PowerState,
    RngStreamState,
};
use rusty_spire_data::{CardEffectDefinition, CombatCatalog, CombatCatalogV1, RelicCombatEffect};

use crate::setup::{CombatSetupV1, InitializedCombat, initialize};

#[cfg(test)]
const STRIKE: &str = "CARD.STRIKE_IRONCLAD";
#[cfg(test)]
const DEFEND: &str = "CARD.DEFEND_IRONCLAD";
#[cfg(test)]
const BASH: &str = "CARD.BASH";
#[cfg(test)]
const SILENT_STRIKE: &str = "CARD.STRIKE_SILENT";
#[cfg(test)]
const SILENT_DEFEND: &str = "CARD.DEFEND_SILENT";
#[cfg(test)]
const NEUTRALIZE: &str = "CARD.NEUTRALIZE";
#[cfg(test)]
const SURVIVOR: &str = "CARD.SURVIVOR";
const ASCENDERS_BANE: &str = "CARD.ASCENDERS_BANE";
#[cfg(test)]
const IRON_WAVE: &str = "CARD.IRON_WAVE";
#[cfg(test)]
const POMMEL_STRIKE: &str = "CARD.POMMEL_STRIKE";
#[cfg(test)]
const BACKFLIP: &str = "CARD.BACKFLIP";
#[cfg(test)]
const ADRENALINE: &str = "CARD.ADRENALINE";
const NIBBIT: &str = "MONSTER.NIBBIT";
const FUZZY_WURM_CRAWLER: &str = "MONSTER.FUZZY_WURM_CRAWLER";
const SHRINKER_BEETLE: &str = "MONSTER.SHRINKER_BEETLE";
const VULNERABLE: &str = "POWER.VULNERABLE_POWER";
const WEAK: &str = "POWER.WEAK_POWER";
const SHRINK: &str = "POWER.SHRINK_POWER";
const STRENGTH: &str = "POWER.STRENGTH_POWER";

#[derive(Debug, Error)]
pub enum SimulatorError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid combat catalog: {0}")]
    Catalog(String),
    #[error("catalog hash mismatch: expected {expected}, got {actual}")]
    CatalogMismatch { expected: String, actual: String },
    #[error("invalid combat setup: {0}")]
    InvalidSetup(String),
    #[error("unknown catalog id: {0}")]
    UnknownId(String),
    #[error("invalid simulator snapshot: {0}")]
    InvalidSnapshot(String),
    #[error("unsupported mechanic: {0}")]
    UnsupportedMechanic(String),
    #[error("invalid action: {0}")]
    InvalidAction(String),
}

pub type CombatError = SimulatorError;

pub struct Simulator<'a> {
    catalog: &'a CombatCatalog,
}

pub type CombatEngine<'a> = Simulator<'a>;

#[derive(Clone, Debug)]
enum Effect {
    DamageEnemy {
        target: String,
        amount: i32,
    },
    GainPlayerBlock(i32),
    ApplyEnemyPower {
        target: String,
        model_id: String,
        amount: i32,
    },
    DrawCards(usize),
    GainEnergy(i32),
}

impl<'a> Simulator<'a> {
    pub fn new(catalog: &'a CombatCatalog) -> Self {
        Self { catalog }
    }

    pub fn initialize(
        &self,
        setup: &CombatSetupV1,
        allow_debug_rng_overrides: bool,
    ) -> Result<InitializedCombat, SimulatorError> {
        initialize(self.catalog, setup, allow_debug_rng_overrides)
    }

    pub fn validate_state(&self, state: &CombatState) -> Result<(), SimulatorError> {
        if state.snapshot_schema != 2 {
            return Err(SimulatorError::InvalidSnapshot(format!(
                "snapshot_schema must be 2, got {}",
                state.snapshot_schema
            )));
        }
        if state.provenance.modded_gameplay {
            return Err(SimulatorError::UnsupportedMechanic("gameplay mods".into()));
        }
        if state.provenance.content_revision != "base" {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "content revision {}",
                state.provenance.content_revision
            )));
        }
        if !matches!(
            state.player.model_id.as_str(),
            "CHARACTER.IRONCLAD" | "CHARACTER.SILENT"
        ) {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "character {}",
                state.player.model_id
            )));
        }
        if !matches!(state.decision, Decision::Terminal) && state.combat.current_side != "Player" {
            return Err(SimulatorError::InvalidSnapshot(format!(
                "expected a player decision, got current_side={}",
                state.combat.current_side
            )));
        }
        if let Some(ascension_level) = state.combat.ascension_level {
            if ascension_level > self.catalog.data.ascensions.max_supported_level {
                return Err(SimulatorError::InvalidSnapshot(format!(
                    "ascension_level must be between 0 and 10, got {ascension_level}"
                )));
            }
            for enemy in &state.enemies {
                let expected_tough =
                    ascension_level >= self.catalog.data.ascensions.tough_enemies_level;
                let expected_deadly =
                    ascension_level >= self.catalog.data.ascensions.deadly_enemies_level;
                if enemy.ai.tough_enemies != expected_tough
                    || enemy.ai.deadly_enemies != expected_deadly
                {
                    return Err(SimulatorError::InvalidSnapshot(format!(
                        "enemy {} ascension flags disagree with ascension_level {}: tough_enemies={}, deadly_enemies={}",
                        enemy.model_id,
                        ascension_level,
                        enemy.ai.tough_enemies,
                        enemy.ai.deadly_enemies
                    )));
                }
            }
        }
        if state.rng.algorithm != "xoshiro256_star_star_v1" {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "rng algorithm {}",
                state.rng.algorithm
            )));
        }
        if !state.rng.streams.contains_key("shuffle") {
            return Err(SimulatorError::InvalidSnapshot(
                "rng.streams.shuffle is required".into(),
            ));
        }
        if !state.player.potions.is_empty() {
            return Err(SimulatorError::UnsupportedMechanic("potions".into()));
        }
        if state.enemies.len() > 1
            || (!matches!(state.decision, Decision::Terminal) && state.enemies.len() != 1)
        {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "the parity slice requires one supported single enemy, got {} enemies",
                state.enemies.len()
            )));
        }
        for relic in &state.player.relics {
            self.catalog
                .data
                .relics
                .get(&relic.model_id)
                .ok_or_else(|| SimulatorError::UnknownId(relic.model_id.clone()))?;
        }
        let mut instance_ids = HashSet::new();
        for card in state
            .hand
            .iter()
            .chain(&state.draw_pile)
            .chain(&state.discard_pile)
            .chain(&state.exhaust_pile)
            .chain(&state.play_pile)
        {
            let definition = self
                .catalog
                .data
                .cards
                .get(&card.model_id)
                .ok_or_else(|| SimulatorError::UnknownId(card.model_id.clone()))?;
            let supported_handler =
                card.model_id == ASCENDERS_BANE || !definition.effects.is_empty();
            let expected_character = definition.character.as_deref();
            if !supported_handler
                || expected_character.is_some_and(|value| value != state.player.model_id)
            {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "card {}",
                    card.model_id
                )));
            }
            if card.upgrade_level > 1 {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "upgrade level {} on card {}",
                    card.upgrade_level, card.instance_id
                )));
            }
            if card.instance_id.is_empty() || !instance_ids.insert(card.instance_id.as_str()) {
                return Err(SimulatorError::InvalidSnapshot(format!(
                    "card instance id must be non-empty and unique: {:?}",
                    card.instance_id
                )));
            }
        }
        for enemy in &state.enemies {
            if !matches!(
                enemy.model_id.as_str(),
                NIBBIT | FUZZY_WURM_CRAWLER | SHRINKER_BEETLE
            ) {
                return Err(SimulatorError::UnsupportedMechanic(format!(
                    "enemy {}",
                    enemy.model_id
                )));
            }
            if enemy.hp > 0 && !valid_enemy_move(&self.catalog.data, enemy) {
                return Err(SimulatorError::InvalidSnapshot(format!(
                    "unknown {} move {}",
                    enemy.model_id, enemy.ai.current_move
                )));
            }
        }
        validate_decision(state)?;
        validate_powers(&self.catalog.data, &state.player.powers)?;
        for enemy in &state.enemies {
            validate_powers(&self.catalog.data, &enemy.powers)?;
        }
        Ok(())
    }

    pub fn legal_actions(&self, state: &CombatState) -> Result<Vec<Action>, SimulatorError> {
        self.validate_state(state)?;
        if state.combat.won || state.combat.lost || matches!(state.decision, Decision::Terminal) {
            return Ok(Vec::new());
        }
        match &state.decision {
            Decision::PlayerAction => {
                let mut actions = Vec::new();
                for card in &state.hand {
                    let cost = card.effective_cost();
                    if card.model_id == ASCENDERS_BANE || cost < 0 || cost > state.player.energy {
                        continue;
                    }
                    let definition = &self.catalog.data.cards[&card.model_id];
                    let targeted = definition.effects.iter().any(|effect| {
                        matches!(
                            effect,
                            CardEffectDefinition::Damage { .. }
                                | CardEffectDefinition::ApplyPower { .. }
                        )
                    });
                    if !targeted {
                        actions.push(card_action(card, None));
                    } else {
                        for (index, enemy) in state
                            .enemies
                            .iter()
                            .enumerate()
                            .filter(|(_, enemy)| enemy.hp > 0)
                        {
                            actions.push(card_action(card, Some((enemy, index))));
                        }
                    }
                }
                actions.push(Action {
                    id: "end_turn".into(),
                    action_type: "end_turn".into(),
                    card_id: None,
                    combat_card_index: None,
                    target_combat_id: None,
                    cost: None,
                    choice_id: None,
                    selection: Vec::new(),
                });
                Ok(actions)
            }
            Decision::CardSelection {
                choice_id,
                candidates,
                min,
                max,
            } => {
                if min == max && *min == 1 {
                    Ok(candidates
                        .iter()
                        .map(|candidate| Action {
                            id: format!("choose:{choice_id}:{candidate}"),
                            action_type: "choice".into(),
                            card_id: None,
                            combat_card_index: None,
                            target_combat_id: None,
                            cost: None,
                            choice_id: Some(choice_id.clone()),
                            selection: vec![candidate.clone()],
                        })
                        .collect())
                } else {
                    Err(SimulatorError::UnsupportedMechanic(format!(
                        "multi-card selection {choice_id} ({min}..{max})"
                    )))
                }
            }
            Decision::Terminal => Ok(Vec::new()),
        }
    }

    pub fn step(
        &self,
        state: &CombatState,
        action: &Action,
    ) -> Result<CombatState, SimulatorError> {
        self.validate_state(state)?;
        let canonical_action = self
            .legal_actions(state)?
            .into_iter()
            .find(|candidate| candidate.id == action.id)
            .ok_or_else(|| SimulatorError::InvalidAction(action.id.clone()))?;
        let mut next = state.clone();
        match canonical_action.action_type.as_str() {
            "card" | "play_card" => play_card(&self.catalog.data, &mut next, &canonical_action)?,
            "end_turn" => end_turn(&self.catalog.data, &mut next)?,
            "choice" => resolve_card_selection(&mut next, &canonical_action)?,
            other => return Err(SimulatorError::InvalidAction(other.into())),
        }
        update_terminal(&mut next);
        Ok(next)
    }

    pub fn prepare_combat_start(&self, state: &CombatState) -> Result<CombatState, SimulatorError> {
        self.validate_state(state)?;
        if state.combat.turn != 1
            || !state.hand.is_empty()
            || !state.discard_pile.is_empty()
            || !state.exhaust_pile.is_empty()
            || !state.play_pile.is_empty()
        {
            return Err(SimulatorError::InvalidSnapshot(
                "combat preparation requires turn 1 with the full deck in draw_pile".into(),
            ));
        }
        let mut next = state.clone();
        let algorithm = next.rng.algorithm.clone();
        let shuffle = next
            .rng
            .streams
            .get_mut("shuffle")
            .ok_or_else(|| SimulatorError::InvalidSnapshot("missing shuffle RNG".into()))?;
        shuffle_in_place(&mut next.draw_pile, shuffle, &algorithm);
        let additional_draw = next
            .player
            .relics
            .iter()
            .filter_map(|relic| self.catalog.data.relics.get(&relic.model_id))
            .map(|relic| match relic.combat_effect {
                RelicCombatEffect::AdditionalOpeningDraw { amount } => amount,
                RelicCombatEffect::Inert => 0,
            })
            .sum::<usize>();
        let hand_size = 5 + additional_draw;
        draw_to_hand(&mut next, hand_size)?;
        Ok(next)
    }

    pub fn state_hash(&self, state: &CombatState) -> Result<String, SimulatorError> {
        self.validate_state(state)?;
        rusty_spire_core::combat_state_id(state)
            .map_err(|error| SimulatorError::InvalidSnapshot(error.to_string()))
    }

    pub fn state_id(&self, state: &CombatState) -> Result<String, SimulatorError> {
        self.state_hash(state)
    }
}

fn validate_powers(catalog: &CombatCatalogV1, powers: &[PowerState]) -> Result<(), SimulatorError> {
    for power in powers {
        if !catalog.powers.contains_key(&power.model_id) {
            return Err(SimulatorError::UnknownId(power.model_id.clone()));
        }
        if !matches!(
            power.model_id.as_str(),
            VULNERABLE | WEAK | SHRINK | STRENGTH
        ) {
            return Err(SimulatorError::UnsupportedMechanic(format!(
                "power {}",
                power.model_id
            )));
        }
    }
    Ok(())
}

fn valid_enemy_move(catalog: &CombatCatalogV1, enemy: &EnemyState) -> bool {
    catalog
        .monsters
        .get(&enemy.model_id)
        .is_some_and(|monster| monster.moves.contains_key(&enemy.ai.current_move))
}

fn validate_decision(state: &CombatState) -> Result<(), SimulatorError> {
    let Decision::CardSelection {
        choice_id,
        candidates,
        min,
        max,
    } = &state.decision
    else {
        return Ok(());
    };
    if *min != 1 || *max != 1 || !choice_id.starts_with("discard:") {
        return Err(SimulatorError::UnsupportedMechanic(format!(
            "card selection {choice_id} ({min}..{max})"
        )));
    }
    let played_card_id = choice_id.trim_start_matches("discard:");
    if !state
        .play_pile
        .iter()
        .any(|card| card.instance_id == played_card_id)
    {
        return Err(SimulatorError::InvalidSnapshot(format!(
            "selection {choice_id} has no resolving card"
        )));
    }
    let hand_ids = state
        .hand
        .iter()
        .map(|card| card.instance_id.as_str())
        .collect::<HashSet<_>>();
    if candidates.is_empty()
        || candidates.len() != hand_ids.len()
        || candidates
            .iter()
            .any(|candidate| !hand_ids.contains(candidate.as_str()))
    {
        return Err(SimulatorError::InvalidSnapshot(format!(
            "selection {choice_id} candidates do not match the hand"
        )));
    }
    Ok(())
}

fn card_action(card: &CardInstance, target: Option<(&EnemyState, usize)>) -> Action {
    Action {
        id: target.map_or_else(
            || format!("play:{}", card.instance_id),
            |(_, index)| format!("play:{}:enemy_{}", card.instance_id, index),
        ),
        action_type: "card".into(),
        card_id: Some(card.model_id.clone()),
        combat_card_index: Some(card.instance_id.clone()),
        target_combat_id: target.map(|(enemy, _)| enemy.combat_id.clone()),
        cost: Some(card.effective_cost()),
        choice_id: None,
        selection: Vec::new(),
    }
}

fn play_card(
    catalog: &CombatCatalogV1,
    state: &mut CombatState,
    action: &Action,
) -> Result<(), SimulatorError> {
    let instance_id = action.combat_card_index.as_deref().ok_or_else(|| {
        SimulatorError::InvalidAction("card action missing combat_card_index".into())
    })?;
    let hand_index = state
        .hand
        .iter()
        .position(|card| card.instance_id == instance_id)
        .ok_or_else(|| {
            SimulatorError::InvalidAction(format!("card {instance_id} is not in hand"))
        })?;
    let card = state.hand.remove(hand_index);
    let cost = card.effective_cost();
    state.player.energy -= cost;
    state.play_pile.push(card.clone());
    let definition = catalog
        .cards
        .get(&card.model_id)
        .ok_or_else(|| SimulatorError::UnknownId(card.model_id.clone()))?;
    let mut effects = VecDeque::new();
    let target = if definition.effects.iter().any(|effect| {
        matches!(
            effect,
            CardEffectDefinition::Damage { .. } | CardEffectDefinition::ApplyPower { .. }
        )
    }) {
        Some(required_target(action)?.to_owned())
    } else {
        None
    };
    for effect in &definition.effects {
        match effect {
            CardEffectDefinition::Damage { amount } => effects.push_back(Effect::DamageEnemy {
                target: target.clone().expect("targeted effects require a target"),
                amount: amount.at(card.upgrade_level),
            }),
            CardEffectDefinition::Block { amount } => {
                effects.push_back(Effect::GainPlayerBlock(amount.at(card.upgrade_level)))
            }
            CardEffectDefinition::Draw { amount } => effects.push_back(Effect::DrawCards(
                amount.at(card.upgrade_level).try_into().map_err(|_| {
                    SimulatorError::Catalog(format!("{} has invalid draw", card.model_id))
                })?,
            )),
            CardEffectDefinition::Energy { amount } => {
                effects.push_back(Effect::GainEnergy(amount.at(card.upgrade_level)))
            }
            CardEffectDefinition::ApplyPower { id, amount } => {
                effects.push_back(Effect::ApplyEnemyPower {
                    target: target.clone().expect("targeted effects require a target"),
                    model_id: id.clone(),
                    amount: amount.at(card.upgrade_level),
                });
            }
            CardEffectDefinition::Discard { .. } => {}
        }
    }
    resolve_effects(catalog, state, effects)?;
    if state.enemies.iter().all(|enemy| enemy.hp <= 0) {
        state.enemies.retain(|enemy| enemy.hp > 0);
        update_terminal(state);
        return Ok(());
    }
    let discard = definition.effects.iter().find_map(|effect| match effect {
        CardEffectDefinition::Discard { amount } => Some(*amount),
        _ => None,
    });
    if discard == Some(1) && !state.hand.is_empty() {
        state.decision = Decision::CardSelection {
            choice_id: format!("discard:{}", card.instance_id),
            candidates: state
                .hand
                .iter()
                .map(|candidate| candidate.instance_id.clone())
                .collect(),
            min: 1,
            max: 1,
        };
        return Ok(());
    }
    finish_played_card(state, &card.instance_id)
}

fn finish_played_card(state: &mut CombatState, instance_id: &str) -> Result<(), SimulatorError> {
    let play_index = state
        .play_pile
        .iter()
        .position(|candidate| candidate.instance_id == instance_id)
        .expect("played card remains in play pile until resolution");
    let resolved_card = state.play_pile.remove(play_index);
    if resolved_card.exhausts {
        state.exhaust_pile.push(resolved_card);
    } else {
        state.discard_pile.push(resolved_card);
    }
    Ok(())
}

fn resolve_card_selection(state: &mut CombatState, action: &Action) -> Result<(), SimulatorError> {
    let choice_id = action
        .choice_id
        .as_deref()
        .ok_or_else(|| SimulatorError::InvalidAction("choice_id is required".into()))?;
    let played_card_id = choice_id.strip_prefix("discard:").ok_or_else(|| {
        SimulatorError::UnsupportedMechanic(format!("card selection {choice_id}"))
    })?;
    let selected_id = action.selection.first().ok_or_else(|| {
        SimulatorError::InvalidAction("Survivor selection must contain one card".into())
    })?;
    let selected_index = state
        .hand
        .iter()
        .position(|card| card.instance_id == *selected_id)
        .ok_or_else(|| SimulatorError::InvalidAction(format!("unknown selection {selected_id}")))?;
    state.discard_pile.push(state.hand.remove(selected_index));
    finish_played_card(state, played_card_id)?;
    state.decision = Decision::PlayerAction;
    Ok(())
}

fn resolve_effects(
    catalog: &CombatCatalogV1,
    state: &mut CombatState,
    mut effects: VecDeque<Effect>,
) -> Result<(), SimulatorError> {
    while let Some(effect) = effects.pop_front() {
        match effect {
            Effect::DamageEnemy { target, amount } => {
                damage_enemy(catalog, state, &target, amount)?
            }
            Effect::GainPlayerBlock(amount) => state.player.block += amount,
            Effect::DrawCards(amount) => draw_to_hand(state, state.hand.len() + amount)?,
            Effect::GainEnergy(amount) => state.player.energy += amount,
            Effect::ApplyEnemyPower {
                target,
                model_id,
                amount,
            } => {
                // The combat terminates as soon as the final enemy dies. Later
                // effects from the same card are not observable at another
                // decision boundary.
                if state.enemies.iter().all(|enemy| enemy.hp <= 0) {
                    break;
                }
                let enemy = target_enemy_by_id_mut(state, &target)?;
                add_power(&mut enemy.powers, &model_id, amount);
            }
        }
    }
    Ok(())
}

fn damage_enemy(
    catalog: &CombatCatalogV1,
    state: &mut CombatState,
    target: &str,
    base_damage: i32,
) -> Result<(), SimulatorError> {
    let attacker_powers = state.player.powers.clone();
    let enemy = target_enemy_by_id_mut(state, target)?;
    let damage = attack_damage(catalog, base_damage, &attacker_powers, &enemy.powers, true);
    apply_damage(&mut enemy.hp, &mut enemy.block, damage);
    Ok(())
}

fn required_target(action: &Action) -> Result<&str, SimulatorError> {
    action
        .target_combat_id
        .as_deref()
        .ok_or_else(|| SimulatorError::InvalidAction("target is required".into()))
}

fn target_enemy_by_id_mut<'a>(
    state: &'a mut CombatState,
    target: &str,
) -> Result<&'a mut EnemyState, SimulatorError> {
    state
        .enemies
        .iter_mut()
        .find(|enemy| enemy.combat_id == target && enemy.hp > 0)
        .ok_or_else(|| SimulatorError::InvalidAction(format!("unknown target {target}")))
}

fn end_turn(catalog: &CombatCatalogV1, state: &mut CombatState) -> Result<(), SimulatorError> {
    let mut retained = Vec::new();
    for card in state.hand.drain(..) {
        if card.ethereal || card.model_id == ASCENDERS_BANE {
            state.exhaust_pile.push(card);
        } else if card.retained {
            retained.push(card);
        } else {
            state.discard_pile.push(card);
        }
    }
    state.hand = retained;

    for enemy in state.enemies.iter_mut().filter(|enemy| enemy.hp > 0) {
        enemy.block = 0;
        execute_enemy_move(catalog, &mut state.player, enemy)?;
    }
    tick_power(&mut state.player.powers, VULNERABLE);
    tick_power(&mut state.player.powers, WEAK);
    for enemy in &mut state.enemies {
        tick_power(&mut enemy.powers, VULNERABLE);
        tick_power(&mut enemy.powers, WEAK);
    }
    if state.player.hp <= 0 {
        update_terminal(state);
        return Ok(());
    }

    state.player.block = 0;
    state.player.energy = state.player.max_energy;
    state.combat.turn += 1;
    state.combat.current_side = "Player".into();
    draw_to_hand(state, 5)?;
    Ok(())
}

fn execute_enemy_move(
    catalog: &CombatCatalogV1,
    player: &mut PlayerState,
    enemy: &mut EnemyState,
) -> Result<(), SimulatorError> {
    let monster = catalog
        .monsters
        .get(&enemy.model_id)
        .ok_or_else(|| SimulatorError::UnknownId(enemy.model_id.clone()))?;
    let current = enemy.ai.current_move.clone();
    let movement = monster.moves.get(&current).cloned().ok_or_else(|| {
        SimulatorError::InvalidSnapshot(format!("unknown {} move {current}", enemy.model_id))
    })?;
    if let Some(damage) = movement.damage {
        damage_player(catalog, player, enemy, damage.at(enemy.ai.deadly_enemies));
    }
    if let Some(block) = movement.block {
        enemy.block += block.at(enemy.ai.tough_enemies);
    }
    if let Some(power) = movement.power {
        let amount = power.amount.at(enemy.ai.deadly_enemies);
        match power.target.as_str() {
            "self" => add_power(&mut enemy.powers, &power.id, amount),
            "player" => add_power(&mut player.powers, &power.id, amount),
            other => {
                return Err(SimulatorError::Catalog(format!(
                    "unsupported target {other} on {} move {current}",
                    enemy.model_id
                )));
            }
        }
    }
    enemy.ai.current_move = movement.next_move;
    enemy.ai.move_history.push(enemy.ai.current_move.clone());
    Ok(())
}

fn attack_damage(
    catalog: &CombatCatalogV1,
    base_damage: i32,
    attacker_powers: &[PowerState],
    defender_powers: &[PowerState],
    attacker_is_player: bool,
) -> i32 {
    let mut numerator = i64::from(base_damage + power_amount(attacker_powers, STRENGTH));
    let mut denominator = 1_i64;
    if power_amount(attacker_powers, WEAK) > 0 {
        numerator *= i64::from(catalog.combat_modifiers.weak.numerator);
        denominator *= i64::from(catalog.combat_modifiers.weak.denominator);
    }
    if attacker_is_player && power_amount(attacker_powers, SHRINK) > 0 {
        numerator *= i64::from(catalog.combat_modifiers.shrink.numerator);
        denominator *= i64::from(catalog.combat_modifiers.shrink.denominator);
    }
    if power_amount(defender_powers, VULNERABLE) > 0 {
        numerator *= i64::from(catalog.combat_modifiers.vulnerable.numerator);
        denominator *= i64::from(catalog.combat_modifiers.vulnerable.denominator);
    }
    (numerator / denominator).max(0) as i32
}

fn damage_player(
    catalog: &CombatCatalogV1,
    player: &mut PlayerState,
    enemy: &EnemyState,
    base_damage: i32,
) {
    let damage = attack_damage(catalog, base_damage, &enemy.powers, &player.powers, false);
    apply_damage(&mut player.hp, &mut player.block, damage);
}

fn apply_damage(hp: &mut i32, block: &mut i32, damage: i32) {
    let blocked = (*block).min(damage.max(0));
    *block -= blocked;
    *hp = (*hp - (damage - blocked)).max(0);
}

fn add_power(powers: &mut Vec<PowerState>, model_id: &str, amount: i32) {
    if let Some(power) = powers.iter_mut().find(|power| power.model_id == model_id) {
        power.amount += amount;
    } else {
        powers.push(PowerState {
            model_id: model_id.into(),
            amount,
        });
    }
}

fn power_amount(powers: &[PowerState], model_id: &str) -> i32 {
    powers
        .iter()
        .find(|power| power.model_id == model_id)
        .map_or(0, |power| power.amount)
}

fn tick_power(powers: &mut Vec<PowerState>, model_id: &str) {
    if let Some(power) = powers.iter_mut().find(|power| power.model_id == model_id) {
        power.amount -= 1;
    }
    powers.retain(|power| power.amount != 0);
}

fn draw_to_hand(state: &mut CombatState, target_size: usize) -> Result<(), SimulatorError> {
    while state.hand.len() < target_size {
        if state.draw_pile.is_empty() {
            if state.discard_pile.is_empty() {
                break;
            }
            dotnet_sort(&mut state.discard_pile);
            let algorithm = state.rng.algorithm.clone();
            let shuffle = state
                .rng
                .streams
                .get_mut("shuffle")
                .ok_or_else(|| SimulatorError::InvalidSnapshot("missing shuffle RNG".into()))?;
            shuffle_in_place(&mut state.discard_pile, shuffle, &algorithm);
            state.draw_pile.append(&mut state.discard_pile);
        }
        state.hand.push(state.draw_pile.remove(0));
    }
    Ok(())
}

fn card_compare(a: &CardInstance, b: &CardInstance) -> Ordering {
    a.model_id
        .cmp(&b.model_id)
        .then_with(|| a.upgrade_level.cmp(&b.upgrade_level))
}

// Port of the .NET introspective sort used by List<T>.Sort before STS2's
// StableShuffle. Equal starter cards follow the same insertion-sort path.
fn dotnet_sort(cards: &mut [CardInstance]) {
    if cards.len() < 2 {
        return;
    }
    let depth_limit = 2 * (usize::BITS - cards.len().leading_zeros()) as usize;
    intro_sort(cards, 0, cards.len() - 1, depth_limit);
}

fn intro_sort(cards: &mut [CardInstance], lo: usize, mut hi: usize, mut depth_limit: usize) {
    while hi > lo {
        let partition_size = hi - lo + 1;
        if partition_size <= 16 {
            insertion_sort(cards, lo, hi);
            return;
        }
        if depth_limit == 0 {
            heap_sort(cards, lo, hi);
            return;
        }
        depth_limit -= 1;
        let pivot = pick_pivot_and_partition(cards, lo, hi);
        if pivot < hi {
            intro_sort(cards, pivot + 1, hi, depth_limit);
        }
        if pivot == 0 {
            return;
        }
        hi = pivot - 1;
    }
}

fn insertion_sort(cards: &mut [CardInstance], lo: usize, hi: usize) {
    for i in (lo + 1)..=hi {
        let value = cards[i].clone();
        let mut j = i;
        while j > lo && card_compare(&value, &cards[j - 1]) == Ordering::Less {
            cards[j] = cards[j - 1].clone();
            j -= 1;
        }
        cards[j] = value;
    }
}

fn swap_if_greater(cards: &mut [CardInstance], a: usize, b: usize) {
    if a != b && card_compare(&cards[a], &cards[b]) == Ordering::Greater {
        cards.swap(a, b);
    }
}

fn pick_pivot_and_partition(cards: &mut [CardInstance], lo: usize, hi: usize) -> usize {
    let middle = lo + ((hi - lo) >> 1);
    swap_if_greater(cards, lo, middle);
    swap_if_greater(cards, lo, hi);
    swap_if_greater(cards, middle, hi);
    let pivot = cards[middle].clone();
    cards.swap(middle, hi - 1);
    let mut left = lo;
    let mut right = hi - 1;
    loop {
        left += 1;
        while card_compare(&cards[left], &pivot) == Ordering::Less {
            left += 1;
        }
        right -= 1;
        while card_compare(&pivot, &cards[right]) == Ordering::Less {
            right -= 1;
        }
        if left >= right {
            break;
        }
        cards.swap(left, right);
    }
    if left != hi - 1 {
        cards.swap(left, hi - 1);
    }
    left
}

fn heap_sort(cards: &mut [CardInstance], lo: usize, hi: usize) {
    let n = hi - lo + 1;
    for i in (1..=(n / 2)).rev() {
        down_heap(cards, i, n, lo);
    }
    for i in (2..=n).rev() {
        cards.swap(lo, lo + i - 1);
        down_heap(cards, 1, i - 1, lo);
    }
}

fn down_heap(cards: &mut [CardInstance], mut i: usize, n: usize, lo: usize) {
    let value = cards[lo + i - 1].clone();
    while i <= n / 2 {
        let mut child = 2 * i;
        if child < n && card_compare(&cards[lo + child - 1], &cards[lo + child]) == Ordering::Less {
            child += 1;
        }
        if card_compare(&value, &cards[lo + child - 1]) != Ordering::Less {
            break;
        }
        cards[lo + i - 1] = cards[lo + child - 1].clone();
        i = child;
    }
    cards[lo + i - 1] = value;
}

fn shuffle_in_place(cards: &mut [CardInstance], stream: &mut RngStreamState, algorithm: &str) {
    for index in (1..cards.len()).rev() {
        let swap = rng::next_int(algorithm, stream, (index + 1) as u32) as usize;
        cards.swap(index, swap);
    }
}

fn update_terminal(state: &mut CombatState) {
    state.combat.won = state.enemies.iter().all(|enemy| enemy.hp <= 0);
    state.combat.lost = state.player.hp <= 0;
    if state.combat.won || state.combat.lost {
        if state.combat.won {
            state.player.powers.retain(|power| power.model_id != SHRINK);
        }
        state.decision = Decision::Terminal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_spire_core::*;
    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    fn test_catalog() -> &'static CombatCatalog {
        static CATALOG: OnceLock<CombatCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json"))
                .unwrap()
        })
    }

    fn simulator() -> Simulator<'static> {
        Simulator::new(test_catalog())
    }

    fn package_simulator() -> Simulator<'static> {
        static PACKAGE: OnceLock<CombatCatalog> = OnceLock::new();
        Simulator::new(PACKAGE.get_or_init(|| {
            CombatCatalog::from_json(include_bytes!(
                "../../../packages/spire-codex-stable-v0.107.1.json"
            ))
            .unwrap()
        }))
    }

    fn source_contract() -> Value {
        serde_json::from_str(include_str!(
            "../../../fixtures/contracts/spire_codex_supported_content_v1.json"
        ))
        .unwrap()
    }

    fn contract_i64(contract: &Value, path: &[&str]) -> i64 {
        let mut value = contract;
        for key in path {
            value = &value[*key];
        }
        value.as_i64().unwrap()
    }

    fn card(id: &str, model_id: &str, cost: i32) -> CardInstance {
        CardInstance {
            instance_id: id.into(),
            model_id: model_id.into(),
            upgrade_level: 0,
            cost,
            cost_for_turn: None,
            retained: false,
            exhausts: false,
            ethereal: model_id == ASCENDERS_BANE,
        }
    }

    fn state() -> CombatState {
        CombatState {
            snapshot_schema: 2,
            provenance: Provenance {
                game_version: "v0.107.0".into(),
                game_commit: "23d60b98".into(),
                assembly_sha256: "fixture".into(),
                content_revision: "base".into(),
                modded_gameplay: false,
            },
            rng: RngBankState {
                algorithm: "xoshiro256_star_star_v1".into(),
                run_seed: "TEST".into(),
                streams: BTreeMap::from([(
                    "shuffle".into(),
                    RngStreamState {
                        seed: 1,
                        counter: 0,
                    },
                )]),
            },
            combat: CombatStatus {
                won: false,
                lost: false,
                turn: 1,
                current_side: "Player".into(),
                ascension_level: Some(0),
            },
            decision: Decision::PlayerAction,
            player: PlayerState {
                combat_id: "0".into(),
                model_id: "CHARACTER.IRONCLAD".into(),
                hp: 70,
                max_hp: 80,
                block: 0,
                energy: 3,
                max_energy: 3,
                powers: Vec::new(),
                relics: Vec::new(),
                potions: Vec::new(),
            },
            enemies: vec![EnemyState {
                combat_id: "1".into(),
                model_id: NIBBIT.into(),
                hp: 47,
                max_hp: 47,
                block: 0,
                powers: Vec::new(),
                ai: EnemyAiState {
                    current_move: "BUTT_MOVE".into(),
                    move_history: Vec::new(),
                    is_front: false,
                    is_alone: true,
                    tough_enemies: false,
                    deadly_enemies: false,
                },
            }],
            hand: vec![
                card("0", STRIKE, 1),
                card("1", DEFEND, 1),
                card("2", BASH, 2),
            ],
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            play_pile: Vec::new(),
            metrics: Metrics::default(),
        }
    }

    fn silent_state(enemy_model: &str, opening_move: &str) -> CombatState {
        let mut state = state();
        state.player.model_id = "CHARACTER.SILENT".into();
        state.player.max_hp = 70;
        state.player.relics = vec![ModelState {
            model_id: "RELIC.RING_OF_THE_SNAKE".into(),
        }];
        state.enemies[0].model_id = enemy_model.into();
        state.enemies[0].ai.current_move = opening_move.into();
        state.hand = vec![
            card("0", SILENT_STRIKE, 1),
            card("1", SILENT_DEFEND, 1),
            card("2", NEUTRALIZE, 0),
            card("3", SURVIVOR, 1),
        ];
        state
    }

    #[test]
    fn silent_ring_prepares_a_seeded_seven_card_hand() {
        let mut state = silent_state(NIBBIT, "BUTT_MOVE");
        state.hand.clear();
        state.draw_pile = (0..12)
            .map(|index| {
                let (model_id, cost) = match index {
                    0..=4 => (SILENT_STRIKE, 1),
                    5..=9 => (SILENT_DEFEND, 1),
                    10 => (NEUTRALIZE, 0),
                    _ => (SURVIVOR, 1),
                };
                card(&index.to_string(), model_id, cost)
            })
            .collect();
        state.rng.algorithm = "xoshiro256_star_star_v1".into();
        let prepared = simulator().prepare_combat_start(&state).unwrap();
        assert_eq!(prepared.hand.len(), 7);
        assert_eq!(prepared.draw_pile.len(), 5);
        assert_eq!(prepared.rng.streams["shuffle"].counter, 11);
    }

    #[test]
    fn ascension_level_must_match_enemy_tier_flags() {
        let mut input = state();
        input.combat.ascension_level = Some(9);
        let error = simulator().validate_state(&input).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ascension flags disagree with ascension_level 9")
        );

        input.enemies[0].ai.tough_enemies = true;
        input.enemies[0].ai.deadly_enemies = true;
        simulator().validate_state(&input).unwrap();
    }

    #[test]
    fn supported_mechanics_match_the_pinned_spire_codex_contract() {
        let contract = source_contract();
        let card_path = |card_id: &str, field: &str| {
            contract_i64(&contract, &["content", "cards", card_id, field]) as i32
        };

        for (model_id, source_id) in [
            (STRIKE, "STRIKE_IRONCLAD"),
            (SILENT_STRIKE, "STRIKE_SILENT"),
        ] {
            for (upgrade_level, field) in [(0, "damage"), (1, "upgrade_damage")] {
                let mut input = if model_id == SILENT_STRIKE {
                    silent_state(NIBBIT, "BUTT_MOVE")
                } else {
                    state()
                };
                let mut strike = card("contract-card", model_id, card_path(source_id, "cost"));
                strike.upgrade_level = upgrade_level;
                input.hand = vec![strike];
                input.enemies[0].hp = 100;
                input.enemies[0].max_hp = 100;
                let action = simulator()
                    .legal_actions(&input)
                    .unwrap()
                    .into_iter()
                    .find(|action| action.action_type == "card")
                    .unwrap();
                let output = simulator().step(&input, &action).unwrap();
                assert_eq!(output.enemies[0].hp, 100 - card_path(source_id, field));
            }
        }

        for (model_id, source_id) in [
            (DEFEND, "DEFEND_IRONCLAD"),
            (SILENT_DEFEND, "DEFEND_SILENT"),
            (SURVIVOR, "SURVIVOR"),
        ] {
            for (upgrade_level, field) in [(0, "block"), (1, "upgrade_block")] {
                let mut input = if model_id == DEFEND {
                    state()
                } else {
                    silent_state(NIBBIT, "BUTT_MOVE")
                };
                let mut defend = card("contract-card", model_id, card_path(source_id, "cost"));
                defend.upgrade_level = upgrade_level;
                input.hand = vec![defend];
                let action = simulator()
                    .legal_actions(&input)
                    .unwrap()
                    .into_iter()
                    .find(|action| action.action_type == "card")
                    .unwrap();
                let output = simulator().step(&input, &action).unwrap();
                assert_eq!(output.player.block, card_path(source_id, field));
            }
        }

        for (model_id, source_id, power_id) in
            [(BASH, "BASH", VULNERABLE), (NEUTRALIZE, "NEUTRALIZE", WEAK)]
        {
            for (upgrade_level, damage_field, power_field) in [
                (0, "damage", "power_amount"),
                (1, "upgrade_damage", "upgrade_power_amount"),
            ] {
                let mut input = if model_id == BASH {
                    state()
                } else {
                    silent_state(NIBBIT, "BUTT_MOVE")
                };
                let mut card = card(
                    "contract-power-card",
                    model_id,
                    card_path(source_id, "cost"),
                );
                card.upgrade_level = upgrade_level;
                input.hand = vec![card];
                input.enemies[0].hp = 100;
                input.enemies[0].max_hp = 100;
                let action = simulator()
                    .legal_actions(&input)
                    .unwrap()
                    .into_iter()
                    .find(|action| action.action_type == "card")
                    .unwrap();
                let output = simulator().step(&input, &action).unwrap();
                assert_eq!(
                    output.enemies[0].hp,
                    100 - card_path(source_id, damage_field)
                );
                assert_eq!(
                    power_amount(&output.enemies[0].powers, power_id),
                    card_path(source_id, power_field)
                );
            }
        }

        for (enemy_model, move_state, source_id, move_id) in [
            (NIBBIT, "BUTT_MOVE", "NIBBIT", "BUTT"),
            (NIBBIT, "SLICE_MOVE", "NIBBIT", "SLICE"),
            (
                FUZZY_WURM_CRAWLER,
                "FIRST_ACID_GOOP_MOVE",
                "FUZZY_WURM_CRAWLER",
                "FIRST_ACID_GOOP",
            ),
            (
                FUZZY_WURM_CRAWLER,
                "ACID_GOOP_MOVE",
                "FUZZY_WURM_CRAWLER",
                "ACID_GOOP",
            ),
            (SHRINKER_BEETLE, "CHOMP_MOVE", "SHRINKER_BEETLE", "CHOMP"),
            (SHRINKER_BEETLE, "STOMP_MOVE", "SHRINKER_BEETLE", "STOMP"),
        ] {
            for (deadly, field) in [(false, "damage"), (true, "ascension_damage")] {
                let mut input = silent_state(enemy_model, move_state);
                input.hand.clear();
                input.combat.ascension_level = Some(if deadly { 9 } else { 0 });
                input.enemies[0].ai.tough_enemies = deadly;
                input.enemies[0].ai.deadly_enemies = deadly;
                let end = simulator().legal_actions(&input).unwrap().pop().unwrap();
                let output = simulator().step(&input, &end).unwrap();
                let expected = contract_i64(
                    &contract,
                    &["content", "monsters", source_id, "moves", move_id, field],
                ) as i32;
                assert_eq!(output.player.hp, 70 - expected);
                if move_id == "SLICE" && !deadly {
                    assert_eq!(
                        output.enemies[0].block,
                        contract_i64(
                            &contract,
                            &["content", "monsters", source_id, "moves", move_id, "block"],
                        ) as i32
                    );
                }
            }
        }

        for (enemy_model, move_state, source_id, move_id, power_id) in [
            (NIBBIT, "HISS_MOVE", "NIBBIT", "HISS", STRENGTH),
            (
                FUZZY_WURM_CRAWLER,
                "INHALE_MOVE",
                "FUZZY_WURM_CRAWLER",
                "INHALE",
                STRENGTH,
            ),
        ] {
            let mut input = silent_state(enemy_model, move_state);
            input.hand.clear();
            let end = simulator().legal_actions(&input).unwrap().pop().unwrap();
            let output = simulator().step(&input, &end).unwrap();
            assert_eq!(
                power_amount(&output.enemies[0].powers, power_id),
                contract_i64(
                    &contract,
                    &[
                        "content",
                        "monsters",
                        source_id,
                        "moves",
                        move_id,
                        "power_amount",
                    ],
                ) as i32
            );
        }
    }

    #[test]
    fn survivor_resolves_its_discard_as_a_branchable_choice() {
        let mut state = silent_state(NIBBIT, "BUTT_MOVE");
        state.hand = vec![
            card("strike", SILENT_STRIKE, 1),
            card("defend", SILENT_DEFEND, 1),
            card("survivor", SURVIVOR, 1),
        ];
        let survivor = simulator()
            .legal_actions(&state)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(SURVIVOR))
            .unwrap();
        let selecting = simulator().step(&state, &survivor).unwrap();
        assert_eq!(selecting.player.block, 8);
        assert!(matches!(selecting.decision, Decision::CardSelection { .. }));
        let discard_strike = simulator()
            .legal_actions(&selecting)
            .unwrap()
            .into_iter()
            .find(|action| action.selection == ["strike"])
            .unwrap();
        let resolved = simulator().step(&selecting, &discard_strike).unwrap();
        assert!(matches!(resolved.decision, Decision::PlayerAction));
        assert_eq!(resolved.hand[0].instance_id, "defend");
        assert_eq!(
            resolved
                .discard_pile
                .iter()
                .map(|card| card.instance_id.as_str())
                .collect::<Vec<_>>(),
            vec!["strike", "survivor"]
        );
    }

    #[test]
    fn composable_effect_cards_follow_reviewed_order_and_upgrades() {
        for upgrade in [0, 1] {
            let expected_iron_wave = if upgrade == 0 { 5 } else { 7 };
            let mut iron = state();
            iron.hand = vec![card("iron-wave", IRON_WAVE, 1)];
            iron.hand[0].upgrade_level = upgrade;
            iron.enemies[0].hp = 50;
            iron.enemies[0].max_hp = 50;
            let action = package_simulator().legal_actions(&iron).unwrap()[0].clone();
            let after = package_simulator().step(&iron, &action).unwrap();
            assert_eq!(after.player.block, expected_iron_wave);
            assert_eq!(after.enemies[0].hp, 50 - expected_iron_wave);

            let mut pommel = state();
            pommel.hand = vec![card("pommel", POMMEL_STRIKE, 1)];
            pommel.hand[0].upgrade_level = upgrade;
            pommel.draw_pile = vec![card("draw-a", DEFEND, 1), card("draw-b", DEFEND, 1)];
            pommel.enemies[0].hp = 50;
            pommel.enemies[0].max_hp = 50;
            let action = package_simulator().legal_actions(&pommel).unwrap()[0].clone();
            let after = package_simulator().step(&pommel, &action).unwrap();
            assert_eq!(after.enemies[0].hp, 50 - if upgrade == 0 { 9 } else { 10 });
            assert_eq!(after.hand.len(), if upgrade == 0 { 1 } else { 2 });

            let mut backflip = silent_state(NIBBIT, "BUTT_MOVE");
            backflip.hand = vec![card("backflip", BACKFLIP, 1)];
            backflip.hand[0].upgrade_level = upgrade;
            backflip.draw_pile = vec![
                card("silent-draw-a", SILENT_DEFEND, 1),
                card("silent-draw-b", SILENT_DEFEND, 1),
            ];
            let action = package_simulator().legal_actions(&backflip).unwrap()[0].clone();
            let after = package_simulator().step(&backflip, &action).unwrap();
            assert_eq!(after.player.block, if upgrade == 0 { 5 } else { 8 });
            assert_eq!(after.hand.len(), 2);

            let mut adrenaline = silent_state(NIBBIT, "BUTT_MOVE");
            adrenaline.player.energy = 1;
            adrenaline.hand = vec![card("adrenaline", ADRENALINE, 0)];
            adrenaline.hand[0].upgrade_level = upgrade;
            adrenaline.hand[0].exhausts = true;
            adrenaline.draw_pile = vec![
                card("adrenaline-draw-a", SILENT_DEFEND, 1),
                card("adrenaline-draw-b", SILENT_DEFEND, 1),
            ];
            let action = package_simulator().legal_actions(&adrenaline).unwrap()[0].clone();
            let after = package_simulator().step(&adrenaline, &action).unwrap();
            assert_eq!(after.player.energy, 1 + if upgrade == 0 { 1 } else { 2 });
            assert_eq!(after.hand.len(), 2);
            assert_eq!(after.exhaust_pile[0].model_id, ADRENALINE);
        }
    }

    #[test]
    fn neutralize_weak_reduces_the_next_enemy_attack() {
        let mut state = silent_state(NIBBIT, "BUTT_MOVE");
        state.hand = vec![card("neutralize", NEUTRALIZE, 0)];
        let neutralize = simulator()
            .legal_actions(&state)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(NEUTRALIZE))
            .unwrap();
        let weakened = simulator().step(&state, &neutralize).unwrap();
        assert_eq!(power_amount(&weakened.enemies[0].powers, WEAK), 1);
        let end = simulator()
            .legal_actions(&weakened)
            .unwrap()
            .into_iter()
            .find(|action| action.action_type == "end_turn")
            .unwrap();
        let after = simulator().step(&weakened, &end).unwrap();
        assert_eq!(after.player.hp, 61);
        assert_eq!(power_amount(&after.enemies[0].powers, WEAK), 0);
    }

    #[test]
    fn fuzzy_wurm_crawler_uses_its_fixed_scaling_cycle() {
        let mut state = silent_state(FUZZY_WURM_CRAWLER, "FIRST_ACID_GOOP_MOVE");
        state.hand.clear();
        let end = simulator().legal_actions(&state).unwrap().pop().unwrap();
        let after_first = simulator().step(&state, &end).unwrap();
        assert_eq!(after_first.player.hp, 66);
        assert_eq!(after_first.enemies[0].ai.current_move, "INHALE_MOVE");
        let after_inhale = simulator().step(&after_first, &end).unwrap();
        assert_eq!(after_inhale.player.hp, 66);
        assert_eq!(power_amount(&after_inhale.enemies[0].powers, STRENGTH), 7);
        let after_scaled_hit = simulator().step(&after_inhale, &end).unwrap();
        assert_eq!(after_scaled_hit.player.hp, 55);
    }

    #[test]
    fn shrinker_beetle_reduces_player_attack_damage() {
        let mut state = silent_state(SHRINKER_BEETLE, "SHRINKER_MOVE");
        state.hand.clear();
        let end = simulator().legal_actions(&state).unwrap().pop().unwrap();
        let mut shrunk = simulator().step(&state, &end).unwrap();
        assert_eq!(power_amount(&shrunk.player.powers, SHRINK), 1);
        shrunk.hand.push(card("strike", SILENT_STRIKE, 1));
        let strike = simulator()
            .legal_actions(&shrunk)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(SILENT_STRIKE))
            .unwrap();
        let after_strike = simulator().step(&shrunk, &strike).unwrap();
        assert_eq!(after_strike.enemies[0].hp, 43);
    }

    #[test]
    fn bash_then_strike_uses_vulnerable() {
        let state = state();
        let bash = simulator()
            .legal_actions(&state)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(BASH))
            .unwrap();
        let after_bash = simulator().step(&state, &bash).unwrap();
        assert_eq!(after_bash.enemies[0].hp, 39);
        assert_eq!(power_amount(&after_bash.enemies[0].powers, VULNERABLE), 2);
        let strike = simulator()
            .legal_actions(&after_bash)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(STRIKE))
            .unwrap();
        let after_strike = simulator().step(&after_bash, &strike).unwrap();
        assert_eq!(after_strike.enemies[0].hp, 30);
        assert_eq!(
            state.enemies[0].hp, 47,
            "parent state must remain immutable"
        );
    }

    #[test]
    fn nibbit_cycle_and_ascenders_bane_are_exact() {
        let mut state = state();
        state.hand.push(card("3", ASCENDERS_BANE, -1));
        let end = simulator()
            .legal_actions(&state)
            .unwrap()
            .into_iter()
            .find(|action| action.action_type == "end_turn")
            .unwrap();
        let after = simulator().step(&state, &end).unwrap();
        assert_eq!(after.player.hp, 58);
        assert_eq!(after.enemies[0].ai.current_move, "SLICE_MOVE");
        assert!(
            after
                .exhaust_pile
                .iter()
                .any(|card| card.model_id == ASCENDERS_BANE)
        );
    }

    #[test]
    fn catalog_drives_ascension_moves_and_inert_relics() {
        let mut slice = state();
        slice.hand.clear();
        slice.combat.ascension_level = Some(9);
        slice.enemies[0].ai.current_move = "SLICE_MOVE".into();
        slice.enemies[0].ai.tough_enemies = true;
        slice.enemies[0].ai.deadly_enemies = true;
        let end = simulator().legal_actions(&slice).unwrap().pop().unwrap();
        let after = simulator().step(&slice, &end).unwrap();
        assert_eq!(after.player.hp, 63);
        assert_eq!(after.enemies[0].block, 6);

        let mut hiss = slice;
        hiss.enemies[0].ai.current_move = "HISS_MOVE".into();
        let end = simulator().legal_actions(&hiss).unwrap().pop().unwrap();
        let after = simulator().step(&hiss, &end).unwrap();
        assert_eq!(power_amount(&after.enemies[0].powers, STRENGTH), 3);

        let mut opening = state();
        opening.hand.clear();
        opening.draw_pile = (0..8)
            .map(|index| card(&index.to_string(), STRIKE, 1))
            .collect();
        opening.player.relics = vec![
            ModelState {
                model_id: "RELIC.BURNING_BLOOD".into(),
            },
            ModelState {
                model_id: "RELIC.WINGED_BOOTS".into(),
            },
        ];
        let opening = simulator().prepare_combat_start(&opening).unwrap();
        assert_eq!(opening.hand.len(), 5);
    }

    #[test]
    fn hash_distinguishes_rng_counters() {
        let a = state();
        let mut b = a.clone();
        b.rng.streams.get_mut("shuffle").unwrap().counter = 1;
        assert_ne!(
            simulator().state_hash(&a).unwrap(),
            simulator().state_hash(&b).unwrap()
        );
    }

    #[test]
    fn canonical_hot_path_preserves_v02_state_hashes() {
        let input = state();
        let legacy = blake3::hash(&serde_json::to_vec(&input).unwrap())
            .to_hex()
            .to_string();
        assert_eq!(simulator().state_hash(&input).unwrap(), legacy);
    }

    #[test]
    fn initial_fisher_yates_matches_seeded_xoshiro_vector() {
        let mut cards = vec![
            card("0", STRIKE, 1),
            card("1", STRIKE, 1),
            card("2", STRIKE, 1),
            card("3", STRIKE, 1),
            card("4", STRIKE, 1),
        ];
        let mut stream = RngStreamState {
            seed: 1,
            counter: 0,
        };
        shuffle_in_place(&mut cards, &mut stream, "xoshiro256_star_star_v1");
        assert_eq!(
            cards
                .iter()
                .map(|card| card.instance_id.as_str())
                .collect::<Vec<_>>(),
            vec!["4", "0", "1", "2", "3"]
        );
        assert_eq!(stream.counter, 4);
    }

    #[test]
    fn dotnet_sort_orders_large_piles_by_model_then_upgrade() {
        let mut cards = (0..40)
            .rev()
            .map(|index| {
                let mut value = card(
                    &index.to_string(),
                    if index % 2 == 0 { STRIKE } else { DEFEND },
                    1,
                );
                value.upgrade_level = (index % 3) as u8;
                value
            })
            .collect::<Vec<_>>();
        dotnet_sort(&mut cards);
        assert!(
            cards
                .windows(2)
                .all(|pair| card_compare(&pair[0], &pair[1]) != Ordering::Greater)
        );
    }

    #[test]
    fn branch_order_cannot_change_repeated_transition() {
        let state = state();
        let actions = simulator().legal_actions(&state).unwrap();
        let strike = actions
            .iter()
            .find(|action| action.card_id.as_deref() == Some(STRIKE))
            .unwrap();
        let defend = actions
            .iter()
            .find(|action| action.card_id.as_deref() == Some(DEFEND))
            .unwrap();
        let first = simulator().step(&state, strike).unwrap();
        let _other_branch = simulator().step(&state, defend).unwrap();
        let repeated = simulator().step(&state, strike).unwrap();
        assert_eq!(
            simulator().state_hash(&first).unwrap(),
            simulator().state_hash(&repeated).unwrap()
        );
    }

    #[test]
    fn replaying_the_same_exhaust_choice_preserves_the_draw_sequence() {
        let mut original = state();
        original.hand = vec![card("bane", ASCENDERS_BANE, -1), card("held", STRIKE, 1)];
        original.draw_pile.clear();
        original.discard_pile = vec![
            card("a", STRIKE, 1),
            card("b", DEFEND, 1),
            card("c", STRIKE, 1),
            card("d", DEFEND, 1),
            card("e", BASH, 2),
        ];
        let end = simulator()
            .legal_actions(&original)
            .unwrap()
            .into_iter()
            .find(|action| action.action_type == "end_turn")
            .unwrap();

        let first_run = simulator().step(&original, &end).unwrap();
        let strike = simulator()
            .legal_actions(&original)
            .unwrap()
            .into_iter()
            .find(|action| action.card_id.as_deref() == Some(STRIKE))
            .unwrap();
        let _different_branch = simulator().step(&original, &strike).unwrap();
        let restarted_run = simulator().step(&original, &end).unwrap();

        assert_eq!(
            first_run
                .hand
                .iter()
                .map(|card| card.instance_id.as_str())
                .collect::<Vec<_>>(),
            restarted_run
                .hand
                .iter()
                .map(|card| card.instance_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(first_run.rng.streams, restarted_run.rng.streams);
        assert_eq!(
            simulator().state_hash(&first_run).unwrap(),
            simulator().state_hash(&restarted_run).unwrap()
        );
    }

    #[test]
    fn changing_reshuffle_membership_changes_only_expected_rng_consumption() {
        let mut full = state();
        full.hand.clear();
        full.draw_pile.clear();
        full.discard_pile = vec![
            card("a", STRIKE, 1),
            card("b", DEFEND, 1),
            card("c", STRIKE, 1),
            card("d", DEFEND, 1),
            card("e", BASH, 2),
            card("f", STRIKE, 1),
        ];
        let mut reduced = full.clone();
        reduced.discard_pile.retain(|card| card.instance_id != "c");
        let end = simulator()
            .legal_actions(&full)
            .unwrap()
            .into_iter()
            .find(|action| action.action_type == "end_turn")
            .unwrap();
        let full_after = simulator().step(&full, &end).unwrap();
        let reduced_after = simulator().step(&reduced, &end).unwrap();
        assert_eq!(full_after.rng.streams["shuffle"].counter, 5);
        assert_eq!(reduced_after.rng.streams["shuffle"].counter, 4);
        assert!(
            full_after
                .hand
                .iter()
                .chain(&full_after.draw_pile)
                .any(|card| card.instance_id == "c")
        );
        assert!(
            !reduced_after
                .hand
                .iter()
                .any(|card| card.instance_id == "c")
        );
    }
}
