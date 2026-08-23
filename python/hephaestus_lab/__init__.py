"""Deterministic, read-only research helpers for Hephaestus evidence."""

from .statistics import (
    BootstrapInterval,
    ComparisonPolicy,
    FitnessVector,
    PairedOutcome,
    SelectionAnalysis,
    analyze_selection,
    paired_bootstrap,
    pareto_dominates,
)

__all__ = [
    "BootstrapInterval",
    "ComparisonPolicy",
    "FitnessVector",
    "PairedOutcome",
    "SelectionAnalysis",
    "analyze_selection",
    "paired_bootstrap",
    "pareto_dominates",
]
