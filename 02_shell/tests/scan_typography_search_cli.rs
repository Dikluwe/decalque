//! Oráculo independente de L2 para a grade fechada de busca tipográfica.
//!
//! Contrato/intenção corrigidos no commit
//! `3ea4903ca154cd5198eeff7d9bceea1ea52ea123`; baseline de produto
//! `f59122681f9afb7c7272490b290213ebb33c1371`.

use decalque_core::{
    ConfidenceRequirement, EvidenceStatus, FontStyleHypothesis, FontWeightHypothesis,
    ScanGranularity, TextNormalization, TypographyHypothesis,
};
use decalque_shell::{
    parse_scan_typography_search_args, render_scan_typography_search_report,
    ScanTypographySearchCliArgs, ScanTypographySearchCliParseError,
    ScanTypographySearchCoverageContext, ScanTypographySearchEligibilityContext,
    ScanTypographySearchReportContext, ScanTypographySearchScoreContext,
    ScanTypographySearchSearchSpaceContext, ScanTypographySearchSelectionContext,
    ScanTypographySearchSupportContext, ScanTypographySearchTrialContext,
};
use std::ffi::OsString;

fn valid_args() -> Vec<OsString> {
    [
        "observation.json",
        "--raster",
        "page.png",
        "--output-pdf",
        "winner.pdf",
        "--font-family",
        "Libertinus Serif",
        "--font-size-pt",
        "10",
        "--horizontal-tolerance-pt",
        "0.5",
        "--baseline-tolerance-pt",
        "0.25",
    ]
    .map(OsString::from)
    .into()
}

fn remove_option(mut args: Vec<OsString>, option: &str) -> Vec<OsString> {
    let index = args
        .iter()
        .position(|argument| argument == option)
        .unwrap_or_else(|| panic!("fixture does not contain {option}"));
    args.drain(index..=index + 1);
    args
}

fn remove_option_if_present(mut args: Vec<OsString>, option: &str) -> Vec<OsString> {
    if let Some(index) = args.iter().position(|argument| argument == option) {
        args.drain(index..=index + 1);
    }
    args
}

fn replace_option_value(args: &mut [OsString], option: &str, value: OsString) {
    let index = args
        .iter()
        .position(|argument| argument == option)
        .unwrap_or_else(|| panic!("fixture does not contain {option}"));
    args[index + 1] = value;
}

fn args_with_grid(font_sizes: &[&str], trackings: Option<&[&str]>) -> Vec<OsString> {
    let mut args = remove_option(valid_args(), "--font-size-pt");

    for font_size in font_sizes {
        args.push(OsString::from("--font-size-pt"));
        args.push(OsString::from(*font_size));
    }

    if let Some(trackings) = trackings {
        for tracking in trackings {
            args.push(OsString::from("--tracking-pt"));
            args.push(OsString::from(*tracking));
        }
    }

    args
}

fn parse_ok(args: Vec<OsString>, context: &str) -> ScanTypographySearchCliArgs {
    match parse_scan_typography_search_args(args) {
        Ok(parsed) => parsed,
        Err(error) => panic!("{context} should parse successfully: {error}"),
    }
}

fn assert_rejected(args: Vec<OsString>, context: &str) {
    if parse_scan_typography_search_args(args).is_ok() {
        panic!("{context} should be rejected");
    }
}

fn assert_error_mentions(args: Vec<OsString>, expected: &str, context: &str) {
    let error: ScanTypographySearchCliParseError = match parse_scan_typography_search_args(args) {
        Ok(_) => panic!("{context} should be rejected"),
        Err(error) => error,
    };
    let rendered = error.to_string();
    assert!(
        rendered.contains(expected),
        "{context} should mention {expected:?}, got {rendered:?}"
    );
}

fn grid_pairs(args: &ScanTypographySearchCliArgs) -> Vec<(f64, f64)> {
    args.typography_grid
        .iter()
        .map(|hypothesis| (hypothesis.size_pt, hypothesis.tracking_pt))
        .collect()
}

#[test]
fn requires_observation_and_every_mandatory_option() {
    let mut missing_observation = valid_args();
    missing_observation.remove(0);
    assert_rejected(missing_observation, "missing observation path");

    for option in [
        "--raster",
        "--output-pdf",
        "--font-family",
        "--font-size-pt",
        "--horizontal-tolerance-pt",
        "--baseline-tolerance-pt",
    ] {
        assert_rejected(
            remove_option(valid_args(), option),
            &format!("missing mandatory option {option}"),
        );
    }
}

#[test]
fn permits_repetition_only_for_font_size_and_tracking() {
    let parsed = parse_ok(
        args_with_grid(&["18", "10"], Some(&["0.5", "-0.5"])),
        "repeated grid options",
    );
    assert_eq!(
        grid_pairs(&parsed),
        vec![(10.0, -0.5), (10.0, 0.5), (18.0, -0.5), (18.0, 0.5)]
    );

    let mut fully_explicit = valid_args();
    fully_explicit.extend(
        [
            "--font-weight",
            "bold",
            "--font-style",
            "italic",
            "--min-text-confidence",
            "0.8",
            "--min-geometry-confidence",
            "0.7",
            "--typst-bin",
            "typst-custom",
        ]
        .map(OsString::from),
    );

    for (option, value) in [
        ("--raster", "other.png"),
        ("--output-pdf", "other.pdf"),
        ("--font-family", "Other Family"),
        ("--font-weight", "regular"),
        ("--font-style", "oblique"),
        ("--horizontal-tolerance-pt", "0.75"),
        ("--baseline-tolerance-pt", "0.75"),
        ("--min-text-confidence", "0.6"),
        ("--min-geometry-confidence", "0.6"),
        ("--typst-bin", "other-typst"),
    ] {
        let mut repeated = fully_explicit.clone();
        repeated.extend([OsString::from(option), OsString::from(value)]);
        assert_rejected(repeated, &format!("repeated option {option}"));
    }

    let mut extra_positional = valid_args();
    extra_positional.push(OsString::from("second-observation.json"));
    assert_rejected(extra_positional, "a second positional argument");
}

#[test]
fn parses_fixed_typography_policy_confidences_and_opaque_native_paths() {
    let observation = OsString::from("__l2_opaque__/observation.json");
    let raster = OsString::from("__l2_opaque__/page.raster");
    let output_pdf = OsString::from("__l2_opaque__/winner.pdf");
    let typst_bin = OsString::from("__l2_must_not_execute__/typst with spaces");
    let mut args = vec![
        observation.clone(),
        OsString::from("--raster"),
        raster.clone(),
        OsString::from("--output-pdf"),
        output_pdf.clone(),
        OsString::from("--font-family"),
        OsString::from("Family With Spaces"),
        OsString::from("--font-size-pt"),
        OsString::from("18"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("0.75"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("0.125"),
    ];
    args.extend(
        [
            "--font-weight",
            "bold",
            "--font-style",
            "oblique",
            "--tracking-pt",
            "-0.25",
            "--min-text-confidence",
            "0.875",
            "--min-geometry-confidence",
            "0.625",
            "--typst-bin",
        ]
        .map(OsString::from),
    );
    args.push(typst_bin.clone());

    let parsed = parse_ok(args, "fully explicit fixed search context");
    assert_eq!(parsed.observation.as_os_str(), observation.as_os_str());
    assert_eq!(parsed.raster.as_os_str(), raster.as_os_str());
    assert_eq!(parsed.output_pdf.as_os_str(), output_pdf.as_os_str());
    assert_eq!(parsed.typst_bin.as_os_str(), typst_bin.as_os_str());
    assert_eq!(parsed.typography_grid.len(), 1);

    let hypothesis = &parsed.typography_grid[0];
    assert_eq!(hypothesis.font_family, "Family With Spaces");
    assert_eq!(hypothesis.size_pt, 18.0);
    assert_eq!(hypothesis.weight, FontWeightHypothesis::Bold);
    assert_eq!(hypothesis.style, FontStyleHypothesis::Oblique);
    assert_eq!(hypothesis.tracking_pt, -0.25);

    assert_eq!(parsed.policy.granularity, ScanGranularity::Line);
    assert_eq!(parsed.policy.horizontal_tolerance_pt, 0.75);
    assert_eq!(parsed.policy.baseline_tolerance_pt, 0.125);
    assert_eq!(
        parsed.policy.text_confidence,
        ConfidenceRequirement::KnownAtLeast(0.875)
    );
    assert_eq!(
        parsed.policy.geometry_confidence,
        ConfidenceRequirement::KnownAtLeast(0.625)
    );
    assert_eq!(parsed.policy.text_normalization, TextNormalization::Exact);
}

#[test]
fn canonicalizes_the_grid_independently_of_input_order() {
    let first = parse_ok(
        args_with_grid(&["18", "4", "96", "10"], Some(&["2", "-0", "-2", "0.5"])),
        "first permutation",
    );
    let second = parse_ok(
        args_with_grid(&["96", "10", "4", "18"], Some(&["0.5", "-2", "0", "2"])),
        "second permutation",
    );

    assert_eq!(first.typography_grid, second.typography_grid);
    assert_eq!(
        grid_pairs(&first),
        vec![
            (4.0, -2.0),
            (4.0, 0.0),
            (4.0, 0.5),
            (4.0, 2.0),
            (10.0, -2.0),
            (10.0, 0.0),
            (10.0, 0.5),
            (10.0, 2.0),
            (18.0, -2.0),
            (18.0, 0.0),
            (18.0, 0.5),
            (18.0, 2.0),
            (96.0, -2.0),
            (96.0, 0.0),
            (96.0, 0.5),
            (96.0, 2.0),
        ]
    );

    assert!(first
        .typography_grid
        .iter()
        .all(|hypothesis| hypothesis.font_family == "Libertinus Serif"));
}

#[test]
fn normalizes_negative_zero_and_rejects_duplicates_after_normalization() {
    let parsed = parse_ok(
        args_with_grid(&["10"], Some(&["-0"])),
        "negative-zero tracking",
    );
    assert_eq!(parsed.typography_grid.len(), 1);
    assert_eq!(
        parsed.typography_grid[0].tracking_pt.to_bits(),
        0.0_f64.to_bits()
    );

    assert_rejected(args_with_grid(&["10", "10"], None), "identical font sizes");
    assert_rejected(
        args_with_grid(&["10", "10.0"], None),
        "numerically equivalent font sizes",
    );
    assert_rejected(
        args_with_grid(&["10"], Some(&["0.5", "0.50"])),
        "numerically equivalent trackings",
    );
    assert_rejected(
        args_with_grid(&["10"], Some(&["-0", "0"])),
        "signed-zero tracking duplicates",
    );
}

#[test]
fn accepts_closed_range_boundaries_and_rejects_out_of_range_or_non_finite_values() {
    let parsed = parse_ok(
        args_with_grid(&["96", "4"], Some(&["2", "-2"])),
        "closed range boundaries",
    );
    assert_eq!(
        grid_pairs(&parsed),
        vec![(4.0, -2.0), (4.0, 2.0), (96.0, -2.0), (96.0, 2.0)]
    );

    for value in [
        "3.999999999999",
        "96.000000000001",
        "NaN",
        "inf",
        "-inf",
        "1e9999",
    ] {
        assert_rejected(
            args_with_grid(&[value], None),
            &format!("invalid --font-size-pt {value}"),
        );
    }

    for value in [
        "-2.000000000001",
        "2.000000000001",
        "NaN",
        "inf",
        "-inf",
        "1e9999",
    ] {
        assert_rejected(
            args_with_grid(&["10"], Some(&[value])),
            &format!("invalid --tracking-pt {value}"),
        );
    }
}

#[test]
fn defaults_fixed_typography_and_tracking_to_regular_normal_positive_zero() {
    let parsed = parse_ok(valid_args(), "omitted fixed typography and tracking");
    assert_eq!(
        parsed.typography_grid[0].weight,
        FontWeightHypothesis::Regular
    );
    assert_eq!(parsed.typography_grid[0].style, FontStyleHypothesis::Normal);
    assert_eq!(grid_pairs(&parsed), vec![(10.0, 0.0)]);
    assert_eq!(
        parsed.typography_grid[0].tracking_pt.to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn accepts_32_hypotheses_and_rejects_33_while_paths_are_still_opaque() {
    let thirty_two = parse_ok(
        args_with_grid(
            &["4", "5", "6", "7", "8", "9", "10", "11"],
            Some(&["-2", "-1", "0", "1"]),
        ),
        "8 x 4 grid",
    );
    assert_eq!(thirty_two.typography_grid.len(), 32);
    assert_eq!(grid_pairs(&thirty_two).first(), Some(&(4.0, -2.0)));
    assert_eq!(grid_pairs(&thirty_two).last(), Some(&(11.0, 1.0)));

    let mut thirty_three = args_with_grid(
        &["4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14"],
        Some(&["-2", "0", "2"]),
    );
    thirty_three[0] = OsString::from("__l2_must_not_read__/observation.json");
    replace_option_value(
        &mut thirty_three,
        "--raster",
        OsString::from("__l2_must_not_read__/page.png"),
    );
    replace_option_value(
        &mut thirty_three,
        "--output-pdf",
        OsString::from("__l2_must_not_write__/winner.pdf"),
    );
    thirty_three.extend(["--typst-bin", "__l2_must_not_execute__/typst"].map(OsString::from));

    assert_error_mentions(
        thirty_three,
        "32",
        "11 x 3 grid must close and fail before any path I/O",
    );
}

#[cfg(unix)]
#[test]
fn preserves_non_utf8_paths_and_rejects_non_utf8_text() {
    use std::os::unix::ffi::OsStringExt;

    let observation = OsString::from_vec(b"observation-\xff.json".to_vec());
    let raster = OsString::from_vec(b"page-\xfe.png".to_vec());
    let output_pdf = OsString::from_vec(b"winner-\xfd.pdf".to_vec());
    let typst_bin = OsString::from_vec(b"typst-\xfc".to_vec());

    let args = vec![
        observation.clone(),
        OsString::from("--raster"),
        raster.clone(),
        OsString::from("--output-pdf"),
        output_pdf.clone(),
        OsString::from("--font-family"),
        OsString::from("Libertinus Serif"),
        OsString::from("--font-size-pt"),
        OsString::from("10"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("0.5"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("0.25"),
        OsString::from("--typst-bin"),
        typst_bin.clone(),
    ];

    let parsed = parse_ok(args, "non-UTF-8 native paths");
    assert_eq!(parsed.observation.as_os_str(), observation.as_os_str());
    assert_eq!(parsed.raster.as_os_str(), raster.as_os_str());
    assert_eq!(parsed.output_pdf.as_os_str(), output_pdf.as_os_str());
    assert_eq!(parsed.typst_bin.as_os_str(), typst_bin.as_os_str());

    for option in ["--font-family", "--font-size-pt", "--tracking-pt"] {
        let mut args = if option == "--tracking-pt" {
            let mut args = valid_args();
            args.extend([OsString::from(option), OsString::from("0")]);
            args
        } else {
            valid_args()
        };
        replace_option_value(
            &mut args,
            option,
            OsString::from_vec(b"not-text-\xff".to_vec()),
        );
        assert_rejected(args, &format!("non-UTF-8 text for {option}"));
    }
}

#[test]
fn rejects_unknown_options_and_options_without_values() {
    for (option, value) in [
        ("--granularity", "line"),
        ("--adaptive-step", "0.1"),
        ("--candidate", "candidate.pdf"),
    ] {
        let mut args = valid_args();
        args.extend([OsString::from(option), OsString::from(value)]);
        assert_error_mentions(args, option, &format!("unknown option {option}"));
    }

    for option in [
        "--raster",
        "--output-pdf",
        "--font-family",
        "--font-size-pt",
        "--font-weight",
        "--font-style",
        "--tracking-pt",
        "--horizontal-tolerance-pt",
        "--baseline-tolerance-pt",
        "--min-text-confidence",
        "--min-geometry-confidence",
        "--typst-bin",
    ] {
        let mut args = remove_option_if_present(valid_args(), option);
        args.push(OsString::from(option));
        assert_error_mentions(args, option, &format!("option without a value: {option}"));
    }
}

const COMPLETE_WINNER_JSON: &str = concat!(
    "{\"schema\":\"decalque.scan-reconstruction-evaluation\",\"schema_version\":1,",
    "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
    "\"hypothesis\":{\"font_family\":\"Winner Family\",\"font_size_pt\":10,",
    "\"font_weight\":\"bold\",\"font_style\":\"oblique\",\"tracking_pt\":0},",
    "\"compiler_version\":\"typst 1\",\"source_sha256\":\"winner-source\",",
    "\"source_size_bytes\":17,\"pdf_sha256\":\"winner-pdf\",\"pdf_size_bytes\":29,",
    "\"comparison\":{\"schema\":\"decalque.scan-comparison-report\",",
    "\"schema_version\":1,\"source\":{\"page_index\":7,",
    "\"raster_sha256\":\"raster-hash\"},\"policy\":{\"granularity\":\"line\",",
    "\"text_normalization\":\"exact\",\"horizontal_tolerance_pt\":0.5,",
    "\"baseline_tolerance_pt\":0.25,\"min_text_confidence\":",
    "{\"status\":\"not-required\"},\"min_geometry_confidence\":",
    "{\"status\":\"not-required\"}},\"content_status\":\"preserved\",",
    "\"geometry_status\":\"preserved\",\"overall_status\":\"preserved\",",
    "\"coverage\":{\"matched_scan\":1,\"total_scan\":1,\"matched_candidate\":1,",
    "\"total_candidate\":1},\"matches\":[],\"unmatched_scan\":[],",
    "\"unmatched_candidate\":[],\"reflow\":[],\"observation_diagnostics\":[],",
    "\"pdf_diagnostics\":[],\"comparison_diagnostics\":[]}}"
);

fn assert_no_paths_or_timestamps(rendered: &str) {
    for forbidden in [
        "\"path\":",
        "\"timestamp\":",
        "\"created_at\":",
        "\"duration\":",
        "\"stderr\":",
        "\"directory\":",
        "\"cwd\":",
        "\"observation\":",
        "\"raster_path\":",
        "\"output_pdf\":",
        "\"typst_bin\":",
        "observation.json",
        "winner.pdf",
        "__l2_opaque__",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "report must not expose {forbidden:?}: {rendered}"
        );
    }
}

#[test]
fn renders_selected_v1_as_an_exact_closed_snapshot_with_the_integral_winner() {
    let font_sizes = [10.0, 18.0];
    let trackings = [0.0];
    let family = "Family \"A\"\\B\nC";
    let first_hypothesis = TypographyHypothesis {
        font_family: family.to_string(),
        size_pt: 10.0,
        weight: FontWeightHypothesis::Bold,
        style: FontStyleHypothesis::Oblique,
        tracking_pt: 0.0,
    };
    let second_hypothesis = TypographyHypothesis {
        font_family: family.to_string(),
        size_pt: 18.0,
        weight: FontWeightHypothesis::Bold,
        style: FontStyleHypothesis::Oblique,
        tracking_pt: 0.0,
    };
    let horizontal_ids = ["line-\"a\"\\\n"];
    let baseline_ids = ["line-\"a\"\\\n"];
    let horizontal_residuals = [u64::MAX, 2_u64];
    let baseline_residuals = [3_u64];
    let trials = [
        ScanTypographySearchTrialContext {
            index: 0_u64,
            hypothesis: &first_hypothesis,
            source_sha256: "source-0",
            source_size_bytes: u64::MAX,
            pdf_sha256: "pdf-0",
            pdf_size_bytes: 29_u64,
            content_status: EvidenceStatus::Preserved,
            geometry_status: EvidenceStatus::Violated,
            overall_status: EvidenceStatus::Unknown,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 1_u64,
                total_scan: 1_u64,
                matched_candidate: 1_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Eligible {
                support: ScanTypographySearchSupportContext {
                    horizontal_scan_unit_ids: &horizontal_ids,
                    known_baseline_scan_unit_ids: &baseline_ids,
                },
                score: ScanTypographySearchScoreContext {
                    horizontal_violations: 1_u64,
                    horizontal_residuals_desc: &horizontal_residuals,
                    baseline_violations: 0_u64,
                    baseline_residuals_desc: &baseline_residuals,
                },
            },
        },
        ScanTypographySearchTrialContext {
            index: 1_u64,
            hypothesis: &second_hypothesis,
            source_sha256: "source-1",
            source_size_bytes: 31_u64,
            pdf_sha256: "pdf-1",
            pdf_size_bytes: 37_u64,
            content_status: EvidenceStatus::Unknown,
            geometry_status: EvidenceStatus::Unknown,
            overall_status: EvidenceStatus::Unknown,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 0_u64,
                total_scan: 1_u64,
                matched_candidate: 0_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Ineligible {
                reason: "missing-horizontal-evidence",
            },
        },
    ];
    let context = ScanTypographySearchReportContext {
        page_index: 7_u64,
        raster_sha256: "raster-hash",
        search_space: ScanTypographySearchSearchSpaceContext {
            font_family: family,
            font_weight: FontWeightHypothesis::Bold,
            font_style: FontStyleHypothesis::Oblique,
            font_sizes_pt: &font_sizes,
            trackings_pt: &trackings,
            hypothesis_count: 2_u64,
        },
        compiler_version: "typst \"dev\"\\build\n",
        trials: &trials,
        selection: ScanTypographySearchSelectionContext::Selected {
            selected_index: 0_u64,
            evidence_scope: "horizontal-and-observed-baseline",
            winner_json: COMPLETE_WINNER_JSON,
        },
    };

    let rendered = render_scan_typography_search_report(&context).unwrap();
    let expected = [
        concat!(
            "{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1,",
            "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
            "\"search_space\":{\"font_family\":\"Family \\\"A\\\"\\\\B\\nC\",",
            "\"font_weight\":\"bold\",\"font_style\":\"oblique\",",
            "\"font_sizes_pt\":[10,18],\"trackings_pt\":[0],\"hypothesis_count\":2},",
            "\"compiler_version\":\"typst \\\"dev\\\"\\\\build\\n\",\"trials\":[",
            "{\"index\":0,\"hypothesis\":{\"font_family\":\"Family \\\"A\\\"\\\\B\\nC\",",
            "\"font_size_pt\":10,\"font_weight\":\"bold\",\"font_style\":\"oblique\",",
            "\"tracking_pt\":0},\"source_sha256\":\"source-0\",",
            "\"source_size_bytes\":18446744073709551615,\"pdf_sha256\":\"pdf-0\",",
            "\"pdf_size_bytes\":29,\"content_status\":\"preserved\",",
            "\"geometry_status\":\"violated\",\"overall_status\":\"unknown\",",
            "\"coverage\":{\"matched_scan\":1,\"total_scan\":1,\"matched_candidate\":1,",
            "\"total_candidate\":1},\"eligibility\":{\"status\":\"eligible\"},",
            "\"support\":{\"horizontal_scan_unit_ids\":[\"line-\\\"a\\\"\\\\\\n\"],",
            "\"known_baseline_scan_unit_ids\":[\"line-\\\"a\\\"\\\\\\n\"]},",
            "\"score\":{\"horizontal_violations\":1,",
            "\"horizontal_residuals_desc\":[18446744073709551615,2],",
            "\"baseline_violations\":0,\"baseline_residuals_desc\":[3]}},",
            "{\"index\":1,\"hypothesis\":{\"font_family\":\"Family \\\"A\\\"\\\\B\\nC\",",
            "\"font_size_pt\":18,\"font_weight\":\"bold\",\"font_style\":\"oblique\",",
            "\"tracking_pt\":0},\"source_sha256\":\"source-1\",\"source_size_bytes\":31,",
            "\"pdf_sha256\":\"pdf-1\",\"pdf_size_bytes\":37,",
            "\"content_status\":\"unknown\",\"geometry_status\":\"unknown\",",
            "\"overall_status\":\"unknown\",\"coverage\":{\"matched_scan\":0,",
            "\"total_scan\":1,\"matched_candidate\":0,\"total_candidate\":1},",
            "\"eligibility\":{\"status\":\"ineligible\",",
            "\"reason\":\"missing-horizontal-evidence\"},\"support\":null,\"score\":null}],",
            "\"selection\":{\"status\":\"selected\",\"selected_index\":0,",
            "\"evidence_scope\":\"horizontal-and-observed-baseline\",",
            "\"artifact_published\":true},\"winner\":"
        ),
        COMPLETE_WINNER_JSON,
        "}",
    ]
    .concat();

    assert_eq!(rendered, expected);
    assert!(!rendered.contains("\"winner_index\":"));
    assert!(!rendered.contains("\"indices\":"));
    assert_no_paths_or_timestamps(&rendered);
}

#[test]
fn renders_tied_v1_as_an_exact_closed_snapshot_with_null_winner() {
    let font_sizes = [10.0, 18.0];
    let trackings = [-0.25];
    let first_hypothesis = TypographyHypothesis {
        font_family: "Tie Family".to_string(),
        size_pt: 10.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Italic,
        tracking_pt: -0.25,
    };
    let second_hypothesis = TypographyHypothesis {
        font_family: "Tie Family".to_string(),
        size_pt: 18.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Italic,
        tracking_pt: -0.25,
    };
    let support_ids = ["line-1"];
    let no_baselines: [&str; 0] = [];
    let residuals = [4_u64];
    let no_baseline_residuals: [u64; 0] = [];
    let trials = [
        ScanTypographySearchTrialContext {
            index: 0_u64,
            hypothesis: &first_hypothesis,
            source_sha256: "tie-source-0",
            source_size_bytes: 11_u64,
            pdf_sha256: "tie-pdf-0",
            pdf_size_bytes: 21_u64,
            content_status: EvidenceStatus::Preserved,
            geometry_status: EvidenceStatus::Preserved,
            overall_status: EvidenceStatus::Preserved,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 1_u64,
                total_scan: 1_u64,
                matched_candidate: 1_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Eligible {
                support: ScanTypographySearchSupportContext {
                    horizontal_scan_unit_ids: &support_ids,
                    known_baseline_scan_unit_ids: &no_baselines,
                },
                score: ScanTypographySearchScoreContext {
                    horizontal_violations: 0_u64,
                    horizontal_residuals_desc: &residuals,
                    baseline_violations: 0_u64,
                    baseline_residuals_desc: &no_baseline_residuals,
                },
            },
        },
        ScanTypographySearchTrialContext {
            index: 1_u64,
            hypothesis: &second_hypothesis,
            source_sha256: "tie-source-1",
            source_size_bytes: 12_u64,
            pdf_sha256: "tie-pdf-1",
            pdf_size_bytes: 22_u64,
            content_status: EvidenceStatus::Preserved,
            geometry_status: EvidenceStatus::Violated,
            overall_status: EvidenceStatus::Violated,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 1_u64,
                total_scan: 1_u64,
                matched_candidate: 1_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Eligible {
                support: ScanTypographySearchSupportContext {
                    horizontal_scan_unit_ids: &support_ids,
                    known_baseline_scan_unit_ids: &no_baselines,
                },
                score: ScanTypographySearchScoreContext {
                    horizontal_violations: 0_u64,
                    horizontal_residuals_desc: &residuals,
                    baseline_violations: 0_u64,
                    baseline_residuals_desc: &no_baseline_residuals,
                },
            },
        },
    ];
    let tied_indices = [0_u64, 1_u64];
    let context = ScanTypographySearchReportContext {
        page_index: 2_u64,
        raster_sha256: "tie-raster",
        search_space: ScanTypographySearchSearchSpaceContext {
            font_family: "Tie Family",
            font_weight: FontWeightHypothesis::Regular,
            font_style: FontStyleHypothesis::Italic,
            font_sizes_pt: &font_sizes,
            trackings_pt: &trackings,
            hypothesis_count: 2_u64,
        },
        compiler_version: "typst tie",
        trials: &trials,
        selection: ScanTypographySearchSelectionContext::Tied {
            indices: &tied_indices,
            evidence_scope: "horizontal-only",
        },
    };

    let rendered = render_scan_typography_search_report(&context).unwrap();
    let expected = concat!(
        "{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1,",
        "\"source\":{\"page_index\":2,\"raster_sha256\":\"tie-raster\"},",
        "\"search_space\":{\"font_family\":\"Tie Family\",\"font_weight\":\"regular\",",
        "\"font_style\":\"italic\",\"font_sizes_pt\":[10,18],",
        "\"trackings_pt\":[-0.25],\"hypothesis_count\":2},",
        "\"compiler_version\":\"typst tie\",\"trials\":[",
        "{\"index\":0,\"hypothesis\":{\"font_family\":\"Tie Family\",",
        "\"font_size_pt\":10,\"font_weight\":\"regular\",\"font_style\":\"italic\",",
        "\"tracking_pt\":-0.25},\"source_sha256\":\"tie-source-0\",",
        "\"source_size_bytes\":11,\"pdf_sha256\":\"tie-pdf-0\",\"pdf_size_bytes\":21,",
        "\"content_status\":\"preserved\",\"geometry_status\":\"preserved\",",
        "\"overall_status\":\"preserved\",\"coverage\":{\"matched_scan\":1,",
        "\"total_scan\":1,\"matched_candidate\":1,\"total_candidate\":1},",
        "\"eligibility\":{\"status\":\"eligible\"},\"support\":{",
        "\"horizontal_scan_unit_ids\":[\"line-1\"],\"known_baseline_scan_unit_ids\":[]},",
        "\"score\":{\"horizontal_violations\":0,\"horizontal_residuals_desc\":[4],",
        "\"baseline_violations\":0,\"baseline_residuals_desc\":[]}},",
        "{\"index\":1,\"hypothesis\":{\"font_family\":\"Tie Family\",",
        "\"font_size_pt\":18,\"font_weight\":\"regular\",\"font_style\":\"italic\",",
        "\"tracking_pt\":-0.25},\"source_sha256\":\"tie-source-1\",",
        "\"source_size_bytes\":12,\"pdf_sha256\":\"tie-pdf-1\",\"pdf_size_bytes\":22,",
        "\"content_status\":\"preserved\",\"geometry_status\":\"violated\",",
        "\"overall_status\":\"violated\",\"coverage\":{\"matched_scan\":1,",
        "\"total_scan\":1,\"matched_candidate\":1,\"total_candidate\":1},",
        "\"eligibility\":{\"status\":\"eligible\"},\"support\":{",
        "\"horizontal_scan_unit_ids\":[\"line-1\"],\"known_baseline_scan_unit_ids\":[]},",
        "\"score\":{\"horizontal_violations\":0,\"horizontal_residuals_desc\":[4],",
        "\"baseline_violations\":0,\"baseline_residuals_desc\":[]}}],",
        "\"selection\":{\"status\":\"tied\",\"indices\":[0,1],",
        "\"evidence_scope\":\"horizontal-only\",\"artifact_published\":false},",
        "\"winner\":null}"
    );

    assert_eq!(rendered, expected);
    assert!(!rendered.contains("\"selected_index\":"));
    assert!(!rendered.contains("\"winner_index\":"));
    assert_no_paths_or_timestamps(&rendered);
}

#[test]
fn renders_inconclusive_v1_as_exact_closed_snapshots_with_null_winner() {
    let font_sizes = [12.0];
    let trackings = [-0.5, 0.5];
    let first_hypothesis = TypographyHypothesis {
        font_family: "Incomparable".to_string(),
        size_pt: 12.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Normal,
        tracking_pt: -0.5,
    };
    let second_hypothesis = TypographyHypothesis {
        font_family: "Incomparable".to_string(),
        size_pt: 12.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Normal,
        tracking_pt: 0.5,
    };
    let first_support = ["line-a"];
    let second_support = ["line-b"];
    let no_baselines: [&str; 0] = [];
    let first_residuals = [1_u64];
    let second_residuals = [2_u64];
    let no_baseline_residuals: [u64; 0] = [];
    let trials = [
        ScanTypographySearchTrialContext {
            index: 0_u64,
            hypothesis: &first_hypothesis,
            source_sha256: "inc-source-0",
            source_size_bytes: 13_u64,
            pdf_sha256: "inc-pdf-0",
            pdf_size_bytes: 23_u64,
            content_status: EvidenceStatus::Preserved,
            geometry_status: EvidenceStatus::Preserved,
            overall_status: EvidenceStatus::Preserved,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 1_u64,
                total_scan: 1_u64,
                matched_candidate: 1_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Eligible {
                support: ScanTypographySearchSupportContext {
                    horizontal_scan_unit_ids: &first_support,
                    known_baseline_scan_unit_ids: &no_baselines,
                },
                score: ScanTypographySearchScoreContext {
                    horizontal_violations: 0_u64,
                    horizontal_residuals_desc: &first_residuals,
                    baseline_violations: 0_u64,
                    baseline_residuals_desc: &no_baseline_residuals,
                },
            },
        },
        ScanTypographySearchTrialContext {
            index: 1_u64,
            hypothesis: &second_hypothesis,
            source_sha256: "inc-source-1",
            source_size_bytes: 14_u64,
            pdf_sha256: "inc-pdf-1",
            pdf_size_bytes: 24_u64,
            content_status: EvidenceStatus::Preserved,
            geometry_status: EvidenceStatus::Violated,
            overall_status: EvidenceStatus::Violated,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: 1_u64,
                total_scan: 1_u64,
                matched_candidate: 1_u64,
                total_candidate: 1_u64,
            },
            eligibility: ScanTypographySearchEligibilityContext::Eligible {
                support: ScanTypographySearchSupportContext {
                    horizontal_scan_unit_ids: &second_support,
                    known_baseline_scan_unit_ids: &no_baselines,
                },
                score: ScanTypographySearchScoreContext {
                    horizontal_violations: 0_u64,
                    horizontal_residuals_desc: &second_residuals,
                    baseline_violations: 0_u64,
                    baseline_residuals_desc: &no_baseline_residuals,
                },
            },
        },
    ];
    let context = ScanTypographySearchReportContext {
        page_index: 3_u64,
        raster_sha256: "inc-raster",
        search_space: ScanTypographySearchSearchSpaceContext {
            font_family: "Incomparable",
            font_weight: FontWeightHypothesis::Regular,
            font_style: FontStyleHypothesis::Normal,
            font_sizes_pt: &font_sizes,
            trackings_pt: &trackings,
            hypothesis_count: 2_u64,
        },
        compiler_version: "typst incomparable",
        trials: &trials,
        selection: ScanTypographySearchSelectionContext::Inconclusive {
            reason: "incomparable-support",
        },
    };

    let rendered = render_scan_typography_search_report(&context).unwrap();
    let expected = concat!(
        "{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1,",
        "\"source\":{\"page_index\":3,\"raster_sha256\":\"inc-raster\"},",
        "\"search_space\":{\"font_family\":\"Incomparable\",",
        "\"font_weight\":\"regular\",\"font_style\":\"normal\",",
        "\"font_sizes_pt\":[12],\"trackings_pt\":[-0.5,0.5],\"hypothesis_count\":2},",
        "\"compiler_version\":\"typst incomparable\",\"trials\":[",
        "{\"index\":0,\"hypothesis\":{\"font_family\":\"Incomparable\",",
        "\"font_size_pt\":12,\"font_weight\":\"regular\",\"font_style\":\"normal\",",
        "\"tracking_pt\":-0.5},\"source_sha256\":\"inc-source-0\",",
        "\"source_size_bytes\":13,\"pdf_sha256\":\"inc-pdf-0\",\"pdf_size_bytes\":23,",
        "\"content_status\":\"preserved\",\"geometry_status\":\"preserved\",",
        "\"overall_status\":\"preserved\",\"coverage\":{\"matched_scan\":1,",
        "\"total_scan\":1,\"matched_candidate\":1,\"total_candidate\":1},",
        "\"eligibility\":{\"status\":\"eligible\"},\"support\":{",
        "\"horizontal_scan_unit_ids\":[\"line-a\"],\"known_baseline_scan_unit_ids\":[]},",
        "\"score\":{\"horizontal_violations\":0,\"horizontal_residuals_desc\":[1],",
        "\"baseline_violations\":0,\"baseline_residuals_desc\":[]}},",
        "{\"index\":1,\"hypothesis\":{\"font_family\":\"Incomparable\",",
        "\"font_size_pt\":12,\"font_weight\":\"regular\",\"font_style\":\"normal\",",
        "\"tracking_pt\":0.5},\"source_sha256\":\"inc-source-1\",",
        "\"source_size_bytes\":14,\"pdf_sha256\":\"inc-pdf-1\",\"pdf_size_bytes\":24,",
        "\"content_status\":\"preserved\",\"geometry_status\":\"violated\",",
        "\"overall_status\":\"violated\",\"coverage\":{\"matched_scan\":1,",
        "\"total_scan\":1,\"matched_candidate\":1,\"total_candidate\":1},",
        "\"eligibility\":{\"status\":\"eligible\"},\"support\":{",
        "\"horizontal_scan_unit_ids\":[\"line-b\"],\"known_baseline_scan_unit_ids\":[]},",
        "\"score\":{\"horizontal_violations\":0,\"horizontal_residuals_desc\":[2],",
        "\"baseline_violations\":0,\"baseline_residuals_desc\":[]}}],",
        "\"selection\":{\"status\":\"inconclusive\",",
        "\"reason\":\"incomparable-support\",\"evidence_scope\":null,",
        "\"artifact_published\":false},\"winner\":null}"
    );

    assert_eq!(rendered, expected);
    assert!(!rendered.contains("\"selected_index\":"));
    assert!(!rendered.contains("\"indices\":"));
    assert!(!rendered.contains("\"winner_index\":"));
    assert_no_paths_or_timestamps(&rendered);

    let no_eligible_font_sizes = [12.0];
    let no_eligible_trackings = [0.0];
    let no_eligible_hypothesis = TypographyHypothesis {
        font_family: "No Eligible".to_string(),
        size_pt: 12.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Normal,
        tracking_pt: 0.0,
    };
    let no_eligible_trials = [ScanTypographySearchTrialContext {
        index: 0_u64,
        hypothesis: &no_eligible_hypothesis,
        source_sha256: "empty-source",
        source_size_bytes: 17_u64,
        pdf_sha256: "empty-pdf",
        pdf_size_bytes: 29_u64,
        content_status: EvidenceStatus::Preserved,
        geometry_status: EvidenceStatus::Unknown,
        overall_status: EvidenceStatus::Unknown,
        coverage: ScanTypographySearchCoverageContext {
            matched_scan: 0_u64,
            total_scan: 0_u64,
            matched_candidate: 0_u64,
            total_candidate: 0_u64,
        },
        eligibility: ScanTypographySearchEligibilityContext::Ineligible {
            reason: "empty-scan-scope",
        },
    }];
    let no_eligible_context = ScanTypographySearchReportContext {
        page_index: 5_u64,
        raster_sha256: "empty-raster",
        search_space: ScanTypographySearchSearchSpaceContext {
            font_family: "No Eligible",
            font_weight: FontWeightHypothesis::Regular,
            font_style: FontStyleHypothesis::Normal,
            font_sizes_pt: &no_eligible_font_sizes,
            trackings_pt: &no_eligible_trackings,
            hypothesis_count: 1_u64,
        },
        compiler_version: "typst no eligible",
        trials: &no_eligible_trials,
        selection: ScanTypographySearchSelectionContext::Inconclusive {
            reason: "no-eligible-trial",
        },
    };

    let no_eligible_rendered = render_scan_typography_search_report(&no_eligible_context).unwrap();
    let no_eligible_expected = concat!(
        "{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1,",
        "\"source\":{\"page_index\":5,\"raster_sha256\":\"empty-raster\"},",
        "\"search_space\":{\"font_family\":\"No Eligible\",",
        "\"font_weight\":\"regular\",\"font_style\":\"normal\",",
        "\"font_sizes_pt\":[12],\"trackings_pt\":[0],\"hypothesis_count\":1},",
        "\"compiler_version\":\"typst no eligible\",\"trials\":[",
        "{\"index\":0,\"hypothesis\":{\"font_family\":\"No Eligible\",",
        "\"font_size_pt\":12,\"font_weight\":\"regular\",\"font_style\":\"normal\",",
        "\"tracking_pt\":0},\"source_sha256\":\"empty-source\",",
        "\"source_size_bytes\":17,\"pdf_sha256\":\"empty-pdf\",",
        "\"pdf_size_bytes\":29,\"content_status\":\"preserved\",",
        "\"geometry_status\":\"unknown\",\"overall_status\":\"unknown\",",
        "\"coverage\":{\"matched_scan\":0,\"total_scan\":0,",
        "\"matched_candidate\":0,\"total_candidate\":0},",
        "\"eligibility\":{\"status\":\"ineligible\",",
        "\"reason\":\"empty-scan-scope\"},\"support\":null,\"score\":null}],",
        "\"selection\":{\"status\":\"inconclusive\",",
        "\"reason\":\"no-eligible-trial\",\"evidence_scope\":null,",
        "\"artifact_published\":false},\"winner\":null}"
    );

    assert_eq!(no_eligible_rendered, no_eligible_expected);
    assert!(!no_eligible_rendered.contains("\"selected_index\":"));
    assert!(!no_eligible_rendered.contains("\"indices\":"));
    assert!(!no_eligible_rendered.contains("\"winner_index\":"));
    assert_no_paths_or_timestamps(&no_eligible_rendered);
}
