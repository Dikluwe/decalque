//! Avaliador tipográfico puro: recebe somente relatórios comparativos
//! materializados, sem serialização, I/O, Typst ou geração de hipóteses.

use std::collections::HashSet;
use std::fmt;

use super::{CandidateUnitRef, EvidenceStatus, ScanComparisonReport, ScanMatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanTypographyIneligibilityReason {
    EmptyScanScope,
    ContentNotPreserved,
    PartialCoverage,
    UnmatchedUnits,
    MissingHorizontalEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanTypographyEligibility {
    Eligible,
    Ineligible(ScanTypographyIneligibilityReason),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScanTypographySupport {
    pub horizontal_scan_unit_ids: Vec<String>,
    pub known_baseline_scan_unit_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScanTypographyFitKey {
    pub horizontal_violations: u64,
    pub horizontal_residuals_desc: Vec<u64>,
    pub baseline_violations: u64,
    pub baseline_residuals_desc: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTypographyTrialEvaluation {
    pub index: usize,
    pub eligibility: ScanTypographyEligibility,
    pub support: Option<ScanTypographySupport>,
    pub key: Option<ScanTypographyFitKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanTypographyEvidenceScope {
    HorizontalOnly,
    HorizontalAndObservedBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanTypographyInconclusiveReason {
    NoEligibleTrial,
    IncomparableSupport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanTypographySelection {
    Selected {
        index: usize,
        evidence_scope: ScanTypographyEvidenceScope,
    },
    Tied {
        indices: Vec<usize>,
        evidence_scope: ScanTypographyEvidenceScope,
    },
    Inconclusive {
        reason: ScanTypographyInconclusiveReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTypographyFitEvaluation {
    pub trials: Vec<ScanTypographyTrialEvaluation>,
    pub selection: ScanTypographySelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanTypographyResidual {
    DxStart,
    DxEnd,
    WidthDelta,
    BaselineDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanTypographyFitError {
    IncoherentCoverage {
        index: usize,
    },
    EmptyScanUnitId {
        index: usize,
        match_index: usize,
    },
    DuplicateScanUnitId {
        index: usize,
        match_index: usize,
        scan_unit_id: String,
    },
    InvalidHorizontalEvidence {
        index: usize,
        match_index: usize,
    },
    InvalidBaselineEvidence {
        index: usize,
        match_index: usize,
    },
    NonFiniteResidual {
        index: usize,
        match_index: usize,
        residual: ScanTypographyResidual,
    },
    QuantizedResidualOutOfRange {
        index: usize,
        match_index: usize,
        residual: ScanTypographyResidual,
    },
    ViolationCountOutOfRange {
        index: usize,
    },
}

impl fmt::Display for ScanTypographyFitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncoherentCoverage { index } => {
                write!(formatter, "scan comparison report {index} has incoherent coverage")
            }
            Self::EmptyScanUnitId { index, match_index } => write!(
                formatter,
                "scan comparison report {index} match {match_index} has an empty scan unit id"
            ),
            Self::DuplicateScanUnitId {
                index,
                match_index,
                scan_unit_id,
            } => write!(
                formatter,
                "scan comparison report {index} match {match_index} repeats scan unit id {scan_unit_id:?}"
            ),
            Self::InvalidHorizontalEvidence { index, match_index } => write!(
                formatter,
                "scan comparison report {index} match {match_index} has contradictory or partial horizontal evidence"
            ),
            Self::InvalidBaselineEvidence { index, match_index } => write!(
                formatter,
                "scan comparison report {index} match {match_index} has contradictory baseline evidence"
            ),
            Self::NonFiniteResidual {
                index,
                match_index,
                residual,
            } => write!(
                formatter,
                "scan comparison report {index} match {match_index} has a non-finite {residual} residual"
            ),
            Self::QuantizedResidualOutOfRange {
                index,
                match_index,
                residual,
            } => write!(
                formatter,
                "scan comparison report {index} match {match_index} has a {residual} residual whose quantized value exceeds u64"
            ),
            Self::ViolationCountOutOfRange { index } => write!(
                formatter,
                "scan comparison report {index} has a violation count that exceeds u64"
            ),
        }
    }
}

impl std::error::Error for ScanTypographyFitError {}

impl fmt::Display for ScanTypographyResidual {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DxStart => "dx_start",
            Self::DxEnd => "dx_end",
            Self::WidthDelta => "width_delta",
            Self::BaselineDelta => "baseline_delta",
        })
    }
}

struct ValidatedTrial {
    support: ScanTypographySupport,
    key: Option<ScanTypographyFitKey>,
}

pub fn evaluate_scan_typography_fit(
    reports: &[ScanComparisonReport],
) -> Result<ScanTypographyFitEvaluation, ScanTypographyFitError> {
    // Validation is deliberately completed for every report before eligibility or
    // selection is considered. An early ineligible trial cannot hide malformed
    // evidence in a later report.
    let validated = reports
        .iter()
        .enumerate()
        .map(|(index, report)| validate_report(index, report))
        .collect::<Result<Vec<_>, _>>()?;

    let trials = reports
        .iter()
        .zip(validated)
        .enumerate()
        .map(|(index, (report, validated))| {
            let reason = ineligibility_reason(report, &validated);
            match reason {
                Some(reason) => ScanTypographyTrialEvaluation {
                    index,
                    eligibility: ScanTypographyEligibility::Ineligible(reason),
                    support: None,
                    key: None,
                },
                None => ScanTypographyTrialEvaluation {
                    index,
                    eligibility: ScanTypographyEligibility::Eligible,
                    support: Some(validated.support),
                    key: validated.key,
                },
            }
        })
        .collect::<Vec<_>>();

    let selection = select(&trials);
    Ok(ScanTypographyFitEvaluation { trials, selection })
}

fn validate_report(
    index: usize,
    report: &ScanComparisonReport,
) -> Result<ValidatedTrial, ScanTypographyFitError> {
    validate_coverage(index, report)?;

    let mut scan_unit_ids = HashSet::with_capacity(report.matches.len());
    let mut horizontal_scan_unit_ids = Vec::with_capacity(report.matches.len());
    let mut known_baseline_scan_unit_ids = Vec::new();
    let mut horizontal_residuals_desc = Vec::with_capacity(report.matches.len());
    let mut baseline_residuals_desc = Vec::new();
    let mut horizontal_violations = 0usize;
    let mut baseline_violations = 0usize;
    let mut missing_horizontal_evidence = false;

    for (match_index, matched) in report.matches.iter().enumerate() {
        if matched.scan_unit_id.is_empty() {
            return Err(ScanTypographyFitError::EmptyScanUnitId { index, match_index });
        }
        if !scan_unit_ids.insert(matched.scan_unit_id.as_str()) {
            return Err(ScanTypographyFitError::DuplicateScanUnitId {
                index,
                match_index,
                scan_unit_id: matched.scan_unit_id.clone(),
            });
        }
        horizontal_scan_unit_ids.push(matched.scan_unit_id.clone());

        match horizontal_residual(index, match_index, matched)? {
            Some(residual) => {
                horizontal_residuals_desc.push(residual);
                if matched.horizontal_status == EvidenceStatus::Violated {
                    horizontal_violations += 1;
                }
            }
            None => missing_horizontal_evidence = true,
        }

        if let Some(residual) = baseline_residual(index, match_index, matched)? {
            known_baseline_scan_unit_ids.push(matched.scan_unit_id.clone());
            baseline_residuals_desc.push(residual);
            if matched.baseline_status == EvidenceStatus::Violated {
                baseline_violations += 1;
            }
        }
    }

    horizontal_scan_unit_ids.sort();
    known_baseline_scan_unit_ids.sort();
    horizontal_residuals_desc.sort_by(|left, right| right.cmp(left));
    baseline_residuals_desc.sort_by(|left, right| right.cmp(left));

    let support = ScanTypographySupport {
        horizontal_scan_unit_ids,
        known_baseline_scan_unit_ids,
    };
    let key = if missing_horizontal_evidence {
        None
    } else {
        Some(ScanTypographyFitKey {
            horizontal_violations: u64::try_from(horizontal_violations)
                .map_err(|_| ScanTypographyFitError::ViolationCountOutOfRange { index })?,
            horizontal_residuals_desc,
            baseline_violations: u64::try_from(baseline_violations)
                .map_err(|_| ScanTypographyFitError::ViolationCountOutOfRange { index })?,
            baseline_residuals_desc,
        })
    };

    Ok(ValidatedTrial { support, key })
}

fn validate_coverage(
    index: usize,
    report: &ScanComparisonReport,
) -> Result<(), ScanTypographyFitError> {
    let coverage = &report.coverage;
    let matched_candidate_units = report
        .matches
        .iter()
        .map(candidate_unit_from_match)
        .collect::<HashSet<_>>();
    let unmatched_scan_units = report
        .unmatched_scan
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let unmatched_candidate_units = report.unmatched_candidate.iter().collect::<HashSet<_>>();
    let matched_scan_units = report
        .matches
        .iter()
        .map(|matched| matched.scan_unit_id.as_str())
        .collect::<HashSet<_>>();

    let coherent = coverage.matched_scan == report.matches.len()
        && coverage.matched_candidate == matched_candidate_units.len()
        && matched_candidate_units.len() == report.matches.len()
        && unmatched_scan_units.len() == report.unmatched_scan.len()
        && unmatched_candidate_units.len() == report.unmatched_candidate.len()
        && matched_scan_units.is_disjoint(&unmatched_scan_units)
        && matched_candidate_units
            .iter()
            .all(|unit| !unmatched_candidate_units.contains(unit))
        && coverage
            .matched_scan
            .checked_add(report.unmatched_scan.len())
            == Some(coverage.total_scan)
        && coverage
            .matched_candidate
            .checked_add(report.unmatched_candidate.len())
            == Some(coverage.total_candidate);

    if coherent {
        Ok(())
    } else {
        Err(ScanTypographyFitError::IncoherentCoverage { index })
    }
}

fn candidate_unit_from_match(matched: &ScanMatch) -> CandidateUnitRef {
    CandidateUnitRef {
        candidate_line_index: matched.candidate_line_index,
        candidate_scalar_range: matched.candidate_scalar_range,
    }
}

fn horizontal_residual(
    index: usize,
    match_index: usize,
    matched: &ScanMatch,
) -> Result<Option<u64>, ScanTypographyFitError> {
    match (
        matched.horizontal_status,
        matched.dx_start,
        matched.dx_end,
        matched.width_delta,
    ) {
        (EvidenceStatus::Unknown, None, None, None) => Ok(None),
        (EvidenceStatus::Unknown, Some(dx_start), Some(dx_end), Some(width_delta)) => {
            quantize(
                index,
                match_index,
                ScanTypographyResidual::DxStart,
                dx_start,
            )?;
            quantize(index, match_index, ScanTypographyResidual::DxEnd, dx_end)?;
            quantize(
                index,
                match_index,
                ScanTypographyResidual::WidthDelta,
                width_delta,
            )?;
            Ok(None)
        }
        (
            EvidenceStatus::Preserved | EvidenceStatus::Violated,
            Some(dx_start),
            Some(dx_end),
            Some(width_delta),
        ) => {
            let dx_start = quantize(
                index,
                match_index,
                ScanTypographyResidual::DxStart,
                dx_start,
            )?;
            let dx_end = quantize(index, match_index, ScanTypographyResidual::DxEnd, dx_end)?;
            let width_delta = quantize(
                index,
                match_index,
                ScanTypographyResidual::WidthDelta,
                width_delta,
            )?;
            Ok(Some(dx_start.max(dx_end).max(width_delta)))
        }
        _ => Err(ScanTypographyFitError::InvalidHorizontalEvidence { index, match_index }),
    }
}

fn baseline_residual(
    index: usize,
    match_index: usize,
    matched: &ScanMatch,
) -> Result<Option<u64>, ScanTypographyFitError> {
    match (matched.baseline_status, matched.baseline_delta) {
        (EvidenceStatus::Unknown, None) => Ok(None),
        (EvidenceStatus::Preserved | EvidenceStatus::Violated, Some(delta)) => quantize(
            index,
            match_index,
            ScanTypographyResidual::BaselineDelta,
            delta,
        )
        .map(Some),
        _ => Err(ScanTypographyFitError::InvalidBaselineEvidence { index, match_index }),
    }
}

fn quantize(
    index: usize,
    match_index: usize,
    residual: ScanTypographyResidual,
    value: f64,
) -> Result<u64, ScanTypographyFitError> {
    if !value.is_finite() {
        return Err(ScanTypographyFitError::NonFiniteResidual {
            index,
            match_index,
            residual,
        });
    }

    let scaled = value.abs() * 1024.0;
    if !scaled.is_finite() {
        return Err(ScanTypographyFitError::QuantizedResidualOutOfRange {
            index,
            match_index,
            residual,
        });
    }
    let rounded = scaled.round_ties_even();
    const U64_EXCLUSIVE_UPPER_BOUND: f64 = 18_446_744_073_709_551_616.0;
    if rounded >= U64_EXCLUSIVE_UPPER_BOUND {
        return Err(ScanTypographyFitError::QuantizedResidualOutOfRange {
            index,
            match_index,
            residual,
        });
    }

    Ok(rounded as u64)
}

fn ineligibility_reason(
    report: &ScanComparisonReport,
    validated: &ValidatedTrial,
) -> Option<ScanTypographyIneligibilityReason> {
    if report.coverage.total_scan == 0 {
        Some(ScanTypographyIneligibilityReason::EmptyScanScope)
    } else if report.content_status != EvidenceStatus::Preserved {
        Some(ScanTypographyIneligibilityReason::ContentNotPreserved)
    } else if report.coverage.matched_scan != report.coverage.total_scan
        || report.coverage.matched_candidate != report.coverage.total_candidate
    {
        Some(ScanTypographyIneligibilityReason::PartialCoverage)
    } else if !report.unmatched_scan.is_empty() || !report.unmatched_candidate.is_empty() {
        Some(ScanTypographyIneligibilityReason::UnmatchedUnits)
    } else if validated.key.is_none() {
        Some(ScanTypographyIneligibilityReason::MissingHorizontalEvidence)
    } else {
        None
    }
}

fn select(trials: &[ScanTypographyTrialEvaluation]) -> ScanTypographySelection {
    let eligible = trials
        .iter()
        .filter(|trial| trial.eligibility == ScanTypographyEligibility::Eligible)
        .collect::<Vec<_>>();

    let Some(first) = eligible.first() else {
        return ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::NoEligibleTrial,
        };
    };
    let support = first
        .support
        .as_ref()
        .expect("eligible trials always have support after validation");
    if eligible
        .iter()
        .any(|trial| trial.support.as_ref() != Some(support))
    {
        return ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::IncomparableSupport,
        };
    }

    let evidence_scope = if support.known_baseline_scan_unit_ids.is_empty() {
        ScanTypographyEvidenceScope::HorizontalOnly
    } else {
        ScanTypographyEvidenceScope::HorizontalAndObservedBaseline
    };
    let minimum_key = eligible
        .iter()
        .map(|trial| {
            trial
                .key
                .as_ref()
                .expect("eligible trials always have a key after validation")
        })
        .min()
        .expect("eligible is non-empty");
    let indices = eligible
        .iter()
        .filter(|trial| trial.key.as_ref() == Some(minimum_key))
        .map(|trial| trial.index)
        .collect::<Vec<_>>();

    match indices.as_slice() {
        [index] => ScanTypographySelection::Selected {
            index: *index,
            evidence_scope,
        },
        _ => ScanTypographySelection::Tied {
            indices,
            evidence_scope,
        },
    }
}
