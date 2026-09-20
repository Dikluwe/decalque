//! Oracle-first contract for `scan-typst-line-reconstruction.md` and its
//! physical-line integrity behavior.
//! These tests exercise only public reconstruction behavior.

use decalque_core::{
    plan_scan_lines, Claim, Confidence, EvidenceBasis, FontStyleHypothesis, FontWeightHypothesis,
    FramedBbox, FramedPolyline, GeometryClaims, ObservationKind, ObservationUnit, PageFrame,
    PageMapping, PageMappingKind, ProducerIdentity, ProvenanceRecord, ProvenanceStage,
    RasterArtifact, RasterFrame, ReconstructionClaimKind, ReconstructionDiagnostic,
    ReconstructionInputError, ReconstructionOutcome, ScanObservation, ScanSource,
    TypographyHypothesis, UnicodeRange, UnknownReason,
};

fn unknown<T>() -> Claim<T> {
    Claim::Unknown {
        reason: UnknownReason::NotObserved,
        evidence: Vec::new(),
        detail: None,
    }
}

fn known<T>(value: T) -> Claim<T> {
    Claim::Known {
        value,
        basis: EvidenceBasis::Derived,
        evidence: vec!["prov".to_string()],
        confidence: Confidence::Unknown {
            reason: UnknownReason::NotObserved,
        },
    }
}

fn typography() -> TypographyHypothesis {
    TypographyHypothesis {
        font_family: "Libertinus Serif".to_string(),
        size_pt: 10.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Normal,
        tracking_pt: 0.0,
    }
}

fn bbox(x0: f64, y0: f64, x1: f64, y1: f64) -> Claim<FramedBbox> {
    known(FramedBbox {
        frame_id: "scan-px".to_string(),
        x0,
        y0,
        x1,
        y1,
    })
}

fn line(
    id: &str,
    parent_id: Option<&str>,
    order: Claim<u32>,
    text: Claim<String>,
    bbox: Claim<FramedBbox>,
    baseline: Claim<FramedPolyline>,
) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind: ObservationKind::Line,
        parent_id: parent_id.map(str::to_string),
        reading_order: order,
        text,
        span_in_parent: unknown::<UnicodeRange>(),
        geometry: GeometryClaims {
            bbox,
            polygon: unknown(),
            baseline,
        },
    }
}

fn region(id: &str, order: u32) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind: ObservationKind::Region,
        parent_id: None,
        reading_order: known(order),
        text: unknown(),
        span_in_parent: unknown(),
        geometry: GeometryClaims {
            bbox: unknown(),
            polygon: unknown(),
            baseline: unknown(),
        },
    }
}

fn mapping(matrix: [f64; 9], extent: (f64, f64)) -> Claim<PageMapping> {
    known(PageMapping {
        source_frame_id: "scan-px".to_string(),
        target_frame: PageFrame {
            id: "source-page-pt".to_string(),
            extent,
        },
        kind: PageMappingKind::Homography3x3,
        matrix,
        max_error_pt: 0.0,
    })
}

fn observation(page_mapping: Claim<PageMapping>, units: Vec<ObservationUnit>) -> ScanObservation {
    ScanObservation {
        source: ScanSource {
            page_index: 0,
            raster: RasterArtifact {
                artifact_id: "raster".to_string(),
                sha256: "0".repeat(64),
                media_type: "image/x-portable-graymap".to_string(),
                width_px: 1000,
                height_px: 2000,
            },
        },
        producer: ProducerIdentity {
            name: "oracle".to_string(),
            version: "1".to_string(),
            run_id: "run".to_string(),
        },
        raster_frame: RasterFrame {
            id: "scan-px".to_string(),
            extent: (1000, 2000),
        },
        page_mapping,
        provenance: vec![ProvenanceRecord {
            id: "prov".to_string(),
            stage: ProvenanceStage::ManualAnnotation,
            tool_name: "oracle".to_string(),
            tool_version: "1".to_string(),
            model_identifier: "none".to_string(),
            method: "fixture".to_string(),
            parameters_sha256: "1".repeat(64),
            input_artifact_ids: vec!["raster".to_string()],
            parent_provenance_ids: Vec::new(),
        }],
        units,
        diagnostics: Vec::new(),
    }
}

fn one_line(page_mapping: Claim<PageMapping>) -> ScanObservation {
    observation(
        page_mapping,
        vec![line(
            "line-1",
            None,
            known(0),
            known("linha".to_string()),
            bbox(100.0, 200.0, 500.0, 600.0),
            unknown(),
        )],
    )
}

fn materializable(outcome: ReconstructionOutcome) -> decalque_core::ReconstructionPlan {
    match outcome {
        ReconstructionOutcome::Materializable(plan) => plan,
        ReconstructionOutcome::Unknown(report) => {
            panic!("expected materializable plan, got {report:?}")
        }
    }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-9,
        "{actual} != {expected}"
    );
}

#[test]
fn affine_mapping_projects_page_and_line_bbox_to_points() {
    let scan = one_line(mapping(
        [0.5, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 1.0],
        (500.0, 500.0),
    ));

    let plan = materializable(plan_scan_lines(&scan, &typography()).unwrap());

    assert_eq!(plan.page.extent, (500.0, 500.0));
    assert_eq!(plan.coverage.planned_lines, 1);
    assert_eq!(plan.coverage.total_lines, 1);
    let target = &plan.lines[0].target_bbox;
    close(target.x0, 50.0);
    close(target.y0, 50.0);
    close(target.x1, 250.0);
    close(target.y1, 150.0);
}

#[test]
fn projective_mapping_uses_all_four_bbox_corners() {
    let scan = observation(
        mapping(
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.001, 1.0],
            (1000.0, 2000.0),
        ),
        vec![line(
            "projective",
            None,
            known(0),
            known("quatro cantos".to_string()),
            bbox(100.0, 100.0, 200.0, 300.0),
            unknown(),
        )],
    );

    let plan = materializable(plan_scan_lines(&scan, &typography()).unwrap());
    let target = &plan.lines[0].target_bbox;
    close(target.x0, 100.0 / 1.3);
    close(target.y0, 100.0 / 1.1);
    close(target.x1, 200.0 / 1.1);
    close(target.y1, 300.0 / 1.3);
}

#[test]
fn hierarchical_reading_key_not_json_order_controls_output_order() {
    let scan = observation(
        mapping(
            [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
            (500.0, 1000.0),
        ),
        vec![
            line(
                "right-line",
                Some("right-region"),
                known(0),
                known("direita".to_string()),
                bbox(600.0, 100.0, 800.0, 150.0),
                unknown(),
            ),
            region("right-region", 1),
            line(
                "left-line",
                Some("left-region"),
                known(0),
                known("esquerda".to_string()),
                bbox(100.0, 100.0, 300.0, 150.0),
                unknown(),
            ),
            region("left-region", 0),
        ],
    );

    let plan = materializable(plan_scan_lines(&scan, &typography()).unwrap());
    assert_eq!(plan.lines[0].source_unit_id, "left-line");
    assert_eq!(plan.lines[0].reading_key, vec![0, 0]);
    assert_eq!(plan.lines[1].source_unit_id, "right-line");
    assert_eq!(plan.lines[1].reading_key, vec![1, 0]);
}

#[test]
fn known_baseline_is_projected_and_unknown_baseline_is_not_invented() {
    let mut known_line = line(
        "known-baseline",
        None,
        known(0),
        known("uma".to_string()),
        bbox(100.0, 100.0, 300.0, 200.0),
        known(FramedPolyline {
            frame_id: "scan-px".to_string(),
            points: vec![(110.0, 180.0), (290.0, 175.0)],
        }),
    );
    let unknown_line = line(
        "unknown-baseline",
        None,
        known(1),
        known("duas".to_string()),
        bbox(100.0, 300.0, 300.0, 400.0),
        unknown(),
    );
    known_line.parent_id = None;
    let scan = observation(
        mapping(
            [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
            (500.0, 1000.0),
        ),
        vec![unknown_line, known_line],
    );

    let plan = materializable(plan_scan_lines(&scan, &typography()).unwrap());
    match &plan.lines[0].target_baseline {
        Claim::Known { value, .. } => {
            assert_eq!(value.frame_id, "source-page-pt");
            assert_eq!(value.points, vec![(55.0, 90.0), (145.0, 87.5)]);
        }
        Claim::Unknown { .. } => panic!("known source baseline became unknown"),
    }
    assert!(matches!(
        plan.lines[1].target_baseline,
        Claim::Unknown {
            reason: UnknownReason::NotObserved,
            ..
        }
    ));
}

#[test]
fn unknown_page_mapping_prevents_plan() {
    let scan = one_line(unknown());
    let outcome = plan_scan_lines(&scan, &typography()).unwrap();

    let ReconstructionOutcome::Unknown(report) = outcome else {
        panic!("unknown mapping emitted a plan");
    };
    assert_eq!(report.coverage.total_lines, 1);
    assert!(report
        .diagnostics
        .contains(&ReconstructionDiagnostic::PageMappingUnknown));
}

#[test]
fn unknown_required_line_claim_prevents_partial_plan() {
    let scan = observation(
        mapping(
            [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
            (500.0, 1000.0),
        ),
        vec![
            line(
                "known",
                None,
                known(0),
                known("visível".to_string()),
                bbox(10.0, 10.0, 100.0, 40.0),
                unknown(),
            ),
            line(
                "opaque",
                None,
                known(1),
                unknown(),
                bbox(10.0, 50.0, 100.0, 80.0),
                unknown(),
            ),
        ],
    );

    let outcome = plan_scan_lines(&scan, &typography()).unwrap();
    let ReconstructionOutcome::Unknown(report) = outcome else {
        panic!("partial page was emitted");
    };
    assert_eq!(report.coverage.planned_lines, 1);
    assert_eq!(report.coverage.total_lines, 2);
    assert!(report
        .diagnostics
        .contains(&ReconstructionDiagnostic::RequiredClaimUnknown {
            unit_id: "opaque".to_string(),
            claim: ReconstructionClaimKind::Text,
        }));
}

#[test]
fn empty_line_scope_is_unknown_not_empty_success() {
    let scan = observation(
        mapping(
            [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
            (500.0, 1000.0),
        ),
        vec![region("region-only", 0)],
    );

    let outcome = plan_scan_lines(&scan, &typography()).unwrap();
    let ReconstructionOutcome::Unknown(report) = outcome else {
        panic!("empty scope emitted an empty successful plan");
    };
    assert_eq!(report.coverage.planned_lines, 0);
    assert_eq!(report.coverage.total_lines, 0);
    assert!(report
        .diagnostics
        .contains(&ReconstructionDiagnostic::NoLines));
}

#[test]
fn invalid_typography_is_an_input_error_not_an_unknown_observation() {
    let scan = one_line(mapping(
        [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
        (500.0, 1000.0),
    ));
    let invalid = TypographyHypothesis {
        font_family: "  ".to_string(),
        ..typography()
    };

    assert_eq!(
        plan_scan_lines(&scan, &invalid),
        Err(ReconstructionInputError::EmptyFontFamily)
    );
}

#[test]
fn explicit_physical_line_separators_are_specific_unknowns() {
    let separators = [
        ("LF", '\n'),
        ("CR", '\r'),
        ("VT", '\u{000b}'),
        ("FF", '\u{000c}'),
        ("NEL", '\u{0085}'),
        ("LINE SEPARATOR", '\u{2028}'),
        ("PARAGRAPH SEPARATOR", '\u{2029}'),
    ];

    for (name, separator) in separators {
        let unit_id = format!("line-{name}");
        let scan = observation(
            mapping(
                [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
                (500.0, 1000.0),
            ),
            vec![line(
                &unit_id,
                None,
                known(0),
                known(format!("a{separator}b")),
                bbox(10.0, 10.0, 100.0, 40.0),
                unknown(),
            )],
        );

        let outcome = plan_scan_lines(&scan, &typography()).unwrap();
        let ReconstructionOutcome::Unknown(report) = outcome else {
            panic!("{name} materialized a contradictory physical line");
        };
        assert_eq!(report.coverage.planned_lines, 0, "{name}");
        assert_eq!(report.coverage.total_lines, 1, "{name}");
        assert_eq!(
            report.diagnostics,
            vec![ReconstructionDiagnostic::ExplicitLineSeparator {
                unit_id,
                scalar_index: 1,
                code_point: u32::from(separator),
            }],
            "{name}"
        );
    }
}
