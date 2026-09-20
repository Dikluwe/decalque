//! Oraculos L1 independentes para a comparacao scan -> PDF digital.
//!
//! A API exigida aqui e deliberadamente pequena: tipos de politica/relatorio e
//! uma funcao pura que recebe referencias imutaveis para as duas entradas.

use decalque_core::engine::{
    compare_scan_observation, ConfidenceRequirement, EvidenceStatus, ScanComparisonDiagnostic,
    ScanComparisonPolicy, ScanGranularity, TextNormalization,
};
use decalque_core::entities::{
    Claim, Confidence, DocumentGeometry, EvidenceBasis, FramedBbox, FramedPolyline, GeometryClaims,
    GlyphInstance, ObservationKind, ObservationUnit, PageFrame, PageGeometry, PageMapping,
    PageMappingKind, PageRotation, ProducerIdentity, ProvenanceRecord, ProvenanceStage,
    RasterArtifact, RasterFrame, ScanObservation, ScanSource, TextMappingStatus, UnicodeRange,
    UnknownReason,
};

const RASTER_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PARAMETERS_SHA256: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn confidence(value: f64) -> Confidence {
    Confidence::Known {
        value,
        semantics: "contract-fixture".to_string(),
    }
}

fn known<T>(value: T, basis: EvidenceBasis, evidence: &str) -> Claim<T> {
    Claim::Known {
        value,
        basis,
        evidence: vec![evidence.to_string()],
        confidence: confidence(1.0),
    }
}

fn unknown<T>(reason: UnknownReason) -> Claim<T> {
    Claim::Unknown {
        reason,
        evidence: Vec::new(),
        detail: Some("intentionally opaque contract fixture".to_string()),
    }
}

fn provenance(
    id: &str,
    stage: ProvenanceStage,
    input_artifact_ids: &[&str],
    parent_provenance_ids: &[&str],
) -> ProvenanceRecord {
    ProvenanceRecord {
        id: id.to_string(),
        stage,
        tool_name: "contract-fixture".to_string(),
        tool_version: "1".to_string(),
        model_identifier: "fixture-model".to_string(),
        method: "independent-contract-oracle".to_string(),
        parameters_sha256: PARAMETERS_SHA256.to_string(),
        input_artifact_ids: input_artifact_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        parent_provenance_ids: parent_provenance_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
    }
}

fn mapping(matrix: [f64; 9]) -> Claim<PageMapping> {
    known(
        PageMapping {
            source_frame_id: "scan-px".to_string(),
            target_frame: PageFrame {
                id: "source-page-pt".to_string(),
                extent: (1000.0, 2000.0),
            },
            kind: PageMappingKind::Homography3x3,
            matrix,
            max_error_pt: 0.0,
        },
        EvidenceBasis::Derived,
        "prov-map",
    )
}

fn identity_mapping() -> Claim<PageMapping> {
    mapping([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
}

fn observation(page_mapping: Claim<PageMapping>, units: Vec<ObservationUnit>) -> ScanObservation {
    ScanObservation {
        source: ScanSource {
            page_index: 0,
            raster: RasterArtifact {
                artifact_id: "raster-0".to_string(),
                sha256: RASTER_SHA256.to_string(),
                media_type: "image/png".to_string(),
                width_px: 1000,
                height_px: 2000,
            },
        },
        producer: ProducerIdentity {
            name: "contract-fixture".to_string(),
            version: "1".to_string(),
            run_id: "run-1".to_string(),
        },
        raster_frame: RasterFrame {
            id: "scan-px".to_string(),
            extent: (1000, 2000),
        },
        page_mapping,
        provenance: vec![
            provenance(
                "prov-layout",
                ProvenanceStage::LayoutDetection,
                &["raster-0"],
                &[],
            ),
            provenance(
                "prov-text",
                ProvenanceStage::TextRecognition,
                &["raster-0"],
                &["prov-layout"],
            ),
            provenance(
                "prov-segment",
                ProvenanceStage::Segmentation,
                &[],
                &["prov-text"],
            ),
            provenance(
                "prov-map",
                ProvenanceStage::CoordinateTransform,
                &["raster-0"],
                &[],
            ),
        ],
        units,
        diagnostics: Vec::new(),
    }
}

fn geometry(x0: f64, y0: f64, x1: f64, y1: f64, baseline_y: f64) -> GeometryClaims {
    GeometryClaims {
        bbox: known(
            FramedBbox {
                frame_id: "scan-px".to_string(),
                x0,
                y0,
                x1,
                y1,
            },
            EvidenceBasis::Observed,
            "prov-layout",
        ),
        polygon: unknown(UnknownReason::NotObserved),
        baseline: known(
            FramedPolyline {
                frame_id: "scan-px".to_string(),
                points: vec![(x0, baseline_y), (x1, baseline_y)],
            },
            EvidenceBasis::Observed,
            "prov-layout",
        ),
    }
}

fn unknown_geometry() -> GeometryClaims {
    GeometryClaims {
        bbox: unknown(UnknownReason::NotObserved),
        polygon: unknown(UnknownReason::NotObserved),
        baseline: unknown(UnknownReason::NotObserved),
    }
}

fn scan_line(
    id: &str,
    text: Claim<String>,
    order: u32,
    geometry: GeometryClaims,
) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind: ObservationKind::Line,
        parent_id: None,
        reading_order: known(order, EvidenceBasis::Inferred, "prov-layout"),
        text,
        span_in_parent: unknown(UnknownReason::NotObserved),
        geometry,
    }
}

fn scan_word(
    id: &str,
    parent_id: &str,
    text: &str,
    start: usize,
    end: usize,
    order: u32,
    geometry: GeometryClaims,
) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind: ObservationKind::Word,
        parent_id: Some(parent_id.to_string()),
        reading_order: known(order, EvidenceBasis::Derived, "prov-segment"),
        text: known(text.to_string(), EvidenceBasis::Derived, "prov-segment"),
        span_in_parent: known(
            UnicodeRange { start, end },
            EvidenceBasis::Derived,
            "prov-segment",
        ),
        geometry,
    }
}

fn glyph(codepoints: Option<Vec<char>>, x: f64, y: f64, advance: f64) -> GlyphInstance {
    let mapping_status = if codepoints.is_some() {
        TextMappingStatus::Mapped
    } else {
        TextMappingStatus::Unmapped
    };
    GlyphInstance {
        glyph_code: 1,
        codepoints,
        position: (x, y),
        advance,
        font_ref: "F1".to_string(),
        font_size_pt: 10.0,
        mapping_status,
        render_mode: 0,
    }
}

fn candidate(lines: &[(&str, f64, f64)]) -> DocumentGeometry {
    let mut glyphs = Vec::new();
    for (text, x0, y) in lines {
        for (index, scalar) in text.chars().enumerate() {
            glyphs.push(glyph(
                Some(vec![scalar]),
                *x0 + index as f64 * 10.0,
                *y,
                10.0,
            ));
        }
    }
    document(glyphs, PageRotation::Deg0, 1.0)
}

fn document(
    glyphs: Vec<GlyphInstance>,
    rotation: PageRotation,
    user_unit: f64,
) -> DocumentGeometry {
    DocumentGeometry {
        page: PageGeometry {
            width: 500.0,
            height: 1000.0,
            origin: (0.0, 0.0),
            rotation,
            user_unit,
        },
        glyphs,
        diagnostics: Vec::new(),
    }
}

fn policy(granularity: ScanGranularity, horizontal: f64, baseline: f64) -> ScanComparisonPolicy {
    ScanComparisonPolicy {
        granularity,
        horizontal_tolerance_pt: horizontal,
        baseline_tolerance_pt: baseline,
        text_confidence: ConfidenceRequirement::Any,
        geometry_confidence: ConfidenceRequirement::Any,
        text_normalization: TextNormalization::Exact,
    }
}

#[test]
fn exact_unique_line_and_half_scale_homography_are_preserved() {
    let scan = observation(
        mapping([0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0]),
        vec![scan_line(
            "line-1",
            known(
                "abcdefghij".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            0,
            geometry(100.0, 200.0, 300.0, 240.0, 220.0),
        )],
    );
    let pdf = candidate(&[("abcdefghij", 50.0, 110.0)]);
    let comparison_policy = policy(ScanGranularity::Line, 0.0, 0.0);

    let report = compare_scan_observation(&scan, &pdf, &comparison_policy);

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.geometry_status, EvidenceStatus::Preserved);
    assert_eq!(report.overall_status, EvidenceStatus::Preserved);
    assert_eq!(report.coverage.matched_scan, 1);
    assert_eq!(report.coverage.total_scan, 1);
    assert_eq!(report.coverage.matched_candidate, 1);
    assert_eq!(report.coverage.total_candidate, 1);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].dx_start, Some(0.0));
    assert_eq!(report.matches[0].dx_end, Some(0.0));
    assert_eq!(report.matches[0].width_delta, Some(0.0));
    assert_eq!(report.matches[0].baseline_delta, Some(0.0));

    let repeated = compare_scan_observation(&scan, &pdf, &comparison_policy);
    assert_eq!(report, repeated, "the comparison must be deterministic");
}

#[test]
fn projective_bbox_uses_all_four_corners() {
    let projective = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.001, 1.0];
    let scan = observation(
        mapping(projective),
        vec![scan_line(
            "line-projective",
            known(
                "abcdefghij".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            0,
            geometry(100.0, 200.0, 300.0, 240.0, 220.0),
        )],
    );
    let projected_x0 = 100.0 / 1.24;
    let projected_x1 = 300.0 / 1.20;
    let projected_baseline = 220.0 / 1.22;
    let pdf = document(
        vec![glyph(
            Some("abcdefghij".chars().collect()),
            projected_x0,
            projected_baseline,
            projected_x1 - projected_x0,
        )],
        PageRotation::Deg0,
        1.0,
    );

    let report =
        compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 1.0e-9, 1.0e-9));

    assert_eq!(report.geometry_status, EvidenceStatus::Preserved);
    assert!(report.matches[0].dx_start.unwrap().abs() <= 1.0e-9);
    assert!(report.matches[0].dx_end.unwrap().abs() <= 1.0e-9);
    assert!(report.matches[0].width_delta.unwrap().abs() <= 1.0e-9);
    assert!(report.matches[0].baseline_delta.unwrap().abs() <= 1.0e-9);
}

#[test]
fn candidate_page_dimensions_do_not_rescale_the_fixed_scan_mapping() {
    let scan = observation(
        mapping([0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0]),
        vec![scan_line(
            "line-fixed-mapping",
            known(
                "abcdefghij".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            0,
            geometry(100.0, 200.0, 300.0, 240.0, 220.0),
        )],
    );
    let original_pdf = candidate(&[("abcdefghij", 60.0, 115.0)]);
    let mut resized_pdf = original_pdf.clone();
    resized_pdf.page.width = 700.0;
    resized_pdf.page.height = 1400.0;
    let comparison_policy = policy(ScanGranularity::Line, 20.0, 20.0);

    let original = compare_scan_observation(&scan, &original_pdf, &comparison_policy);
    let resized = compare_scan_observation(&scan, &resized_pdf, &comparison_policy);
    let original_match = &original.matches[0];
    let resized_match = &resized.matches[0];

    assert_eq!(original_match.dx_start, Some(10.0));
    assert_eq!(original_match.dx_end, Some(10.0));
    assert_eq!(original_match.width_delta, Some(0.0));
    assert_eq!(original_match.baseline_delta, Some(5.0));
    assert_eq!(original_match.dx_start, resized_match.dx_start);
    assert_eq!(original_match.dx_end, resized_match.dx_end);
    assert_eq!(original_match.width_delta, resized_match.width_delta);
    assert_eq!(original_match.baseline_delta, resized_match.baseline_delta);

    let projected_from_original = (
        60.0 - original_match.dx_start.unwrap(),
        160.0 - original_match.dx_end.unwrap(),
        115.0 - original_match.baseline_delta.unwrap(),
    );
    let projected_from_resized = (
        60.0 - resized_match.dx_start.unwrap(),
        160.0 - resized_match.dx_end.unwrap(),
        115.0 - resized_match.baseline_delta.unwrap(),
    );
    assert_eq!(projected_from_original, (50.0, 150.0, 110.0));
    assert_eq!(projected_from_resized, projected_from_original);
}

#[test]
fn reordering_serialized_scan_units_preserves_the_semantic_report() {
    let make_units = || {
        vec![
            scan_line(
                "line-alpha",
                known("alpha".to_string(), EvidenceBasis::Inferred, "prov-text"),
                0,
                geometry(100.0, 90.0, 150.0, 110.0, 100.0),
            ),
            scan_line(
                "line-beta",
                known("beta".to_string(), EvidenceBasis::Inferred, "prov-text"),
                1,
                geometry(200.0, 190.0, 240.0, 210.0, 200.0),
            ),
        ]
    };
    let original_scan = observation(identity_mapping(), make_units());
    let mut reversed_units = make_units();
    reversed_units.reverse();
    let reordered_scan = observation(identity_mapping(), reversed_units);
    let pdf = candidate(&[("alpha", 100.0, 100.0), ("beta", 200.0, 200.0)]);
    let comparison_policy = policy(ScanGranularity::Line, 0.0, 0.0);

    let original = compare_scan_observation(&original_scan, &pdf, &comparison_policy);
    let reordered = compare_scan_observation(&reordered_scan, &pdf, &comparison_policy);

    assert_eq!(original.content_status, reordered.content_status);
    assert_eq!(original.geometry_status, reordered.geometry_status);
    assert_eq!(original.overall_status, reordered.overall_status);
    assert_eq!(
        original.coverage.matched_scan,
        reordered.coverage.matched_scan
    );
    assert_eq!(original.coverage.total_scan, reordered.coverage.total_scan);
    assert_eq!(
        original.coverage.matched_candidate,
        reordered.coverage.matched_candidate
    );
    assert_eq!(
        original.coverage.total_candidate,
        reordered.coverage.total_candidate
    );
    for unit_id in ["line-alpha", "line-beta"] {
        let original_match = original
            .matches
            .iter()
            .find(|matched| matched.scan_unit_id == unit_id)
            .unwrap();
        let reordered_match = reordered
            .matches
            .iter()
            .find(|matched| matched.scan_unit_id == unit_id)
            .unwrap();
        assert_eq!(original_match, reordered_match);
    }
    assert!(original.unmatched_scan.is_empty());
    assert!(reordered.unmatched_scan.is_empty());
    assert!(original.unmatched_candidate.is_empty());
    assert!(reordered.unmatched_candidate.is_empty());
    assert!(original.reflow.is_empty());
    assert!(reordered.reflow.is_empty());
    assert!(original.diagnostics.is_empty());
    assert!(reordered.diagnostics.is_empty());
}

#[test]
fn unknown_page_mapping_preserves_text_but_never_invents_zero_deltas() {
    let scan = observation(
        unknown(UnknownReason::NotObserved),
        vec![scan_line(
            "line-1",
            known(
                "known text".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            0,
            geometry(100.0, 100.0, 200.0, 120.0, 110.0),
        )],
    );
    let pdf = candidate(&[("known text", 100.0, 110.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].dx_start, None);
    assert_eq!(report.matches[0].dx_end, None);
    assert_eq!(report.matches[0].width_delta, None);
    assert_eq!(report.matches[0].baseline_delta, None);
    assert_eq!(report.matches[0].horizontal_status, EvidenceStatus::Unknown);
    assert_eq!(report.matches[0].baseline_status, EvidenceStatus::Unknown);
}

#[test]
fn empty_scope_with_known_mapping_is_unknown_and_diagnosed() {
    let scan = observation(identity_mapping(), Vec::new());
    let pdf = document(Vec::new(), PageRotation::Deg0, 1.0);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.coverage.matched_scan, 0);
    assert_eq!(report.coverage.total_scan, 0);
    assert_eq!(report.coverage.matched_candidate, 0);
    assert_eq!(report.coverage.total_candidate, 0);
    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic, ScanComparisonDiagnostic::EmptyComparisonScope)));
}

#[test]
fn unknown_page_mapping_cannot_preserve_geometry_in_an_empty_scope() {
    let scan = observation(unknown(UnknownReason::NotObserved), Vec::new());
    let pdf = document(Vec::new(), PageRotation::Deg0, 1.0);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.coverage.total_scan, 0);
    assert_eq!(report.coverage.total_candidate, 0);
    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic, ScanComparisonDiagnostic::EmptyComparisonScope)));
    assert!(report.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        ScanComparisonDiagnostic::PageMappingUnknown { .. }
    )));
}

#[test]
fn deltas_exactly_at_the_declared_tolerances_are_preserved() {
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-limit",
            known("limit".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            geometry(100.0, 90.0, 150.0, 110.0, 100.0),
        )],
    );
    let shifted = document(
        vec![glyph(Some("limit".chars().collect()), 105.0, 105.0, 50.0)],
        PageRotation::Deg0,
        1.0,
    );
    let widened = document(
        vec![glyph(Some("limit".chars().collect()), 100.0, 105.0, 55.0)],
        PageRotation::Deg0,
        1.0,
    );
    let comparison_policy = policy(ScanGranularity::Line, 5.0, 5.0);

    let shifted_report = compare_scan_observation(&scan, &shifted, &comparison_policy);
    assert_eq!(shifted_report.matches[0].dx_start, Some(5.0));
    assert_eq!(shifted_report.matches[0].dx_end, Some(5.0));
    assert_eq!(shifted_report.matches[0].width_delta, Some(0.0));
    assert_eq!(shifted_report.matches[0].baseline_delta, Some(5.0));
    assert_eq!(shifted_report.geometry_status, EvidenceStatus::Preserved);
    assert_eq!(shifted_report.overall_status, EvidenceStatus::Preserved);

    let widened_report = compare_scan_observation(&scan, &widened, &comparison_policy);
    assert_eq!(widened_report.matches[0].dx_start, Some(0.0));
    assert_eq!(widened_report.matches[0].dx_end, Some(5.0));
    assert_eq!(widened_report.matches[0].width_delta, Some(5.0));
    assert_eq!(widened_report.matches[0].baseline_delta, Some(5.0));
    assert_eq!(widened_report.geometry_status, EvidenceStatus::Preserved);
    assert_eq!(widened_report.overall_status, EvidenceStatus::Preserved);
}

#[test]
fn unknown_baseline_is_not_replaced_by_the_known_bbox_edge() {
    let mut bbox_only = geometry(100.0, 90.0, 140.0, 110.0, 100.0);
    bbox_only.baseline = unknown(UnknownReason::NotObserved);
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-bbox-only",
            known("bbox".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            bbox_only,
        )],
    );
    let pdf = candidate(&[("bbox", 100.0, 100.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.matches[0].dx_start, Some(0.0));
    assert_eq!(report.matches[0].dx_end, Some(0.0));
    assert_eq!(report.matches[0].width_delta, Some(0.0));
    assert_eq!(
        report.matches[0].horizontal_status,
        EvidenceStatus::Preserved
    );
    assert_eq!(report.matches[0].baseline_delta, None);
    assert_eq!(report.matches[0].baseline_status, EvidenceStatus::Unknown);
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
}

#[test]
fn repeated_candidate_line_text_is_ambiguous_not_preserved() {
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-repeat",
            known("repeat".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            geometry(100.0, 90.0, 160.0, 110.0, 100.0),
        )],
    );
    let pdf = candidate(&[("repeat", 100.0, 100.0), ("repeat", 200.0, 120.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert_eq!(report.coverage.matched_scan, 0);
    assert_eq!(report.coverage.total_scan, 1);
    assert_eq!(report.coverage.matched_candidate, 0);
    assert_eq!(report.coverage.total_candidate, 2);
    assert!(report.matches.is_empty());
    assert_eq!(report.unmatched_scan, vec!["line-repeat".to_string()]);
}

#[test]
fn unmapped_candidate_glyph_keeps_content_opaque() {
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-a",
            known("A".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            geometry(100.0, 90.0, 110.0, 110.0, 100.0),
        )],
    );
    let pdf = document(
        vec![glyph(None, 100.0, 100.0, 10.0)],
        PageRotation::Deg0,
        1.0,
    );

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert_eq!(report.coverage.matched_scan, 0);
    assert_eq!(report.coverage.total_scan, 1);
    assert_eq!(report.coverage.matched_candidate, 0);
    assert_eq!(report.coverage.total_candidate, 1);
}

#[test]
fn repeated_words_are_disambiguated_by_span_inside_the_matched_line() {
    let units = vec![
        scan_line(
            "line-echo",
            known(
                "echo echo".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            0,
            unknown_geometry(),
        ),
        scan_word(
            "word-first",
            "line-echo",
            "echo",
            0,
            4,
            0,
            geometry(100.0, 190.0, 140.0, 210.0, 200.0),
        ),
        scan_word(
            "word-second",
            "line-echo",
            "echo",
            5,
            9,
            1,
            geometry(150.0, 190.0, 190.0, 210.0, 200.0),
        ),
    ];
    let scan = observation(identity_mapping(), units);
    let pdf = candidate(&[("echo echo", 100.0, 200.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Word, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.geometry_status, EvidenceStatus::Preserved);
    assert_eq!(report.overall_status, EvidenceStatus::Preserved);
    assert_eq!(report.coverage.matched_scan, 2);
    assert_eq!(report.coverage.total_scan, 2);
    assert_eq!(report.coverage.matched_candidate, 2);
    assert_eq!(report.coverage.total_candidate, 2);
    let first = report
        .matches
        .iter()
        .find(|matched| matched.scan_unit_id == "word-first")
        .unwrap();
    let second = report
        .matches
        .iter()
        .find(|matched| matched.scan_unit_id == "word-second")
        .unwrap();
    assert_eq!(first.candidate_scalar_range, (0, 4));
    assert_eq!(second.candidate_scalar_range, (5, 9));
}

#[test]
fn multi_scalar_ligature_entirely_inside_one_word_has_correct_geometry() {
    let scan = observation(
        identity_mapping(),
        vec![
            scan_line(
                "line-fi",
                known("fi".to_string(), EvidenceBasis::Inferred, "prov-text"),
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-fi",
                "line-fi",
                "fi",
                0,
                2,
                0,
                geometry(100.0, 90.0, 120.0, 110.0, 100.0),
            ),
        ],
    );
    let pdf = document(
        vec![glyph(Some(vec!['f', 'i']), 100.0, 100.0, 20.0)],
        PageRotation::Deg0,
        1.0,
    );

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Word, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.geometry_status, EvidenceStatus::Preserved);
    assert_eq!(report.overall_status, EvidenceStatus::Preserved);
    assert_eq!(report.coverage.matched_scan, 1);
    assert_eq!(report.coverage.total_scan, 1);
    assert_eq!(report.coverage.matched_candidate, 1);
    assert_eq!(report.coverage.total_candidate, 1);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].scan_unit_id, "word-fi");
    assert_eq!(report.matches[0].candidate_scalar_range, (0, 2));
    assert_eq!(report.matches[0].dx_start, Some(0.0));
    assert_eq!(report.matches[0].dx_end, Some(0.0));
    assert_eq!(report.matches[0].width_delta, Some(0.0));
    assert_eq!(report.matches[0].baseline_delta, Some(0.0));
    assert_eq!(
        report.matches[0].horizontal_status,
        EvidenceStatus::Preserved
    );
    assert_eq!(report.matches[0].baseline_status, EvidenceStatus::Preserved);
}

#[test]
fn word_boundary_inside_one_ligature_keeps_geometry_unknown() {
    let scan = observation(
        identity_mapping(),
        vec![
            scan_line(
                "line-ligature",
                known("a b".to_string(), EvidenceBasis::Inferred, "prov-text"),
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-a",
                "line-ligature",
                "a",
                0,
                1,
                0,
                geometry(100.0, 90.0, 110.0, 110.0, 100.0),
            ),
            scan_word(
                "word-b",
                "line-ligature",
                "b",
                2,
                3,
                1,
                geometry(120.0, 90.0, 130.0, 110.0, 100.0),
            ),
        ],
    );
    let pdf = document(
        vec![glyph(Some(vec!['a', ' ', 'b']), 100.0, 100.0, 30.0)],
        PageRotation::Deg0,
        1.0,
    );

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Word, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.coverage.matched_scan, 2);
    assert_eq!(report.coverage.total_scan, 2);
    assert_eq!(report.coverage.matched_candidate, 2);
    assert_eq!(report.coverage.total_candidate, 2);
    assert_eq!(report.matches.len(), 2);
    for matched in &report.matches {
        assert_eq!(matched.dx_start, None);
        assert_eq!(matched.dx_end, None);
        assert_eq!(matched.width_delta, None);
        assert_eq!(matched.baseline_delta, None);
        assert_eq!(matched.horizontal_status, EvidenceStatus::Unknown);
        assert_eq!(matched.baseline_status, EvidenceStatus::Unknown);
    }
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
}

#[test]
fn zero_deltas_with_partial_candidate_coverage_remain_unknown() {
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-match",
            known("match".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            geometry(100.0, 90.0, 150.0, 110.0, 100.0),
        )],
    );
    let pdf = candidate(&[("match", 100.0, 100.0), ("extra", 300.0, 200.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

    assert_eq!(report.matches[0].dx_start, Some(0.0));
    assert_eq!(report.matches[0].dx_end, Some(0.0));
    assert_eq!(report.matches[0].width_delta, Some(0.0));
    assert_eq!(report.matches[0].baseline_delta, Some(0.0));
    assert_eq!(report.coverage.matched_scan, 1);
    assert_eq!(report.coverage.total_scan, 1);
    assert_eq!(report.coverage.matched_candidate, 1);
    assert_eq!(report.coverage.total_candidate, 2);
    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
}

#[test]
fn a_known_geometry_violation_dominates_an_unknown_unit() {
    let scan = observation(
        identity_mapping(),
        vec![
            scan_line(
                "line-violated",
                known("badgeom".to_string(), EvidenceBasis::Inferred, "prov-text"),
                0,
                geometry(100.0, 90.0, 170.0, 110.0, 100.0),
            ),
            scan_line(
                "line-unknown",
                known("opaque".to_string(), EvidenceBasis::Inferred, "prov-text"),
                1,
                unknown_geometry(),
            ),
        ],
    );
    let pdf = candidate(&[("badgeom", 120.0, 100.0), ("opaque", 100.0, 200.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Line, 1.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Preserved);
    assert_eq!(report.geometry_status, EvidenceStatus::Violated);
    assert_eq!(report.overall_status, EvidenceStatus::Violated);
    let violated = report
        .matches
        .iter()
        .find(|matched| matched.scan_unit_id == "line-violated")
        .unwrap();
    let opaque = report
        .matches
        .iter()
        .find(|matched| matched.scan_unit_id == "line-unknown")
        .unwrap();
    assert_eq!(violated.horizontal_status, EvidenceStatus::Violated);
    assert_eq!(opaque.horizontal_status, EvidenceStatus::Unknown);
    assert_eq!(opaque.dx_start, None);
}

#[test]
fn unique_words_split_across_candidate_lines_prove_reflow() {
    let scan = observation(
        identity_mapping(),
        vec![
            scan_line(
                "line-source",
                known(
                    "alpha beta".to_string(),
                    EvidenceBasis::Inferred,
                    "prov-text",
                ),
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-alpha",
                "line-source",
                "alpha",
                0,
                5,
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-beta",
                "line-source",
                "beta",
                6,
                10,
                1,
                unknown_geometry(),
            ),
        ],
    );
    let pdf = candidate(&[("alpha", 100.0, 100.0), ("beta", 100.0, 120.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Word, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Violated);
    assert_eq!(report.overall_status, EvidenceStatus::Violated);
    assert_eq!(report.reflow.len(), 1);
    assert_eq!(report.reflow[0].scan_line_id, "line-source");
    assert_eq!(report.reflow[0].candidate_line_indices, vec![0, 1]);
    assert_eq!(
        report.reflow[0].scan_word_ids,
        vec!["word-alpha".to_string(), "word-beta".to_string()]
    );
}

#[test]
fn repeated_word_prevents_a_false_reflow_witness() {
    let scan = observation(
        identity_mapping(),
        vec![
            scan_line(
                "line-source",
                known(
                    "echo echo".to_string(),
                    EvidenceBasis::Inferred,
                    "prov-text",
                ),
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-first",
                "line-source",
                "echo",
                0,
                4,
                0,
                unknown_geometry(),
            ),
            scan_word(
                "word-second",
                "line-source",
                "echo",
                5,
                9,
                1,
                unknown_geometry(),
            ),
        ],
    );
    let pdf = candidate(&[("echo", 100.0, 100.0), ("echo", 100.0, 120.0)]);

    let report = compare_scan_observation(&scan, &pdf, &policy(ScanGranularity::Word, 0.0, 0.0));

    assert_eq!(report.content_status, EvidenceStatus::Unknown);
    assert_eq!(report.overall_status, EvidenceStatus::Unknown);
    assert!(report.reflow.is_empty());
}

#[test]
fn rotation_or_nondefault_user_unit_leave_candidate_geometry_unknown() {
    let scan = observation(
        identity_mapping(),
        vec![scan_line(
            "line-1",
            known("rotate".to_string(), EvidenceBasis::Inferred, "prov-text"),
            0,
            geometry(100.0, 90.0, 160.0, 110.0, 100.0),
        )],
    );
    let mut rotated = candidate(&[("rotate", 100.0, 100.0)]);
    rotated.page.rotation = PageRotation::Deg90;
    let mut scaled_user_unit = candidate(&[("rotate", 100.0, 100.0)]);
    scaled_user_unit.page.user_unit = 2.0;

    for pdf in [&rotated, &scaled_user_unit] {
        let report = compare_scan_observation(&scan, pdf, &policy(ScanGranularity::Line, 0.0, 0.0));

        assert_eq!(report.content_status, EvidenceStatus::Preserved);
        assert_eq!(report.geometry_status, EvidenceStatus::Unknown);
        assert_eq!(report.overall_status, EvidenceStatus::Unknown);
        assert_eq!(report.matches[0].dx_start, None);
        assert_eq!(report.matches[0].dx_end, None);
        assert_eq!(report.matches[0].width_delta, None);
        assert_eq!(report.matches[0].baseline_delta, None);
        assert!(report.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ScanComparisonDiagnostic::UnsupportedCandidateCoordinateMapping { .. }
        )));
    }
}
