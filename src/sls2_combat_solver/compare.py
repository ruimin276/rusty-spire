from __future__ import annotations

from dataclasses import dataclass

from .oracle import CombatOracle
from .schema import JsonDict
from .solver import SolveLimits, SolveResult, solve_combat


@dataclass(frozen=True)
class ComparisonResult:
    baseline: SolveResult
    candidate: SolveResult
    hp_loss_delta: int | None
    better: str

    def to_json(self) -> JsonDict:
        return {
            "baseline": self.baseline.to_json(),
            "candidate": self.candidate.to_json(),
            "hp_loss_delta": self.hp_loss_delta,
            "better": self.better,
        }


def compare_scenarios(
    baseline_state: JsonDict,
    candidate_state: JsonDict,
    baseline_oracle: CombatOracle,
    candidate_oracle: CombatOracle,
    limits: SolveLimits | None = None,
) -> ComparisonResult:
    baseline = solve_combat(baseline_state, baseline_oracle, limits)
    candidate = solve_combat(candidate_state, candidate_oracle, limits)
    delta = _delta(baseline, candidate)
    return ComparisonResult(
        baseline=baseline,
        candidate=candidate,
        hp_loss_delta=delta,
        better=_better(baseline, candidate, delta),
    )


def _delta(baseline: SolveResult, candidate: SolveResult) -> int | None:
    if not baseline.won or not candidate.won:
        return None
    if baseline.hp_loss is None or candidate.hp_loss is None:
        return None
    return candidate.hp_loss - baseline.hp_loss


def _better(baseline: SolveResult, candidate: SolveResult, delta: int | None) -> str:
    if baseline.won and not candidate.won:
        return "baseline"
    if candidate.won and not baseline.won:
        return "candidate"
    if not baseline.won and not candidate.won:
        return "neither"
    if delta is None or delta == 0:
        return "tie"
    return "candidate" if delta < 0 else "baseline"
