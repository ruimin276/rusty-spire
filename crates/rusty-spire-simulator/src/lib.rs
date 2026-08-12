//! Exact and explicitly approximate combat search.

mod clock;
mod objective;
mod search;

pub use objective::{CombatObjective, MinimizeHpLoss, ObjectiveKind};
pub use search::{
    CompareResult, Heuristic, SearchMode, SolveLimits, SolveResult, TraceStep, ZeroHeuristic,
    compare, solve, solve_with,
};
