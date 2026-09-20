//! Oraculos L1 independentes para a busca discreta de corpo e tracking.
//!
//! API publica minima esperada em `decalque_core::engine`:
//! - `evaluate_scan_typography_fit(&[ScanComparisonReport])`;
//! - uma avaliacao com `trials` (indice, elegibilidade, suporte e chave) e `selection`;
//! - os enums importados abaixo para inelegibilidade, inconclusao, escopo e selecao.
//!
//! O avaliador recebe somente relatorios comparativos. Portanto indice, hipotese
//! tipografica e ordem de entrada nao podem participar da chave geometrica.

use decalque_core::engine::{
    evaluate_scan_typography_fit, CandidateUnitRef, EvidenceStatus, ScanComparisonReport,
    ScanCoverage, ScanMatch, ScanTypographyEligibility, ScanTypographyEvidenceScope,
    ScanTypographyInconclusiveReason, ScanTypographyIneligibilityReason, ScanTypographySelection,
};

fn matched_line(
    id: &str,
    horizontal_deltas: [Option<f64>; 3],
    horizontal_status: EvidenceStatus,
    baseline_delta: Option<f64>,
    baseline_status: EvidenceStatus,
) -> ScanMatch {
    ScanMatch {
        scan_unit_id: id.to_string(),
        candidate_line_index: 0,
        candidate_scalar_range: (0, 1),
        dx_start: horizontal_deltas[0],
        dx_end: horizontal_deltas[1],
        width_delta: horizontal_deltas[2],
        baseline_delta,
        horizontal_status,
        baseline_status,
    }
}

fn horizontal_line(id: &str, residual: f64, status: EvidenceStatus) -> ScanMatch {
    matched_line(
        id,
        [Some(residual), Some(residual), Some(residual)],
        status,
        None,
        EvidenceStatus::Unknown,
    )
}

fn observed_baseline_line(
    id: &str,
    horizontal_residual: f64,
    horizontal_status: EvidenceStatus,
    baseline_delta: f64,
    baseline_status: EvidenceStatus,
) -> ScanMatch {
    matched_line(
        id,
        [
            Some(horizontal_residual),
            Some(horizontal_residual),
            Some(horizontal_residual),
        ],
        horizontal_status,
        Some(baseline_delta),
        baseline_status,
    )
}

fn report(mut matches: Vec<ScanMatch>) -> ScanComparisonReport {
    for (index, matched) in matches.iter_mut().enumerate() {
        matched.candidate_line_index = index;
    }

    let geometry_status = if matches.iter().any(|matched| {
        matched.horizontal_status == EvidenceStatus::Violated
            || matched.baseline_status == EvidenceStatus::Violated
    }) {
        EvidenceStatus::Violated
    } else if !matches.is_empty()
        && matches.iter().all(|matched| {
            matched.horizontal_status == EvidenceStatus::Preserved
                && matched.baseline_status == EvidenceStatus::Preserved
        })
    {
        EvidenceStatus::Preserved
    } else {
        EvidenceStatus::Unknown
    };
    let count = matches.len();

    ScanComparisonReport {
        content_status: EvidenceStatus::Preserved,
        geometry_status,
        overall_status: geometry_status,
        coverage: ScanCoverage {
            matched_scan: count,
            total_scan: count,
            matched_candidate: count,
            total_candidate: count,
        },
        matches,
        unmatched_scan: Vec::new(),
        unmatched_candidate: Vec::new(),
        reflow: Vec::new(),
        diagnostics: Vec::new(),
    }
}

#[test]
fn none_never_becomes_zero_and_only_an_unknown_baseline_is_absent() {
    let missing_horizontal = report(vec![matched_line(
        "line-1",
        [None, None, None],
        EvidenceStatus::Unknown,
        None,
        EvidenceStatus::Unknown,
    )]);

    let alone = evaluate_scan_typography_fit(std::slice::from_ref(&missing_horizontal)).unwrap();
    assert_eq!(
        alone.trials[0].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::MissingHorizontalEvidence,
        )
    );
    assert_eq!(
        alone.selection,
        ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::NoEligibleTrial,
        }
    );

    let measured_horizontal = report(vec![horizontal_line(
        "line-1",
        1.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    let evaluation =
        evaluate_scan_typography_fit(&[missing_horizontal, measured_horizontal]).unwrap();

    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 1,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
    let key = evaluation.trials[1].key.as_ref().unwrap();
    assert_eq!(key.horizontal_residuals_desc, vec![1]);
    assert!(key.baseline_residuals_desc.is_empty());
}

#[test]
fn partial_coverage_with_zero_residual_never_competes_with_full_coverage() {
    let mut partial = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    partial.coverage.total_scan = 2;
    partial.unmatched_scan.push("line-2".to_string());

    let complete = report(vec![horizontal_line(
        "line-1",
        1.0,
        EvidenceStatus::Preserved,
    )]);
    let evaluation = evaluate_scan_typography_fit(&[partial, complete]).unwrap();

    assert!(matches!(
        &evaluation.trials[0].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::PartialCoverage
                | ScanTypographyIneligibilityReason::UnmatchedUnits
        )
    ));
    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 1,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}

#[test]
fn different_support_is_inconclusive_while_match_order_is_irrelevant() {
    let first = report(vec![
        horizontal_line("line-a", 0.0, EvidenceStatus::Preserved),
        horizontal_line("line-b", 1.0, EvidenceStatus::Preserved),
    ]);
    let reordered = report(vec![
        horizontal_line("line-b", 1.0, EvidenceStatus::Preserved),
        horizontal_line("line-a", 0.0, EvidenceStatus::Preserved),
    ]);

    let comparable = evaluate_scan_typography_fit(&[first.clone(), reordered]).unwrap();
    assert_eq!(
        comparable.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
    for trial in &comparable.trials {
        let support = trial.support.as_ref().unwrap();
        assert_eq!(
            support.horizontal_scan_unit_ids,
            vec!["line-a".to_string(), "line-b".to_string()]
        );
        assert!(support.known_baseline_scan_unit_ids.is_empty());
    }

    let different_ids = report(vec![
        horizontal_line("line-a", 0.0, EvidenceStatus::Preserved),
        horizontal_line("line-c", 0.0, EvidenceStatus::Preserved),
    ]);
    let incomparable = evaluate_scan_typography_fit(&[first, different_ids]).unwrap();
    assert_eq!(
        incomparable.selection,
        ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::IncomparableSupport,
        }
    );
}

#[test]
fn quantization_uses_exact_1024th_points_and_ties_to_even() {
    let zero = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    let half_unit = report(vec![horizontal_line(
        "line-1",
        0.5 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    let one_unit = report(vec![horizontal_line(
        "line-1",
        1.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);

    let lower_tie = evaluate_scan_typography_fit(&[zero, half_unit, one_unit]).unwrap();
    assert_eq!(
        lower_tie.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
    assert_eq!(
        lower_tie.trials[0]
            .key
            .as_ref()
            .unwrap()
            .horizontal_residuals_desc,
        vec![0]
    );
    assert_eq!(
        lower_tie.trials[1]
            .key
            .as_ref()
            .unwrap()
            .horizontal_residuals_desc,
        vec![0]
    );
    assert_eq!(
        lower_tie.trials[2]
            .key
            .as_ref()
            .unwrap()
            .horizontal_residuals_desc,
        vec![1]
    );

    let two_units = report(vec![horizontal_line(
        "line-1",
        2.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    let two_and_a_half_units = report(vec![horizontal_line(
        "line-1",
        2.5 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    let upper_tie = evaluate_scan_typography_fit(&[two_units, two_and_a_half_units]).unwrap();
    assert_eq!(
        upper_tie.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
    for trial in upper_tie.trials {
        assert_eq!(trial.key.unwrap().horizontal_residuals_desc, vec![2]);
    }
}

#[test]
fn violations_then_worst_residual_win_without_sum_or_order_bias() {
    let one_large_violation = report(vec![
        horizontal_line("line-a", 100.0, EvidenceStatus::Violated),
        horizontal_line("line-b", 0.0, EvidenceStatus::Preserved),
    ]);
    let two_small_violations = report(vec![
        horizontal_line("line-a", 2.0, EvidenceStatus::Violated),
        horizontal_line("line-b", 2.0, EvidenceStatus::Violated),
    ]);
    let violation_priority =
        evaluate_scan_typography_fit(&[one_large_violation, two_small_violations]).unwrap();
    assert_eq!(
        violation_priority.selection,
        ScanTypographySelection::Selected {
            index: 0,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );

    let lower_sum_but_worse_maximum = report(vec![
        horizontal_line("line-a", 6.0, EvidenceStatus::Preserved),
        horizontal_line("line-b", 0.0, EvidenceStatus::Preserved),
    ]);
    let higher_sum_but_better_maximum = report(vec![
        horizontal_line("line-a", 4.0, EvidenceStatus::Preserved),
        horizontal_line("line-b", 4.0, EvidenceStatus::Preserved),
    ]);
    let original = evaluate_scan_typography_fit(&[
        lower_sum_but_worse_maximum.clone(),
        higher_sum_but_better_maximum.clone(),
    ])
    .unwrap();
    assert_eq!(
        original.selection,
        ScanTypographySelection::Selected {
            index: 1,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );

    let mut first_reordered = lower_sum_but_worse_maximum;
    first_reordered.matches.reverse();
    let mut second_reordered = higher_sum_but_better_maximum;
    second_reordered.matches.reverse();
    let reordered = evaluate_scan_typography_fit(&[first_reordered, second_reordered]).unwrap();

    assert_eq!(reordered.selection, original.selection);
    assert_eq!(reordered.trials[0].key, original.trials[0].key);
    assert_eq!(reordered.trials[1].key, original.trials[1].key);
}

#[test]
fn an_integral_tie_returns_every_index_without_a_tie_break() {
    let identical = report(vec![horizontal_line(
        "line-1",
        3.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    let evaluation =
        evaluate_scan_typography_fit(&[identical.clone(), identical.clone(), identical]).unwrap();

    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1, 2],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}

#[test]
fn wholly_unknown_baselines_produce_horizontal_only_scope() {
    let evaluation = evaluate_scan_typography_fit(&[report(vec![
        horizontal_line("line-a", 1.0, EvidenceStatus::Preserved),
        horizontal_line("line-b", 2.0, EvidenceStatus::Preserved),
    ])])
    .unwrap();

    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 0,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
    let trial = &evaluation.trials[0];
    let support = trial.support.as_ref().unwrap();
    let key = trial.key.as_ref().unwrap();
    assert!(support.known_baseline_scan_unit_ids.is_empty());
    assert_eq!(key.baseline_violations, 0);
    assert!(key.baseline_residuals_desc.is_empty());
}

#[test]
fn non_finite_partial_or_contradictory_evidence_is_rejected() {
    let invalid = vec![
        (
            "NaN horizontal",
            report(vec![horizontal_line(
                "line-1",
                f64::NAN,
                EvidenceStatus::Preserved,
            )]),
        ),
        (
            "infinite horizontal",
            report(vec![horizontal_line(
                "line-1",
                f64::INFINITY,
                EvidenceStatus::Violated,
            )]),
        ),
        (
            "partial horizontal delta",
            report(vec![matched_line(
                "line-1",
                [Some(0.0), None, Some(0.0)],
                EvidenceStatus::Unknown,
                None,
                EvidenceStatus::Unknown,
            )]),
        ),
        (
            "unknown baseline with a delta",
            report(vec![matched_line(
                "line-1",
                [Some(0.0), Some(0.0), Some(0.0)],
                EvidenceStatus::Preserved,
                Some(0.0),
                EvidenceStatus::Unknown,
            )]),
        ),
        (
            "known baseline without a delta",
            report(vec![matched_line(
                "line-1",
                [Some(0.0), Some(0.0), Some(0.0)],
                EvidenceStatus::Preserved,
                None,
                EvidenceStatus::Preserved,
            )]),
        ),
        (
            "known horizontal state without deltas",
            report(vec![matched_line(
                "line-1",
                [None, None, None],
                EvidenceStatus::Preserved,
                None,
                EvidenceStatus::Unknown,
            )]),
        ),
    ];

    for (case, invalid_report) in invalid {
        assert!(
            evaluate_scan_typography_fit(&[invalid_report]).is_err(),
            "{case} must be rejected as invalid evidence"
        );
    }
}

#[test]
fn observed_baselines_participate_only_after_the_complete_horizontal_key() {
    let winner = report(vec![
        observed_baseline_line(
            "line-a",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            -8.0 / 1024.0,
            EvidenceStatus::Violated,
        ),
        observed_baseline_line(
            "line-b",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            3.0 / 1024.0,
            EvidenceStatus::Preserved,
        ),
    ]);
    let better_baseline_but_worse_horizontal = report(vec![
        observed_baseline_line(
            "line-a",
            2.0 / 1024.0,
            EvidenceStatus::Preserved,
            0.0,
            EvidenceStatus::Preserved,
        ),
        observed_baseline_line(
            "line-b",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            0.0,
            EvidenceStatus::Preserved,
        ),
    ]);
    let more_baseline_violations = report(vec![
        observed_baseline_line(
            "line-a",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            1.0 / 1024.0,
            EvidenceStatus::Violated,
        ),
        observed_baseline_line(
            "line-b",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            1.0 / 1024.0,
            EvidenceStatus::Violated,
        ),
    ]);
    let worse_baseline_tail = report(vec![
        observed_baseline_line(
            "line-a",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            8.0 / 1024.0,
            EvidenceStatus::Violated,
        ),
        observed_baseline_line(
            "line-b",
            1.0 / 1024.0,
            EvidenceStatus::Preserved,
            4.0 / 1024.0,
            EvidenceStatus::Preserved,
        ),
    ]);

    let evaluation = evaluate_scan_typography_fit(&[
        winner,
        better_baseline_but_worse_horizontal,
        more_baseline_violations,
        worse_baseline_tail,
    ])
    .unwrap();

    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 0,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalAndObservedBaseline,
        }
    );
    let support = evaluation.trials[0].support.as_ref().unwrap();
    assert_eq!(
        support.known_baseline_scan_unit_ids,
        vec!["line-a".to_string(), "line-b".to_string()]
    );
    let key = evaluation.trials[0].key.as_ref().unwrap();
    assert_eq!(key.horizontal_violations, 0);
    assert_eq!(key.horizontal_residuals_desc, vec![1, 1]);
    assert_eq!(key.baseline_violations, 1);
    assert_eq!(key.baseline_residuals_desc, vec![8, 3]);
}

#[test]
fn different_known_baseline_support_is_incomparable() {
    let baseline_on_a = report(vec![
        observed_baseline_line(
            "line-a",
            0.0,
            EvidenceStatus::Preserved,
            0.0,
            EvidenceStatus::Preserved,
        ),
        horizontal_line("line-b", 0.0, EvidenceStatus::Preserved),
    ]);
    let baseline_on_b = report(vec![
        horizontal_line("line-a", 0.0, EvidenceStatus::Preserved),
        observed_baseline_line(
            "line-b",
            0.0,
            EvidenceStatus::Preserved,
            0.0,
            EvidenceStatus::Preserved,
        ),
    ]);

    let evaluation = evaluate_scan_typography_fit(&[baseline_on_a, baseline_on_b]).unwrap();

    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::IncomparableSupport,
        }
    );
    assert_eq!(
        evaluation.trials[0]
            .support
            .as_ref()
            .unwrap()
            .known_baseline_scan_unit_ids,
        vec!["line-a".to_string()]
    );
    assert_eq!(
        evaluation.trials[1]
            .support
            .as_ref()
            .unwrap()
            .known_baseline_scan_unit_ids,
        vec!["line-b".to_string()]
    );
}

#[test]
fn horizontal_residuals_use_absolute_component_maxima_and_vector_tail() {
    let better_tail = report(vec![
        matched_line(
            "line-a",
            [
                Some(-4.0 / 1024.0),
                Some(-1.0 / 1024.0),
                Some(-2.0 / 1024.0),
            ],
            EvidenceStatus::Preserved,
            None,
            EvidenceStatus::Unknown,
        ),
        matched_line(
            "line-b",
            [
                Some(-1.0 / 1024.0),
                Some(-1.0 / 1024.0),
                Some(-1.0 / 1024.0),
            ],
            EvidenceStatus::Preserved,
            None,
            EvidenceStatus::Unknown,
        ),
    ]);
    let worse_tail = report(vec![
        matched_line(
            "line-a",
            [Some(4.0 / 1024.0), Some(0.0), Some(0.0)],
            EvidenceStatus::Preserved,
            None,
            EvidenceStatus::Unknown,
        ),
        matched_line(
            "line-b",
            [Some(-2.0 / 1024.0), Some(0.0), Some(0.0)],
            EvidenceStatus::Preserved,
            None,
            EvidenceStatus::Unknown,
        ),
    ]);

    let evaluation = evaluate_scan_typography_fit(&[better_tail, worse_tail]).unwrap();

    assert_eq!(
        evaluation.trials[0]
            .key
            .as_ref()
            .unwrap()
            .horizontal_residuals_desc,
        vec![4, 1]
    );
    assert_eq!(
        evaluation.trials[1]
            .key
            .as_ref()
            .unwrap()
            .horizontal_residuals_desc,
        vec![4, 2]
    );
    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 0,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}

#[test]
fn non_preserved_content_and_unmatched_units_are_ineligible() {
    let mut unknown_content = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    unknown_content.content_status = EvidenceStatus::Unknown;

    let mut violated_content = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    violated_content.content_status = EvidenceStatus::Violated;

    let mut unmatched = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    unmatched.coverage.total_scan = 2;
    unmatched.unmatched_scan.push("line-2".to_string());

    let eligible = report(vec![horizontal_line(
        "line-1",
        1.0,
        EvidenceStatus::Preserved,
    )]);
    let evaluation =
        evaluate_scan_typography_fit(&[unknown_content, violated_content, unmatched, eligible])
            .unwrap();

    assert_eq!(
        evaluation.trials[0].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::ContentNotPreserved,
        )
    );
    assert_eq!(
        evaluation.trials[1].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::ContentNotPreserved,
        )
    );
    assert!(matches!(
        &evaluation.trials[2].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::PartialCoverage
                | ScanTypographyIneligibilityReason::UnmatchedUnits
        )
    ));
    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 3,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}

#[test]
fn incoherent_counters_and_empty_or_duplicate_ids_are_evidence_errors() {
    let mut wrong_matched_scan = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    wrong_matched_scan.coverage.matched_scan = 0;

    let mut wrong_matched_candidate = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    wrong_matched_candidate.coverage.matched_candidate = 0;

    let mut missing_unmatched_scan = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    missing_unmatched_scan.coverage.total_scan = 2;

    let mut unexpected_unmatched_scan = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    unexpected_unmatched_scan
        .unmatched_scan
        .push("line-2".to_string());

    let invalid = vec![
        ("matched_scan disagrees with matches", wrong_matched_scan),
        (
            "matched_candidate disagrees with matches",
            wrong_matched_candidate,
        ),
        (
            "total_scan omits a required unmatched id",
            missing_unmatched_scan,
        ),
        (
            "unmatched id is absent from the coverage total",
            unexpected_unmatched_scan,
        ),
        (
            "empty scan_unit_id",
            report(vec![horizontal_line("", 0.0, EvidenceStatus::Preserved)]),
        ),
        (
            "duplicate scan_unit_id",
            report(vec![
                horizontal_line("line-1", 0.0, EvidenceStatus::Preserved),
                horizontal_line("line-1", 0.0, EvidenceStatus::Preserved),
            ]),
        ),
    ];

    for (case, invalid_report) in invalid {
        assert!(
            evaluate_scan_typography_fit(&[invalid_report]).is_err(),
            "{case} must be rejected as invalid evidence"
        );
    }
}

#[test]
fn non_finite_baselines_and_finite_quantization_overflow_are_rejected() {
    let quantized_above_u64_max = 2.0_f64.powi(55);
    assert!(quantized_above_u64_max.is_finite());
    assert!(quantized_above_u64_max * 1024.0 > u64::MAX as f64);

    let invalid = vec![
        (
            "NaN baseline",
            report(vec![observed_baseline_line(
                "line-1",
                0.0,
                EvidenceStatus::Preserved,
                f64::NAN,
                EvidenceStatus::Preserved,
            )]),
        ),
        (
            "infinite baseline",
            report(vec![observed_baseline_line(
                "line-1",
                0.0,
                EvidenceStatus::Preserved,
                f64::NEG_INFINITY,
                EvidenceStatus::Violated,
            )]),
        ),
        (
            "finite horizontal quantization overflow",
            report(vec![horizontal_line(
                "line-1",
                quantized_above_u64_max,
                EvidenceStatus::Violated,
            )]),
        ),
        (
            "finite baseline quantization overflow",
            report(vec![observed_baseline_line(
                "line-1",
                0.0,
                EvidenceStatus::Preserved,
                quantized_above_u64_max,
                EvidenceStatus::Violated,
            )]),
        ),
    ];

    for (case, invalid_report) in invalid {
        assert!(
            evaluate_scan_typography_fit(&[invalid_report]).is_err(),
            "{case} must be rejected as invalid evidence"
        );
    }
}

#[test]
fn empty_input_and_a_coherent_empty_report_have_no_eligible_trial() {
    let no_reports = evaluate_scan_typography_fit(&[]).unwrap();
    assert!(no_reports.trials.is_empty());
    assert_eq!(
        no_reports.selection,
        ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::NoEligibleTrial,
        }
    );

    let empty_report = evaluate_scan_typography_fit(&[report(Vec::new())]).unwrap();
    assert_eq!(
        empty_report.trials[0].eligibility,
        ScanTypographyEligibility::Ineligible(ScanTypographyIneligibilityReason::EmptyScanScope,)
    );
    assert_eq!(
        empty_report.selection,
        ScanTypographySelection::Inconclusive {
            reason: ScanTypographyInconclusiveReason::NoEligibleTrial,
        }
    );
}

#[test]
fn finite_horizontal_deltas_with_unknown_status_do_not_compete_as_zero() {
    let unknown = report(vec![matched_line(
        "line-1",
        [Some(0.0), Some(0.0), Some(0.0)],
        EvidenceStatus::Unknown,
        None,
        EvidenceStatus::Unknown,
    )]);
    let measured = report(vec![horizontal_line(
        "line-1",
        1.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);

    let evaluation = evaluate_scan_typography_fit(&[unknown, measured]).unwrap();

    assert_eq!(
        evaluation.trials[0].eligibility,
        ScanTypographyEligibility::Ineligible(
            ScanTypographyIneligibilityReason::MissingHorizontalEvidence,
        )
    );
    assert!(evaluation.trials[0].key.is_none());
    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Selected {
            index: 1,
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}

#[test]
fn total_candidate_and_unmatched_candidate_must_agree_in_both_directions() {
    let mut total_claims_an_unmatched_candidate = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    total_claims_an_unmatched_candidate.coverage.total_candidate = 2;

    let mut list_claims_an_unmatched_candidate = report(vec![horizontal_line(
        "line-1",
        0.0,
        EvidenceStatus::Preserved,
    )]);
    list_claims_an_unmatched_candidate
        .unmatched_candidate
        .push(CandidateUnitRef {
            candidate_line_index: 1,
            candidate_scalar_range: (1, 2),
        });

    for invalid_report in [
        total_claims_an_unmatched_candidate,
        list_claims_an_unmatched_candidate,
    ] {
        assert!(evaluate_scan_typography_fit(&[invalid_report]).is_err());
    }
}

#[test]
fn baseline_quantization_ties_to_even_at_half_and_two_and_a_half_units() {
    let baseline = |units: f64| {
        report(vec![observed_baseline_line(
            "line-1",
            0.0,
            EvidenceStatus::Preserved,
            units / 1024.0,
            EvidenceStatus::Preserved,
        )])
    };

    let lower_tie = evaluate_scan_typography_fit(&[baseline(0.0), baseline(0.5)]).unwrap();
    assert_eq!(
        lower_tie.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalAndObservedBaseline,
        }
    );
    for trial in lower_tie.trials {
        assert_eq!(trial.key.unwrap().baseline_residuals_desc, vec![0]);
    }

    let upper_tie = evaluate_scan_typography_fit(&[baseline(2.0), baseline(2.5)]).unwrap();
    assert_eq!(
        upper_tie.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalAndObservedBaseline,
        }
    );
    for trial in upper_tie.trials {
        assert_eq!(trial.key.unwrap().baseline_residuals_desc, vec![2]);
    }
}

#[test]
fn overall_status_does_not_participate_in_an_identical_key() {
    let mut preserved_overall = report(vec![horizontal_line(
        "line-1",
        1.0 / 1024.0,
        EvidenceStatus::Preserved,
    )]);
    preserved_overall.overall_status = EvidenceStatus::Preserved;

    let mut violated_overall = preserved_overall.clone();
    violated_overall.overall_status = EvidenceStatus::Violated;

    let evaluation = evaluate_scan_typography_fit(&[preserved_overall, violated_overall]).unwrap();

    assert_eq!(evaluation.trials[0].key, evaluation.trials[1].key);
    assert_eq!(
        evaluation.selection,
        ScanTypographySelection::Tied {
            indices: vec![0, 1],
            evidence_scope: ScanTypographyEvidenceScope::HorizontalOnly,
        }
    );
}
