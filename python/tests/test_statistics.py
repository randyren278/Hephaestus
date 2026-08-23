import unittest

from hephaestus_lab.statistics import (
    ComparisonPolicy,
    FitnessVector,
    PairedOutcome,
    analyze_selection,
    paired_bootstrap,
    pareto_dominates,
)


def outcome(parent: bool, candidate: bool, *, regression: bool = False) -> PairedOutcome:
    return PairedOutcome(
        parent_correct=parent,
        candidate_correct=candidate,
        parent_reliable=True,
        candidate_reliable=True,
        parent_cost_microusd=10,
        candidate_cost_microusd=9,
        parent_latency_millis=20,
        candidate_latency_millis=19,
        invariant_regression=regression,
    )


class StatisticsTests(unittest.TestCase):
    def test_bootstrap_is_seeded_and_reproducible(self) -> None:
        values = [1, 1, 1, 0, 0, 1, 1, 0]
        first = paired_bootstrap(values, seed=42, resamples=1_000)
        second = paired_bootstrap(values, seed=42, resamples=1_000)
        self.assertEqual(first, second)
        self.assertEqual(first.estimate_bps, 6_250)
        self.assertEqual((first.lower_bps, first.upper_bps), (2_500, 10_000))

    def test_pareto_keeps_dimensions_separate(self) -> None:
        parent = FitnessVector(8_000, 9_000, 100, 100)
        better = FitnessVector(8_500, 9_000, 90, 100)
        tradeoff = FitnessVector(8_500, 9_000, 110, 100)
        self.assertTrue(pareto_dominates(better, parent))
        self.assertFalse(pareto_dominates(tradeoff, parent))
        self.assertFalse(pareto_dominates(parent, parent))

    def test_selection_requires_effect_confidence_regression_and_pareto_gates(self) -> None:
        outcomes = [outcome(False, True) for _ in range(8)]
        policy = ComparisonPolicy(5_000, 0, 9_500)
        accepted = analyze_selection(outcomes, policy, seed=7, resamples=1_000)
        self.assertTrue(accepted.promotion_eligible)
        regressed = list(outcomes)
        regressed[0] = outcome(False, True, regression=True)
        denied = analyze_selection(regressed, policy, seed=7, resamples=1_000)
        self.assertFalse(denied.promotion_eligible)
        tied = analyze_selection(
            [outcome(True, True) for _ in range(8)], policy, seed=7, resamples=1_000
        )
        self.assertEqual(tied.correctness.estimate_bps, 0)
        self.assertFalse(tied.promotion_eligible)

    def test_invalid_statistics_inputs_fail_closed(self) -> None:
        with self.assertRaises(ValueError):
            paired_bootstrap([1], seed=1)
        with self.assertRaises(ValueError):
            paired_bootstrap([1, 0], seed=1, resamples=99)
        with self.assertRaises(ValueError):
            ComparisonPolicy(1, -1, 9_500)
        with self.assertRaises(ValueError):
            PairedOutcome(True, True, True, True, -1, 0, 0, 0)


if __name__ == "__main__":
    unittest.main()
