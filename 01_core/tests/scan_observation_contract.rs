//! Oraculos L1 independentes para `ScanObservation v1`.
//!
//! Este teste fixa apenas a API publica de dominio e validacao pura. Ele nao
//! pressupoe JSON, OCR, arquivos, builders ou uma implementacao candidata.

use decalque_core::entities::{
    validate_scan_observation, Claim, Confidence, EvidenceBasis, FramedBbox, FramedPolygon,
    FramedPolyline, GeometryClaims, ObservationKind, ObservationUnit, PageFrame, PageMapping,
    PageMappingKind, ProducerIdentity, ProvenanceRecord, ProvenanceStage, RasterArtifact,
    RasterFrame, ScanObservation, ScanSource, UnicodeRange, UnknownReason,
};

const RASTER_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PARAMETERS_SHA256: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn known_confidence(value: f64) -> Confidence {
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
        confidence: known_confidence(1.0),
    }
}

fn unknown<T>(reason: UnknownReason) -> Claim<T> {
    Claim::Unknown {
        reason,
        evidence: Vec::new(),
        detail: Some("not supplied by this fixture".to_string()),
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

fn bbox(x0: f64, y0: f64, x1: f64, y1: f64, evidence: &str) -> Claim<FramedBbox> {
    known(
        FramedBbox {
            frame_id: "scan-px".to_string(),
            x0,
            y0,
            x1,
            y1,
        },
        EvidenceBasis::Observed,
        evidence,
    )
}

fn baseline(y: f64, evidence: &str) -> Claim<FramedPolyline> {
    known(
        FramedPolyline {
            frame_id: "scan-px".to_string(),
            points: vec![(100.0, y), (300.0, y)],
        },
        EvidenceBasis::Observed,
        evidence,
    )
}

fn unknown_geometry() -> GeometryClaims {
    GeometryClaims {
        bbox: unknown(UnknownReason::NotObserved),
        polygon: unknown(UnknownReason::NotObserved),
        baseline: unknown(UnknownReason::NotObserved),
    }
}

fn line(id: &str, text: Claim<String>, geometry: GeometryClaims) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind: ObservationKind::Line,
        parent_id: None,
        reading_order: known(0, EvidenceBasis::Inferred, "prov-layout"),
        text,
        span_in_parent: unknown(UnknownReason::NotObserved),
        geometry,
    }
}

fn observation_with_mapping(page_mapping: Claim<PageMapping>) -> ScanObservation {
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
        units: vec![line(
            "line-1",
            known(
                "Hello world".to_string(),
                EvidenceBasis::Inferred,
                "prov-text",
            ),
            GeometryClaims {
                bbox: bbox(100.0, 200.0, 300.0, 240.0, "prov-layout"),
                polygon: unknown(UnknownReason::NotObserved),
                baseline: baseline(232.0, "prov-layout"),
            },
        )],
        diagnostics: Vec::new(),
    }
}

fn scaled_mapping() -> Claim<PageMapping> {
    known(
        PageMapping {
            source_frame_id: "scan-px".to_string(),
            target_frame: PageFrame {
                id: "source-page-pt".to_string(),
                extent: (500.0, 1000.0),
            },
            kind: PageMappingKind::Homography3x3,
            matrix: [0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0],
            max_error_pt: 0.0,
        },
        EvidenceBasis::Derived,
        "prov-map",
    )
}

fn valid_observation() -> ScanObservation {
    observation_with_mapping(scaled_mapping())
}

#[test]
fn accepts_a_valid_observation_with_a_nonsingular_homography() {
    assert!(validate_scan_observation(&valid_observation()).is_ok());
}

#[test]
fn accepts_unknown_claims_without_inventing_default_values() {
    let mut observation = observation_with_mapping(unknown(UnknownReason::NotObserved));
    observation.units[0].text = unknown(UnknownReason::Ambiguous);
    observation.units[0].reading_order = unknown(UnknownReason::BudgetExhausted);
    observation.units[0].geometry = unknown_geometry();

    assert!(validate_scan_observation(&observation).is_ok());
    assert!(matches!(observation.page_mapping, Claim::Unknown { .. }));
    assert!(matches!(observation.units[0].text, Claim::Unknown { .. }));
    assert!(matches!(
        observation.units[0].geometry.bbox,
        Claim::Unknown { .. }
    ));
}

#[test]
fn rejects_a_known_claim_without_evidence() {
    let mut observation = valid_observation();
    observation.units[0].text = Claim::Known {
        value: "Hello world".to_string(),
        basis: EvidenceBasis::Inferred,
        evidence: Vec::new(),
        confidence: known_confidence(1.0),
    };

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_degenerate_inverted_nonfinite_and_out_of_frame_bboxes() {
    for invalid_bbox in [
        (100.0, 200.0, 100.0, 240.0),
        (300.0, 200.0, 100.0, 240.0),
        (100.0, 200.0, f64::NAN, 240.0),
        (100.0, 200.0, 1001.0, 240.0),
    ] {
        let mut observation = valid_observation();
        observation.units[0].geometry.bbox = bbox(
            invalid_bbox.0,
            invalid_bbox.1,
            invalid_bbox.2,
            invalid_bbox.3,
            "prov-layout",
        );
        assert!(
            validate_scan_observation(&observation).is_err(),
            "bbox should be rejected: {invalid_bbox:?}"
        );
    }
}

#[test]
fn accepts_an_independently_observed_child_bbox() {
    let mut observation = valid_observation();
    observation.provenance.push(provenance(
        "prov-word-layout",
        ProvenanceStage::LayoutDetection,
        &[],
        &["prov-layout"],
    ));
    observation.units.push(ObservationUnit {
        id: "word-1".to_string(),
        kind: ObservationKind::Word,
        parent_id: Some("line-1".to_string()),
        reading_order: known(0, EvidenceBasis::Derived, "prov-segment"),
        text: known("Hello".to_string(), EvidenceBasis::Derived, "prov-segment"),
        span_in_parent: known(
            UnicodeRange { start: 0, end: 5 },
            EvidenceBasis::Derived,
            "prov-segment",
        ),
        geometry: GeometryClaims {
            bbox: bbox(100.0, 200.0, 180.0, 240.0, "prov-word-layout"),
            polygon: unknown(UnknownReason::NotObserved),
            baseline: unknown(UnknownReason::NotObserved),
        },
    });

    assert!(validate_scan_observation(&observation).is_ok());
}

#[test]
fn rejects_a_child_bbox_copied_from_its_parent() {
    let mut observation = valid_observation();
    observation.units.push(ObservationUnit {
        id: "word-1".to_string(),
        kind: ObservationKind::Word,
        parent_id: Some("line-1".to_string()),
        reading_order: known(0, EvidenceBasis::Derived, "prov-segment"),
        text: known("Hello".to_string(), EvidenceBasis::Derived, "prov-segment"),
        span_in_parent: known(
            UnicodeRange { start: 0, end: 5 },
            EvidenceBasis::Derived,
            "prov-segment",
        ),
        geometry: GeometryClaims {
            // This is the parent's exact box with the parent's evidence, not
            // independently observed word geometry.
            bbox: known(
                FramedBbox {
                    frame_id: "scan-px".to_string(),
                    x0: 100.0,
                    y0: 200.0,
                    x1: 300.0,
                    y1: 240.0,
                },
                EvidenceBasis::Derived,
                "prov-layout",
            ),
            polygon: unknown(UnknownReason::NotObserved),
            baseline: unknown(UnknownReason::NotObserved),
        },
    });

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_a_cycle_in_the_unit_parent_graph() {
    let mut observation = valid_observation();
    observation.units = vec![
        ObservationUnit {
            id: "line-a".to_string(),
            kind: ObservationKind::Line,
            parent_id: Some("line-b".to_string()),
            reading_order: unknown(UnknownReason::NotObserved),
            text: unknown(UnknownReason::NotObserved),
            span_in_parent: unknown(UnknownReason::NotObserved),
            geometry: unknown_geometry(),
        },
        ObservationUnit {
            id: "line-b".to_string(),
            kind: ObservationKind::Line,
            parent_id: Some("line-a".to_string()),
            reading_order: unknown(UnknownReason::NotObserved),
            text: unknown(UnknownReason::NotObserved),
            span_in_parent: unknown(UnknownReason::NotObserved),
            geometry: unknown_geometry(),
        },
    ];

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_a_cycle_in_the_provenance_graph() {
    let mut observation = valid_observation();
    observation.provenance[0].parent_provenance_ids = vec!["prov-text".to_string()];
    observation.provenance[1].parent_provenance_ids = vec!["prov-layout".to_string()];

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_every_provenance_root_without_the_observed_raster_including_manual_annotation() {
    for (label, stage) in [
        ("layout", ProvenanceStage::LayoutDetection),
        ("text", ProvenanceStage::TextRecognition),
        ("segmentation", ProvenanceStage::Segmentation),
        ("mapping", ProvenanceStage::CoordinateTransform),
        ("manual", ProvenanceStage::ManualAnnotation),
        ("other", ProvenanceStage::Other),
    ] {
        let mut observation = valid_observation();
        observation
            .provenance
            .push(provenance(&format!("prov-{label}-root"), stage, &[], &[]));

        assert!(
            validate_scan_observation(&observation).is_err(),
            "raiz {label} sem referencia direta ao raster deveria ser rejeitada"
        );
    }
}

#[test]
fn rejects_a_singular_homography() {
    let mut observation = valid_observation();
    let Claim::Known { value, .. } = &mut observation.page_mapping else {
        unreachable!("fixture has a known page mapping")
    };
    value.matrix = [0.0; 9];

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_a_nonsingular_homography_with_zero_denominator_at_an_observed_point() {
    // det(M) == -1, but w' = 0.01*x - 1 is zero at the bbox/baseline
    // point x=100. This isolates the projective-horizon invariant from
    // ordinary matrix singularity.
    let matrix = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.01, 0.0, -1.0];
    let determinant = matrix[0] * (matrix[4] * matrix[8] - matrix[5] * matrix[7])
        - matrix[1] * (matrix[3] * matrix[8] - matrix[5] * matrix[6])
        + matrix[2] * (matrix[3] * matrix[7] - matrix[4] * matrix[6]);
    assert_eq!(determinant, -1.0);

    let mut observation = valid_observation();
    let Claim::Known { value, .. } = &mut observation.page_mapping else {
        unreachable!("fixture has a known page mapping")
    };
    value.matrix = matrix;

    assert!(validate_scan_observation(&observation).is_err());
}

#[test]
fn rejects_bbox_that_does_not_enclose_its_known_polygon() {
    let mut observation = valid_observation();
    observation.units[0].geometry.polygon = known(
        FramedPolygon {
            frame_id: "scan-px".to_string(),
            points: vec![(90.0, 210.0), (200.0, 200.0), (300.0, 240.0)],
        },
        EvidenceBasis::Observed,
        "prov-layout",
    );

    assert!(validate_scan_observation(&observation).is_err());
}
