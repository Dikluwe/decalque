//! Testes comportamentais da derivação do perfil geométrico observado.
//!
//! As expectativas usam somente a API pública e não dependem da representação privada.

use decalque_core::{
    derive_scan_layout, Claim, Confidence, EvidenceBasis, FramedBbox, FramedPolygon,
    FramedPolyline, GeometryClaims, ObservationKind, ObservationUnit, PageFrame, PageMapping,
    PageMappingKind, ProducerIdentity, ProvenanceRecord, ProvenanceStage, RasterArtifact,
    RasterFrame, ScanLayoutBoxMeasurementView, ScanLayoutBoxView, ScanLayoutCountView,
    ScanLayoutInsetsMeasurementView, ScanLayoutInsetsView, ScanLayoutNumberListMeasurementView,
    ScanLayoutPageView, ScanLayoutProfile, ScanLayoutRegionView, ScanLayoutScalarMeasurementView,
    ScanLayoutStringListView, ScanObservation, ScanSource, UnicodeRange, UnknownReason,
};

const RASTER_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DISTINCT_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMETERS_SHA256: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const PROVENANCE_ID: &str = "prov-observed-layout";

#[derive(Debug, PartialEq)]
struct Measurement<T> {
    status: &'static str,
    reason: Option<&'static str>,
    value: Option<T>,
}

#[derive(Debug, PartialEq)]
struct BoxLedger {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

#[derive(Debug, PartialEq)]
struct InsetsLedger {
    top: f64,
    left: f64,
    right: f64,
    bottom: f64,
}

#[derive(Debug, PartialEq)]
struct PageLedger {
    status: &'static str,
    reason: Option<&'static str>,
    frame_id: Option<String>,
    extent: Option<(f64, f64)>,
    max_error: Option<f64>,
}

#[derive(Debug, PartialEq)]
struct RegionLedger {
    id: String,
    support_ids: Vec<String>,
    envelope: Measurement<BoxLedger>,
    starts: Measurement<Vec<f64>>,
    gaps: Measurement<Vec<f64>>,
    delta: Measurement<f64>,
}

#[derive(Debug, PartialEq)]
struct ProfileLedger {
    page_index: u32,
    raster_sha256: String,
    total_lines: usize,
    projected_lines: Measurement<usize>,
    page: PageLedger,
    global_envelope: Measurement<BoxLedger>,
    global_insets: Measurement<InsetsLedger>,
    regions: Vec<RegionLedger>,
}

fn derived<T>(value: T) -> Measurement<T> {
    Measurement {
        status: "derived",
        reason: None,
        value: Some(value),
    }
}

fn unavailable<T>(reason: &'static str) -> Measurement<T> {
    Measurement {
        status: "unknown",
        reason: Some(reason),
        value: None,
    }
}

fn known<T>(value: T, basis: EvidenceBasis) -> Claim<T> {
    Claim::Known {
        value,
        basis,
        evidence: vec![PROVENANCE_ID.to_string()],
        confidence: Confidence::Known {
            value: 1.0,
            semantics: "behavior-fixture".to_string(),
        },
    }
}

fn unknown<T>() -> Claim<T> {
    Claim::Unknown {
        reason: UnknownReason::NotObserved,
        evidence: Vec::new(),
        detail: Some("not observed by the fixture".to_string()),
    }
}

fn known_bbox(x0: f64, y0: f64, x1: f64, y1: f64) -> Claim<FramedBbox> {
    known(
        FramedBbox {
            frame_id: "scan-px".to_string(),
            x0,
            y0,
            x1,
            y1,
        },
        EvidenceBasis::Observed,
    )
}

fn geometry(bbox: Claim<FramedBbox>) -> GeometryClaims {
    GeometryClaims {
        bbox,
        polygon: unknown::<FramedPolygon>(),
        baseline: unknown::<FramedPolyline>(),
    }
}

fn unit(
    id: &str,
    kind: ObservationKind,
    parent_id: Option<&str>,
    reading_order: Option<u32>,
    bbox: Claim<FramedBbox>,
) -> ObservationUnit {
    ObservationUnit {
        id: id.to_string(),
        kind,
        parent_id: parent_id.map(str::to_string),
        reading_order: reading_order
            .map(|value| known(value, EvidenceBasis::Inferred))
            .unwrap_or_else(unknown),
        text: unknown::<String>(),
        span_in_parent: unknown::<UnicodeRange>(),
        geometry: geometry(bbox),
    }
}

fn region(id: &str, reading_order: Option<u32>) -> ObservationUnit {
    unit(id, ObservationKind::Region, None, reading_order, unknown())
}

fn region_with_bbox(
    id: &str,
    reading_order: Option<u32>,
    coords: (f64, f64, f64, f64),
) -> ObservationUnit {
    unit(
        id,
        ObservationKind::Region,
        None,
        reading_order,
        known_bbox(coords.0, coords.1, coords.2, coords.3),
    )
}

fn line(
    id: &str,
    parent_id: Option<&str>,
    reading_order: Option<u32>,
    coords: Option<(f64, f64, f64, f64)>,
) -> ObservationUnit {
    unit(
        id,
        ObservationKind::Line,
        parent_id,
        reading_order,
        coords
            .map(|(x0, y0, x1, y1)| known_bbox(x0, y0, x1, y1))
            .unwrap_or_else(unknown),
    )
}

fn descendant(
    id: &str,
    kind: ObservationKind,
    parent_id: &str,
    coords: (f64, f64, f64, f64),
) -> ObservationUnit {
    unit(
        id,
        kind,
        Some(parent_id),
        Some(0),
        known_bbox(coords.0, coords.1, coords.2, coords.3),
    )
}

fn known_mapping(matrix: [f64; 9], extent: (f64, f64), max_error_pt: f64) -> Claim<PageMapping> {
    known(
        PageMapping {
            source_frame_id: "scan-px".to_string(),
            target_frame: PageFrame {
                id: "source-page-pt".to_string(),
                extent,
            },
            kind: PageMappingKind::Homography3x3,
            matrix,
            max_error_pt,
        },
        EvidenceBasis::Derived,
    )
}

fn identity_mapping(extent: (f64, f64), max_error_pt: f64) -> Claim<PageMapping> {
    known_mapping(
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        extent,
        max_error_pt,
    )
}

fn observation(
    page_index: u32,
    raster_sha256: &str,
    page_mapping: Claim<PageMapping>,
    units: Vec<ObservationUnit>,
) -> ScanObservation {
    ScanObservation {
        source: ScanSource {
            page_index,
            raster: RasterArtifact {
                artifact_id: "raster-observed".to_string(),
                sha256: raster_sha256.to_string(),
                media_type: "image/png".to_string(),
                width_px: 1000,
                height_px: 1000,
            },
        },
        producer: ProducerIdentity {
            name: "independent-oracle".to_string(),
            version: "1".to_string(),
            run_id: "layout-l1".to_string(),
        },
        raster_frame: RasterFrame {
            id: "scan-px".to_string(),
            extent: (1000, 1000),
        },
        page_mapping,
        provenance: vec![ProvenanceRecord {
            id: PROVENANCE_ID.to_string(),
            stage: ProvenanceStage::LayoutDetection,
            tool_name: "independent-oracle".to_string(),
            tool_version: "1".to_string(),
            model_identifier: "fixture-model".to_string(),
            method: "behavior-fixture".to_string(),
            parameters_sha256: PARAMETERS_SHA256.to_string(),
            input_artifact_ids: vec!["raster-observed".to_string()],
            parent_provenance_ids: Vec::new(),
        }],
        units,
        diagnostics: Vec::new(),
    }
}

fn capture_count(view: ScanLayoutCountView<'_>) -> Measurement<usize> {
    Measurement {
        status: view.status(),
        reason: view.reason(),
        value: view.value(),
    }
}

fn capture_box_value(view: ScanLayoutBoxView) -> BoxLedger {
    BoxLedger {
        x0: view.x0_pt(),
        y0: view.y0_pt(),
        x1: view.x1_pt(),
        y1: view.y1_pt(),
    }
}

fn capture_box(view: ScanLayoutBoxMeasurementView<'_>) -> Measurement<BoxLedger> {
    Measurement {
        status: view.status(),
        reason: view.reason(),
        value: view.value().map(capture_box_value),
    }
}

fn capture_insets_value(view: ScanLayoutInsetsView) -> InsetsLedger {
    InsetsLedger {
        top: view.top_pt(),
        left: view.left_pt(),
        right: view.right_pt(),
        bottom: view.bottom_pt(),
    }
}

fn capture_insets(view: ScanLayoutInsetsMeasurementView<'_>) -> Measurement<InsetsLedger> {
    Measurement {
        status: view.status(),
        reason: view.reason(),
        value: view.value().map(capture_insets_value),
    }
}

fn capture_numbers(view: ScanLayoutNumberListMeasurementView<'_>) -> Measurement<Vec<f64>> {
    assert_eq!(view.is_empty(), view.len().checked_sub(1).is_none());
    assert!(view.get(view.len()).is_none());
    let values = (0..view.len())
        .map(|index| view.get(index).expect("index below len must exist"))
        .collect::<Vec<_>>();
    let value = if view.status() == "derived" {
        Some(values)
    } else {
        assert_eq!(view.len(), 0, "an unknown list exposes no fallback values");
        None
    };
    Measurement {
        status: view.status(),
        reason: view.reason(),
        value,
    }
}

fn capture_scalar(view: ScanLayoutScalarMeasurementView<'_>) -> Measurement<f64> {
    Measurement {
        status: view.status(),
        reason: view.reason(),
        value: view.value(),
    }
}

fn capture_strings(view: ScanLayoutStringListView<'_>) -> Vec<String> {
    assert_eq!(view.is_empty(), view.len().checked_sub(1).is_none());
    assert!(view.get(view.len()).is_none());
    (0..view.len())
        .map(|index| {
            view.get(index)
                .expect("index below len must exist")
                .to_string()
        })
        .collect()
}

fn capture_region(view: ScanLayoutRegionView<'_>) -> RegionLedger {
    RegionLedger {
        id: view.region_id().to_string(),
        support_ids: capture_strings(view.line_support_ids()),
        envelope: capture_box(view.observed_line_envelope()),
        starts: capture_numbers(view.line_starts_pt()),
        gaps: capture_numbers(view.signed_line_box_gaps_pt()),
        delta: capture_scalar(view.first_line_start_delta_pt()),
    }
}

fn capture_page(view: ScanLayoutPageView<'_>) -> PageLedger {
    PageLedger {
        status: view.status(),
        reason: view.reason(),
        frame_id: view.frame_id().map(str::to_string),
        extent: view
            .extent_pt()
            .map(|extent| (extent.width(), extent.height())),
        max_error: view.max_error_pt(),
    }
}

fn capture_profile(profile: &ScanLayoutProfile) -> ProfileLedger {
    let coverage = profile.coverage();
    let regions = profile.regions();
    assert_eq!(regions.is_empty(), regions.len().checked_sub(1).is_none());
    assert!(regions.get(regions.len()).is_none());
    ProfileLedger {
        page_index: profile.source().page_index(),
        raster_sha256: profile.source().raster_sha256().to_string(),
        total_lines: coverage.total_lines(),
        projected_lines: capture_count(coverage.projected_lines()),
        page: capture_page(profile.page()),
        global_envelope: capture_box(profile.observed_text_envelope()),
        global_insets: capture_insets(profile.observed_text_insets()),
        regions: (0..regions.len())
            .map(|index| capture_region(regions.get(index).expect("index below len must exist")))
            .collect(),
    }
}

fn all_unknown_region(id: &str, support_ids: &[&str], reason: &'static str) -> RegionLedger {
    RegionLedger {
        id: id.to_string(),
        support_ids: support_ids.iter().map(|id| (*id).to_string()).collect(),
        envelope: unavailable(reason),
        starts: unavailable(reason),
        gaps: unavailable(reason),
        delta: unavailable(reason),
    }
}

#[test]
fn source_and_unknown_page_keep_observation_identity_and_override_dependent_reasons() {
    let units = vec![
        region("region-u", Some(0)),
        line(
            "line-child",
            Some("region-u"),
            Some(0),
            Some((10.0, 20.0, 30.0, 40.0)),
        ),
        line("line-top", None, Some(1), Some((50.0, 60.0, 70.0, 80.0))),
    ];
    let profile = derive_scan_layout(&observation(
        4_000_000_001,
        DISTINCT_SHA256,
        unknown(),
        units,
    ))
    .expect("page-mapping Unknown is a valid semantic outcome");
    let actual = capture_profile(&profile);

    assert_eq!(actual.page_index, 4_000_000_001);
    assert_eq!(actual.raster_sha256, DISTINCT_SHA256);
    assert_eq!(actual.total_lines, 2);
    assert_eq!(actual.projected_lines, unavailable("page-mapping-unknown"));
    assert_eq!(
        actual.page,
        PageLedger {
            status: "unknown",
            reason: Some("page-mapping-unknown"),
            frame_id: None,
            extent: None,
            max_error: None,
        }
    );
    assert_eq!(actual.global_envelope, unavailable("page-mapping-unknown"));
    assert_eq!(actual.global_insets, unavailable("page-mapping-unknown"));
    assert_eq!(
        actual.regions,
        vec![all_unknown_region(
            "region-u",
            &["line-child"],
            "page-mapping-unknown",
        )]
    );
}

#[test]
fn identity_projection_preserves_signed_insets_and_max_error_does_not_expand_geometry() {
    let profile = derive_scan_layout(&observation(
        7,
        RASTER_SHA256,
        identity_mapping((100.0, 100.0), 7.25),
        vec![line(
            "line-outside-page",
            None,
            Some(0),
            Some((5.0, 10.0, 120.0, 140.0)),
        )],
    ))
    .expect("identity projection is valid");
    let actual = capture_profile(&profile);

    assert_eq!(actual.total_lines, 1);
    assert_eq!(actual.projected_lines, derived(1));
    assert_eq!(
        actual.page,
        PageLedger {
            status: "derived",
            reason: None,
            frame_id: Some("source-page-pt".to_string()),
            extent: Some((100.0, 100.0)),
            max_error: Some(7.25),
        }
    );
    assert_eq!(
        actual.global_envelope,
        derived(BoxLedger {
            x0: 5.0,
            y0: 10.0,
            x1: 120.0,
            y1: 140.0,
        })
    );
    assert_eq!(
        actual.global_insets,
        derived(InsetsLedger {
            top: 10.0,
            left: 5.0,
            right: -20.0,
            bottom: -40.0,
        })
    );
}

#[test]
fn non_affine_projection_envelopes_all_four_corners_not_a_fixed_diagonal() {
    let projective = [2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.1, 0.2, 1.0];
    let profile = derive_scan_layout(&observation(
        0,
        RASTER_SHA256,
        known_mapping(projective, (100.0, 200.0), 0.0),
        vec![
            region("projective-region", Some(0)),
            line(
                "projective-line",
                Some("projective-region"),
                Some(0),
                Some((10.0, 20.0, 30.0, 40.0)),
            ),
        ],
    ))
    .expect("all four denominators are finite and nonzero");
    let actual = capture_profile(&profile);

    // The extrema come from the two off-diagonal corners:
    // (30,20)->(7.5,7.5) and (10,40)->(2,12).
    let four_corner_envelope = BoxLedger {
        x0: 2.0,
        y0: 7.5,
        x1: 7.5,
        y1: 12.0,
    };
    assert_eq!(actual.global_envelope, derived(four_corner_envelope));
    assert_eq!(actual.regions.len(), 1);
    assert_eq!(
        actual.regions[0].envelope,
        derived(BoxLedger {
            x0: 2.0,
            y0: 7.5,
            x1: 7.5,
            y1: 12.0,
        })
    );
    assert_eq!(actual.regions[0].starts, derived(vec![2.0]));
    assert_eq!(actual.regions[0].gaps, derived(Vec::new()));
    assert_eq!(actual.regions[0].delta, unavailable("insufficient-support"));
}

#[test]
fn direct_l1_rejects_a_late_fourth_corner_on_the_horizon() {
    let horizon_mapping = known_mapping(
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, -7.0],
        (100.0, 100.0),
        0.0,
    );
    let invalid = observation(
        0,
        RASTER_SHA256,
        horizon_mapping.clone(),
        vec![
            line(
                "early-valid-line",
                None,
                Some(0),
                Some((10.0, 10.0, 20.0, 20.0)),
            ),
            line(
                "late-horizon-line",
                None,
                Some(1),
                Some((1.0, 1.0, 3.0, 4.0)),
            ),
        ],
    );
    assert!(
        derive_scan_layout(&invalid).is_err(),
        "the fourth corner (3,4) has denominator 3+4-7 = 0"
    );

    let valid_control = observation(
        0,
        RASTER_SHA256,
        horizon_mapping,
        vec![
            line(
                "early-valid-line",
                None,
                Some(0),
                Some((10.0, 10.0, 20.0, 20.0)),
            ),
            line(
                "late-finite-line",
                None,
                Some(1),
                Some((1.0, 1.0, 2.0, 3.0)),
            ),
        ],
    );
    assert!(derive_scan_layout(&valid_control).is_ok());
}

#[test]
fn post_validation_overflow_is_projection_failed_and_isolated_from_a_good_region() {
    let overflow_after_validation = known_mapping(
        [f64::MAX, 0.0, 0.0, 0.0, 1.0e-308, 0.0, 0.0, 0.0, 1.0],
        (100.0, 100.0),
        0.0,
    );
    let profile = derive_scan_layout(&observation(
        0,
        RASTER_SHA256,
        overflow_after_validation,
        vec![
            region("good", Some(0)),
            region("overflow", Some(1)),
            line(
                "good-line",
                Some("good"),
                Some(0),
                Some((0.25, 10.0, 1.0, 20.0)),
            ),
            line(
                "overflow-line",
                Some("overflow"),
                Some(0),
                Some((2.0, 30.0, 3.0, 40.0)),
            ),
        ],
    ))
    .expect("finite nonsingular mapping validates before projection overflows");
    let actual = capture_profile(&profile);

    assert_eq!(actual.total_lines, 2);
    assert_eq!(actual.projected_lines, derived(1));
    assert_eq!(actual.global_envelope, unavailable("projection-failed"));
    assert_eq!(actual.global_insets, unavailable("projection-failed"));
    assert_eq!(actual.regions[0].id, "good");
    assert_eq!(actual.regions[0].envelope.status, "derived");
    assert_eq!(
        actual.regions[1],
        all_unknown_region("overflow", &["overflow-line"], "projection-failed")
    );
}

#[test]
fn global_r0_ri_r1_rn_have_distinct_coverage_and_cardinality() {
    let derive = |units| {
        let profile = derive_scan_layout(&observation(
            0,
            RASTER_SHA256,
            identity_mapping((100.0, 100.0), 0.0),
            units,
        ))
        .expect("fixture must derive");
        capture_profile(&profile)
    };

    let r0 = derive(Vec::new());
    assert_eq!(r0.total_lines, 0);
    assert_eq!(r0.projected_lines, derived(0));
    assert_eq!(r0.global_envelope, unavailable("no-lines"));
    assert_eq!(r0.global_insets, unavailable("no-lines"));

    let ri = derive(vec![line("missing-box", None, Some(0), None)]);
    assert_eq!(ri.total_lines, 1);
    assert_eq!(ri.projected_lines, derived(0));
    assert_eq!(ri.global_envelope, unavailable("incomplete-line-bbox"));
    assert_eq!(ri.global_insets, unavailable("incomplete-line-bbox"));

    let r1 = derive(vec![line(
        "one",
        None,
        Some(0),
        Some((10.0, 20.0, 30.0, 40.0)),
    )]);
    assert_eq!(r1.total_lines, 1);
    assert_eq!(r1.projected_lines, derived(1));
    assert_eq!(
        r1.global_envelope,
        derived(BoxLedger {
            x0: 10.0,
            y0: 20.0,
            x1: 30.0,
            y1: 40.0,
        })
    );

    let rn = derive(vec![
        line("second", None, Some(1), Some((40.0, 50.0, 80.0, 90.0))),
        line("first", None, Some(0), Some((5.0, 10.0, 20.0, 30.0))),
    ]);
    assert_eq!(rn.total_lines, 2);
    assert_eq!(rn.projected_lines, derived(2));
    assert_eq!(
        rn.global_envelope,
        derived(BoxLedger {
            x0: 5.0,
            y0: 10.0,
            x1: 80.0,
            y1: 90.0,
        })
    );
}

#[test]
fn regional_r0_ri_r1_rn_reasons_ordering_signed_gaps_and_cardinality_follow_one_expected_profile() {
    let units = vec![
        region("r-rn", Some(40)),
        line(
            "rn-3",
            Some("r-rn"),
            Some(30),
            Some((60.0, 25.0, 70.0, 30.0)),
        ),
        region_with_bbox("r-r0", Some(10), (100.0, 100.0, 200.0, 200.0)),
        region("z-ri-order", None),
        line(
            "z-child",
            Some("z-ri-order"),
            None,
            Some((10.0, 50.0, 20.0, 60.0)),
        ),
        region("r-r1", Some(30)),
        line(
            "single",
            Some("r-r1"),
            Some(0),
            Some((10.0, 10.0, 20.0, 20.0)),
        ),
        line(
            "rn-1",
            Some("r-rn"),
            Some(10),
            Some((50.0, 10.0, 55.0, 20.0)),
        ),
        region("é-ri-both", None),
        line("both", Some("é-ri-both"), None, None),
        region("r-ri-bbox", Some(20)),
        line("bbox-missing", Some("r-ri-bbox"), Some(0), None),
        line(
            "rn-4",
            Some("r-rn"),
            Some(40),
            Some((55.0, 35.0, 65.0, 40.0)),
        ),
        line(
            "a-child",
            Some("z-ri-order"),
            Some(99),
            Some((30.0, 70.0, 40.0, 80.0)),
        ),
        line(
            "rn-2",
            Some("r-rn"),
            Some(20),
            Some((40.0, 15.0, 50.0, 25.0)),
        ),
    ];
    let source = observation(
        0,
        RASTER_SHA256,
        identity_mapping((300.0, 300.0), 0.0),
        units,
    );
    let profile = derive_scan_layout(&source).expect("mixed regional states are valid");
    let actual = capture_profile(&profile);

    let expected_regions = vec![
        all_unknown_region("r-r0", &[], "no-lines"),
        all_unknown_region("r-ri-bbox", &["bbox-missing"], "incomplete-line-bbox"),
        RegionLedger {
            id: "r-r1".to_string(),
            support_ids: vec!["single".to_string()],
            envelope: derived(BoxLedger {
                x0: 10.0,
                y0: 10.0,
                x1: 20.0,
                y1: 20.0,
            }),
            starts: derived(vec![10.0]),
            gaps: derived(Vec::new()),
            delta: unavailable("insufficient-support"),
        },
        RegionLedger {
            id: "r-rn".to_string(),
            support_ids: ["rn-1", "rn-2", "rn-3", "rn-4"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            envelope: derived(BoxLedger {
                x0: 40.0,
                y0: 10.0,
                x1: 70.0,
                y1: 40.0,
            }),
            starts: derived(vec![50.0, 40.0, 60.0, 55.0]),
            gaps: derived(vec![-5.0, 0.0, 5.0]),
            delta: derived(10.0),
        },
        all_unknown_region(
            "z-ri-order",
            &["a-child", "z-child"],
            "incomplete-line-reading-order",
        ),
        all_unknown_region("é-ri-both", &["both"], "incomplete-line-reading-order"),
    ];

    assert_eq!(actual.total_lines, 9);
    assert_eq!(actual.projected_lines, derived(7));
    assert_eq!(actual.global_envelope, unavailable("incomplete-line-bbox"));
    assert_eq!(actual.regions, expected_regions);

    let mut permuted = source;
    permuted.units.reverse();
    let permuted_profile = derive_scan_layout(&permuted).expect("unit order is not semantic");
    assert_eq!(capture_profile(&permuted_profile), actual);
}

#[test]
fn global_lines_include_top_level_and_region_children_but_regions_use_only_direct_line_children() {
    let units = vec![
        region("region-a", Some(0)),
        region("region-b", Some(1)),
        region_with_bbox("empty-region", Some(2), (300.0, 300.0, 900.0, 900.0)),
        line(
            "line-a",
            Some("region-a"),
            Some(0),
            Some((20.0, 20.0, 30.0, 30.0)),
        ),
        descendant(
            "word-a",
            ObservationKind::Word,
            "line-a",
            (2.0, 2.0, 8.0, 8.0),
        ),
        descendant(
            "glyph-a",
            ObservationKind::Glyph,
            "word-a",
            (3.0, 3.0, 4.0, 4.0),
        ),
        line(
            "line-b",
            Some("region-b"),
            Some(0),
            Some((40.0, 40.0, 50.0, 50.0)),
        ),
        line("line-top", None, Some(3), Some((1.0, 10.0, 10.0, 15.0))),
    ];
    let profile = derive_scan_layout(&observation(
        0,
        RASTER_SHA256,
        identity_mapping((100.0, 100.0), 0.0),
        units,
    ))
    .expect("hierarchy fixture is valid");
    let actual = capture_profile(&profile);

    assert_eq!(actual.total_lines, 3);
    assert_eq!(actual.projected_lines, derived(3));
    assert_eq!(
        actual.global_envelope,
        derived(BoxLedger {
            x0: 1.0,
            y0: 10.0,
            x1: 50.0,
            y1: 50.0,
        })
    );
    assert_eq!(actual.regions.len(), 3);
    assert_eq!(actual.regions[0].support_ids, vec!["line-a"]);
    assert_eq!(actual.regions[1].support_ids, vec!["line-b"]);
    assert_eq!(
        actual.regions[2],
        all_unknown_region("empty-region", &[], "no-lines")
    );
}
