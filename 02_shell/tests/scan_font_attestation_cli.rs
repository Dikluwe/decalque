//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-font-attestation.md
//! @layer L2
//! @updated 2026-09-19
//!
//! Oráculo independente de L2 para a CLI e o relatório externo da atestação
//! estrutural de fonte. Baseline de intenção:
//! `592ecfef648aac380e7c8e7e271b36167a7a59d6`.

use decalque_core::{
    ConfidenceRequirement, EvidenceStatus, FontStyleHypothesis, FontWeightHypothesis,
    ScanGranularity, TextNormalization, TypographyHypothesis,
};
use decalque_shell::{
    parse_scan_font_attestation_args, render_scan_font_attestation_report,
    ScanFontAttestationCliArgs, ScanFontAttestationCliParseError,
    ScanFontAttestationDiagnosticContext, ScanFontAttestationEvidenceContext,
    ScanFontAttestationReportContext, ScanFontAttestationResourceContext, SCAN_EVALUATION_LIMITS,
    SCAN_FONT_ATTESTATION_LIMITS,
};
use std::ffi::OsString;

fn valid_args() -> Vec<OsString> {
    [
        "__l2_must_not_read__/observation.json",
        "--raster",
        "__l2_must_not_read__/page.png",
        "--output-pdf",
        "__l2_must_not_write__/candidate.pdf",
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

fn fully_explicit_args() -> Vec<OsString> {
    let mut args = valid_args();
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
            "__l2_must_not_execute__/typst with spaces",
        ]
        .map(OsString::from),
    );
    args
}

fn option_index(args: &[OsString], option: &str) -> usize {
    args.iter()
        .position(|argument| argument == option)
        .unwrap_or_else(|| panic!("fixture does not contain {option}"))
}

fn remove_option(mut args: Vec<OsString>, option: &str) -> Vec<OsString> {
    let index = option_index(&args, option);
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
    let index = option_index(args, option);
    args[index + 1] = value;
}

fn with_option_value(option: &str, value: &str) -> Vec<OsString> {
    let mut args = valid_args();
    if args.iter().any(|argument| argument == option) {
        replace_option_value(&mut args, option, OsString::from(value));
    } else {
        args.extend([OsString::from(option), OsString::from(value)]);
    }
    args
}

fn parse_ok(args: Vec<OsString>, context: &str) -> ScanFontAttestationCliArgs {
    match parse_scan_font_attestation_args(args) {
        Ok(parsed) => parsed,
        Err(error) => panic!("{context} should parse successfully: {error}"),
    }
}

fn assert_rejected(args: Vec<OsString>, context: &str) {
    if parse_scan_font_attestation_args(args).is_ok() {
        panic!("{context} should be rejected");
    }
}

fn assert_error_mentions(args: Vec<OsString>, expected: &str, context: &str) {
    let error: ScanFontAttestationCliParseError = match parse_scan_font_attestation_args(args) {
        Ok(_) => panic!("{context} should be rejected"),
        Err(error) => error,
    };
    let rendered = error.to_string();
    assert!(
        rendered.contains(expected),
        "{context} should mention {expected:?}, got {rendered:?}"
    );
}

#[test]
fn requires_the_observation_and_every_mandatory_option_before_any_domain_io() {
    assert_rejected(Vec::new(), "empty invocation");

    let mut missing_observation = valid_args();
    missing_observation.remove(0);
    assert_rejected(missing_observation, "missing observation path");

    let mut empty_observation = valid_args();
    empty_observation[0] = OsString::new();
    assert_rejected(empty_observation, "empty observation path");

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

    parse_ok(
        valid_args(),
        "syntactically valid but deliberately nonexistent paths",
    );
}

#[test]
fn applies_closed_defaults_and_fixes_granularity_to_lines_without_a_cli_option() {
    let parsed = parse_ok(valid_args(), "defaulted invocation");

    assert_eq!(parsed.typst_bin.as_os_str(), "typst");
    assert_eq!(parsed.typography.font_family, "Libertinus Serif");
    assert_eq!(parsed.typography.size_pt, 10.0);
    assert_eq!(parsed.typography.weight, FontWeightHypothesis::Regular);
    assert_eq!(parsed.typography.style, FontStyleHypothesis::Normal);
    assert_eq!(parsed.typography.tracking_pt.to_bits(), 0.0_f64.to_bits());

    assert_eq!(parsed.policy.granularity, ScanGranularity::Line);
    assert_eq!(parsed.policy.horizontal_tolerance_pt, 0.5);
    assert_eq!(parsed.policy.baseline_tolerance_pt, 0.25);
    assert_eq!(parsed.policy.text_confidence, ConfidenceRequirement::Any);
    assert_eq!(
        parsed.policy.geometry_confidence,
        ConfidenceRequirement::Any
    );
    assert_eq!(parsed.policy.text_normalization, TextNormalization::Exact);
}

#[test]
fn uses_product_execution_limits_without_cli_overrides() {
    assert_eq!(SCAN_FONT_ATTESTATION_LIMITS, SCAN_EVALUATION_LIMITS);
    assert_eq!(
        SCAN_FONT_ATTESTATION_LIMITS.source_max_bytes,
        16 * 1024 * 1024
    );
    assert_eq!(
        SCAN_FONT_ATTESTATION_LIMITS.pdf_max_bytes,
        128 * 1024 * 1024
    );
    assert_eq!(SCAN_FONT_ATTESTATION_LIMITS.stderr_max_bytes, 1024 * 1024);
    assert_eq!(SCAN_FONT_ATTESTATION_LIMITS.version_max_bytes, 64 * 1024);
    assert_eq!(
        SCAN_FONT_ATTESTATION_LIMITS.timeout,
        std::time::Duration::from_secs(30)
    );
}

#[test]
fn parses_the_complete_fixed_hypothesis_policy_and_opaque_native_paths() {
    let parsed = parse_ok(fully_explicit_args(), "fully explicit invocation");

    assert_eq!(
        parsed.observation.as_os_str(),
        "__l2_must_not_read__/observation.json"
    );
    assert_eq!(parsed.raster.as_os_str(), "__l2_must_not_read__/page.png");
    assert_eq!(
        parsed.output_pdf.as_os_str(),
        "__l2_must_not_write__/candidate.pdf"
    );
    assert_eq!(
        parsed.typst_bin.as_os_str(),
        "__l2_must_not_execute__/typst with spaces"
    );
    assert_eq!(parsed.typography.font_family, "Libertinus Serif");
    assert_eq!(parsed.typography.size_pt, 10.0);
    assert_eq!(parsed.typography.weight, FontWeightHypothesis::Bold);
    assert_eq!(parsed.typography.style, FontStyleHypothesis::Oblique);
    assert_eq!(parsed.typography.tracking_pt, -0.25);
    assert_eq!(parsed.policy.granularity, ScanGranularity::Line);
    assert_eq!(
        parsed.policy.text_confidence,
        ConfidenceRequirement::KnownAtLeast(0.875)
    );
    assert_eq!(
        parsed.policy.geometry_confidence,
        ConfidenceRequirement::KnownAtLeast(0.625)
    );
}

#[test]
fn rejects_every_repeated_option_and_a_second_positional_argument() {
    for (option, value) in [
        ("--raster", "other.png"),
        ("--output-pdf", "other.pdf"),
        ("--font-family", "Other Family"),
        ("--font-size-pt", "12"),
        ("--font-weight", "regular"),
        ("--font-style", "italic"),
        ("--tracking-pt", "0.5"),
        ("--horizontal-tolerance-pt", "0.75"),
        ("--baseline-tolerance-pt", "0.75"),
        ("--min-text-confidence", "0.5"),
        ("--min-geometry-confidence", "0.5"),
        ("--typst-bin", "other-typst"),
    ] {
        let mut repeated = fully_explicit_args();
        repeated.extend([OsString::from(option), OsString::from(value)]);
        assert_rejected(repeated, &format!("repeated option {option}"));
    }

    let mut extra_positional = valid_args();
    extra_positional.push(OsString::from("second-observation.json"));
    assert_rejected(extra_positional, "a second positional argument");
}

#[test]
fn rejects_unknown_options_including_granularity_and_limit_overrides() {
    for (option, value) in [
        ("--granularity", "line"),
        ("--source-max-bytes", "1"),
        ("--pdf-max-bytes", "1"),
        ("--stderr-max-bytes", "1"),
        ("--version-max-bytes", "1"),
        ("--timeout-seconds", "1"),
        ("--candidate", "candidate.pdf"),
    ] {
        let mut args = valid_args();
        args.extend([OsString::from(option), OsString::from(value)]);
        assert_error_mentions(args, option, &format!("unknown option {option}"));
    }
}

#[test]
fn rejects_every_known_option_without_a_value() {
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
        assert_error_mentions(args, option, &format!("missing value for {option}"));
    }
}

#[test]
fn rejects_non_finite_numbers_in_every_numeric_position() {
    for option in [
        "--font-size-pt",
        "--tracking-pt",
        "--horizontal-tolerance-pt",
        "--baseline-tolerance-pt",
        "--min-text-confidence",
        "--min-geometry-confidence",
    ] {
        for value in ["NaN", "inf", "-inf", "1e9999"] {
            assert_rejected(
                with_option_value(option, value),
                &format!("non-finite {value} for {option}"),
            );
        }
    }
}

#[test]
fn enforces_positive_body_nonnegative_tolerances_and_closed_confidences() {
    for value in ["0", "-0.0001", "-10"] {
        assert_rejected(
            with_option_value("--font-size-pt", value),
            &format!("non-positive body {value}"),
        );
    }
    for value in ["-0.0001", "-10"] {
        for option in ["--horizontal-tolerance-pt", "--baseline-tolerance-pt"] {
            assert_rejected(
                with_option_value(option, value),
                &format!("negative tolerance {value} for {option}"),
            );
        }
    }
    for value in ["-0.0001", "1.0001"] {
        for option in ["--min-text-confidence", "--min-geometry-confidence"] {
            assert_rejected(
                with_option_value(option, value),
                &format!("out-of-range confidence {value} for {option}"),
            );
        }
    }

    for value in ["0", "1"] {
        parse_ok(
            with_option_value("--min-text-confidence", value),
            &format!("text confidence boundary {value}"),
        );
        parse_ok(
            with_option_value("--min-geometry-confidence", value),
            &format!("geometry confidence boundary {value}"),
        );
    }
    parse_ok(
        with_option_value("--horizontal-tolerance-pt", "0"),
        "zero horizontal tolerance",
    );
    parse_ok(
        with_option_value("--baseline-tolerance-pt", "0"),
        "zero baseline tolerance",
    );
    parse_ok(
        with_option_value("--font-size-pt", "1e300"),
        "large but finite positive body",
    );
    parse_ok(
        with_option_value("--tracking-pt", "-1e300"),
        "large but finite tracking",
    );
}

#[test]
fn rejects_empty_family_and_values_outside_closed_weight_and_style_enums() {
    for family in ["", " ", "\t\n"] {
        assert_rejected(
            with_option_value("--font-family", family),
            "empty or blank font family",
        );
    }
    for weight in ["medium", "Regular", "700"] {
        assert_rejected(
            with_option_value("--font-weight", weight),
            &format!("unsupported weight {weight}"),
        );
    }
    for style in ["roman", "Normal", "slanted"] {
        assert_rejected(
            with_option_value("--font-style", style),
            &format!("unsupported style {style}"),
        );
    }
}

#[cfg(unix)]
#[test]
fn preserves_non_utf8_unix_paths_and_rejects_non_utf8_text_values() {
    use std::os::unix::ffi::OsStringExt;

    let observation = OsString::from_vec(b"observation-\xff.json".to_vec());
    let raster = OsString::from_vec(b"page-\xfe.png".to_vec());
    let output_pdf = OsString::from_vec(b"candidate-\xfd.pdf".to_vec());
    let typst_bin = OsString::from_vec(b"typst-\xfc".to_vec());

    let mut args = valid_args();
    args[0] = observation.clone();
    replace_option_value(&mut args, "--raster", raster.clone());
    replace_option_value(&mut args, "--output-pdf", output_pdf.clone());
    args.extend([OsString::from("--typst-bin"), typst_bin.clone()]);

    let parsed = parse_ok(args, "non-UTF-8 native paths");
    assert_eq!(parsed.observation.as_os_str(), observation.as_os_str());
    assert_eq!(parsed.raster.as_os_str(), raster.as_os_str());
    assert_eq!(parsed.output_pdf.as_os_str(), output_pdf.as_os_str());
    assert_eq!(parsed.typst_bin.as_os_str(), typst_bin.as_os_str());

    for option in ["--font-family", "--font-size-pt", "--font-weight"] {
        let mut args = if option == "--font-weight" {
            let mut args = valid_args();
            args.extend([OsString::from(option), OsString::from("regular")]);
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

const COMPLETE_COMPARISON_JSON: &str = concat!(
    "{\"schema\":\"decalque.scan-comparison-report\",\"schema_version\":1,",
    "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
    "\"policy\":{\"granularity\":\"line\",\"text_normalization\":\"exact\",",
    "\"horizontal_tolerance_pt\":0.5,\"baseline_tolerance_pt\":0.25,",
    "\"min_text_confidence\":{\"status\":\"not-required\"},",
    "\"min_geometry_confidence\":{\"status\":\"not-required\"}},",
    "\"content_status\":\"preserved\",\"geometry_status\":\"violated\",",
    "\"overall_status\":\"violated\",\"coverage\":{\"matched_scan\":1,",
    "\"total_scan\":1,\"matched_candidate\":1,\"total_candidate\":1},",
    "\"matches\":[],\"unmatched_scan\":[],\"unmatched_candidate\":[],",
    "\"reflow\":[],\"observation_diagnostics\":[],\"pdf_diagnostics\":[],",
    "\"comparison_diagnostics\":[]}",
);

fn hypothesis(
    family: &str,
    size_pt: f64,
    weight: FontWeightHypothesis,
    style: FontStyleHypothesis,
    tracking_pt: f64,
) -> TypographyHypothesis {
    TypographyHypothesis {
        font_family: family.to_string(),
        size_pt,
        weight,
        style,
        tracking_pt,
    }
}

fn assert_closed_report(rendered: &str) {
    assert!(
        rendered.starts_with('{'),
        "report must start with an object"
    );
    assert!(rendered.ends_with('}'), "report must end with an object");
    assert!(!rendered.ends_with('\n'), "report must not end in LF");

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
        "candidate.pdf",
        "__l2_must_not_read__",
        "__l2_must_not_write__",
        "__l2_must_not_execute__",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "report must not expose {forbidden:?}: {rendered}"
        );
    }
}

#[test]
fn renders_preserved_v1_byte_exact_with_forbidden_fallback_and_canonical_used_resources() {
    let typography = hypothesis(
        "Libertinus Serif",
        10.0,
        FontWeightHypothesis::Bold,
        FontStyleHypothesis::Italic,
        0.0,
    );
    let diagnostics: [ScanFontAttestationDiagnosticContext<'_>; 0] = [];
    let used_resources = [
        ScanFontAttestationResourceContext {
            resource_name: "F2",
            base_font: Some("UVWXYZ+Libertinus_Serif-BoldItalic"),
            normalized_stem: Some("libertinusserif"),
            glyph_count: 3,
        },
        ScanFontAttestationResourceContext {
            resource_name: "F1",
            base_font: Some("ABCDEF+LibertinusSerif-BoldItalic-Identity-H"),
            normalized_stem: Some("libertinusserif"),
            glyph_count: 4,
        },
    ];
    let context = ScanFontAttestationReportContext {
        page_index: 7,
        raster_sha256: "raster-hash",
        typography: &typography,
        compiler_version: "typst 0.15.1",
        source_sha256: "source-hash",
        source_size_bytes: 17,
        pdf_sha256: "pdf-hash",
        pdf_size_bytes: 29,
        attestation: ScanFontAttestationEvidenceContext {
            status: EvidenceStatus::Preserved,
            expected_scalar_count: 7,
            candidate_scalar_count: Some(7),
            diagnostics: &diagnostics,
            used_resources: &used_resources,
        },
        comparison_json: COMPLETE_COMPARISON_JSON,
        artifact_published: true,
    };

    let rendered = render_scan_font_attestation_report(&context).unwrap();
    let expected = [
        concat!(
            "{\"schema\":\"decalque.scan-font-attestation\",\"schema_version\":1,",
            "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
            "\"hypothesis\":{\"font_family\":\"Libertinus Serif\",",
            "\"font_size_pt\":10,\"font_weight\":\"bold\",",
            "\"font_style\":\"italic\",\"tracking_pt\":0},",
            "\"fallback_policy\":\"forbidden\",\"compiler_version\":\"typst 0.15.1\",",
            "\"source_sha256\":\"source-hash\",\"source_size_bytes\":17,",
            "\"pdf_sha256\":\"pdf-hash\",\"pdf_size_bytes\":29,",
            "\"attestation\":{\"status\":\"preserved\",",
            "\"expected_scalar_count\":7,\"candidate_scalar_count\":7,",
            "\"diagnostics\":[],\"used_resources\":[",
            "{\"resource_name\":\"F1\",",
            "\"base_font\":\"ABCDEF+LibertinusSerif-BoldItalic-Identity-H\",",
            "\"normalized_stem\":\"libertinusserif\",\"glyph_count\":4},",
            "{\"resource_name\":\"F2\",",
            "\"base_font\":\"UVWXYZ+Libertinus_Serif-BoldItalic\",",
            "\"normalized_stem\":\"libertinusserif\",\"glyph_count\":3}]},",
            "\"comparison\":"
        ),
        COMPLETE_COMPARISON_JSON,
        ",\"artifact_published\":true}",
    ]
    .concat();

    assert_eq!(rendered, expected);
    assert_closed_report(&rendered);
}

#[test]
fn renders_violated_v1_byte_exact_with_canonical_diagnostics_and_no_publication() {
    let typography = hypothesis(
        "Wanted Serif",
        12.5,
        FontWeightHypothesis::Regular,
        FontStyleHypothesis::Normal,
        -0.25,
    );
    let diagnostics = [
        ScanFontAttestationDiagnosticContext {
            code: "text-sequence-mismatch",
            resource_name: None,
            scalar_index: Some(2),
        },
        ScanFontAttestationDiagnosticContext {
            code: "family-mismatch",
            resource_name: Some("F9"),
            scalar_index: None,
        },
    ];
    let used_resources = [
        ScanFontAttestationResourceContext {
            resource_name: "F9",
            base_font: Some("UVWXYZ+OtherSerif-Regular"),
            normalized_stem: Some("otherserif"),
            glyph_count: 1,
        },
        ScanFontAttestationResourceContext {
            resource_name: "F1",
            base_font: Some("ABCDEF+WantedSerif-Regular"),
            normalized_stem: Some("wantedserif"),
            glyph_count: 2,
        },
    ];
    let context = ScanFontAttestationReportContext {
        page_index: 7,
        raster_sha256: "raster-hash",
        typography: &typography,
        compiler_version: "typst 0.15.1",
        source_sha256: "violated-source",
        source_size_bytes: u64::MAX,
        pdf_sha256: "violated-pdf",
        pdf_size_bytes: 31,
        attestation: ScanFontAttestationEvidenceContext {
            status: EvidenceStatus::Violated,
            expected_scalar_count: 3,
            candidate_scalar_count: Some(3),
            diagnostics: &diagnostics,
            used_resources: &used_resources,
        },
        comparison_json: COMPLETE_COMPARISON_JSON,
        artifact_published: false,
    };

    let rendered = render_scan_font_attestation_report(&context).unwrap();
    let expected = [
        concat!(
            "{\"schema\":\"decalque.scan-font-attestation\",\"schema_version\":1,",
            "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
            "\"hypothesis\":{\"font_family\":\"Wanted Serif\",",
            "\"font_size_pt\":12.5,\"font_weight\":\"regular\",",
            "\"font_style\":\"normal\",\"tracking_pt\":-0.25},",
            "\"fallback_policy\":\"forbidden\",\"compiler_version\":\"typst 0.15.1\",",
            "\"source_sha256\":\"violated-source\",",
            "\"source_size_bytes\":18446744073709551615,",
            "\"pdf_sha256\":\"violated-pdf\",\"pdf_size_bytes\":31,",
            "\"attestation\":{\"status\":\"violated\",",
            "\"expected_scalar_count\":3,\"candidate_scalar_count\":3,",
            "\"diagnostics\":[{\"code\":\"family-mismatch\",",
            "\"resource_name\":\"F9\",\"scalar_index\":null},",
            "{\"code\":\"text-sequence-mismatch\",",
            "\"resource_name\":null,\"scalar_index\":2}],\"used_resources\":[",
            "{\"resource_name\":\"F1\",",
            "\"base_font\":\"ABCDEF+WantedSerif-Regular\",",
            "\"normalized_stem\":\"wantedserif\",\"glyph_count\":2},",
            "{\"resource_name\":\"F9\",",
            "\"base_font\":\"UVWXYZ+OtherSerif-Regular\",",
            "\"normalized_stem\":\"otherserif\",\"glyph_count\":1}]},",
            "\"comparison\":"
        ),
        COMPLETE_COMPARISON_JSON,
        ",\"artifact_published\":false}",
    ]
    .concat();

    assert_eq!(rendered, expected);
    assert_closed_report(&rendered);
}

#[test]
fn renders_unknown_v1_byte_exact_with_null_evidence_json_escaping_and_no_publication() {
    let typography = hypothesis(
        "Opaque \"Family\"\\\n",
        9.75,
        FontWeightHypothesis::Regular,
        FontStyleHypothesis::Oblique,
        0.125,
    );
    let opaque_resource = "F\"3\\\n";
    let diagnostics = [
        ScanFontAttestationDiagnosticContext {
            code: "unmapped-glyph",
            resource_name: None,
            scalar_index: Some(2),
        },
        ScanFontAttestationDiagnosticContext {
            code: "missing-base-font",
            resource_name: Some(opaque_resource),
            scalar_index: None,
        },
    ];
    let used_resources = [ScanFontAttestationResourceContext {
        resource_name: opaque_resource,
        base_font: None,
        normalized_stem: None,
        glyph_count: u64::MAX,
    }];
    let context = ScanFontAttestationReportContext {
        page_index: 7,
        raster_sha256: "raster-hash",
        typography: &typography,
        compiler_version: "typst \"dev\"\\build\n",
        source_sha256: "unknown-source",
        source_size_bytes: 41,
        pdf_sha256: "unknown-pdf",
        pdf_size_bytes: 43,
        attestation: ScanFontAttestationEvidenceContext {
            status: EvidenceStatus::Unknown,
            expected_scalar_count: 3,
            candidate_scalar_count: None,
            diagnostics: &diagnostics,
            used_resources: &used_resources,
        },
        comparison_json: COMPLETE_COMPARISON_JSON,
        artifact_published: false,
    };

    let rendered = render_scan_font_attestation_report(&context).unwrap();
    let expected = [
        concat!(
            "{\"schema\":\"decalque.scan-font-attestation\",\"schema_version\":1,",
            "\"source\":{\"page_index\":7,\"raster_sha256\":\"raster-hash\"},",
            "\"hypothesis\":{\"font_family\":\"Opaque \\\"Family\\\"\\\\\\n\",",
            "\"font_size_pt\":9.75,\"font_weight\":\"regular\",",
            "\"font_style\":\"oblique\",\"tracking_pt\":0.125},",
            "\"fallback_policy\":\"forbidden\",",
            "\"compiler_version\":\"typst \\\"dev\\\"\\\\build\\n\",",
            "\"source_sha256\":\"unknown-source\",\"source_size_bytes\":41,",
            "\"pdf_sha256\":\"unknown-pdf\",\"pdf_size_bytes\":43,",
            "\"attestation\":{\"status\":\"unknown\",",
            "\"expected_scalar_count\":3,\"candidate_scalar_count\":null,",
            "\"diagnostics\":[{\"code\":\"missing-base-font\",",
            "\"resource_name\":\"F\\\"3\\\\\\n\",\"scalar_index\":null},",
            "{\"code\":\"unmapped-glyph\",\"resource_name\":null,",
            "\"scalar_index\":2}],\"used_resources\":[",
            "{\"resource_name\":\"F\\\"3\\\\\\n\",\"base_font\":null,",
            "\"normalized_stem\":null,",
            "\"glyph_count\":18446744073709551615}]},\"comparison\":"
        ),
        COMPLETE_COMPARISON_JSON,
        ",\"artifact_published\":false}",
    ]
    .concat();

    assert_eq!(rendered, expected);
    assert_closed_report(&rendered);
}

#[test]
fn refuses_artifact_published_for_violated_or_unknown_attestations() {
    let typography = hypothesis(
        "Family",
        10.0,
        FontWeightHypothesis::Regular,
        FontStyleHypothesis::Normal,
        0.0,
    );
    let diagnostics: [ScanFontAttestationDiagnosticContext<'_>; 0] = [];
    let used_resources: [ScanFontAttestationResourceContext<'_>; 0] = [];

    for status in [EvidenceStatus::Violated, EvidenceStatus::Unknown] {
        let context = ScanFontAttestationReportContext {
            page_index: 7,
            raster_sha256: "raster-hash",
            typography: &typography,
            compiler_version: "typst 0.15.1",
            source_sha256: "source-hash",
            source_size_bytes: 17,
            pdf_sha256: "pdf-hash",
            pdf_size_bytes: 29,
            attestation: ScanFontAttestationEvidenceContext {
                status,
                expected_scalar_count: 0,
                candidate_scalar_count: Some(0),
                diagnostics: &diagnostics,
                used_resources: &used_resources,
            },
            comparison_json: COMPLETE_COMPARISON_JSON,
            artifact_published: true,
        };

        assert!(
            render_scan_font_attestation_report(&context).is_err(),
            "only preserved may report a published artifact; got {status:?}"
        );
    }
}

#[test]
fn rejects_non_finite_hypothesis_numbers_during_report_rendering() {
    let diagnostics: [ScanFontAttestationDiagnosticContext<'_>; 0] = [];
    let used_resources: [ScanFontAttestationResourceContext<'_>; 0] = [];

    for (size_pt, tracking_pt, context_name) in [
        (f64::NAN, 0.0, "NaN body"),
        (f64::INFINITY, 0.0, "infinite body"),
        (10.0, f64::NAN, "NaN tracking"),
        (10.0, f64::NEG_INFINITY, "infinite tracking"),
    ] {
        let typography = hypothesis(
            "Family",
            size_pt,
            FontWeightHypothesis::Regular,
            FontStyleHypothesis::Normal,
            tracking_pt,
        );
        let context = ScanFontAttestationReportContext {
            page_index: 7,
            raster_sha256: "raster-hash",
            typography: &typography,
            compiler_version: "typst 0.15.1",
            source_sha256: "source-hash",
            source_size_bytes: 17,
            pdf_sha256: "pdf-hash",
            pdf_size_bytes: 29,
            attestation: ScanFontAttestationEvidenceContext {
                status: EvidenceStatus::Unknown,
                expected_scalar_count: 0,
                candidate_scalar_count: Some(0),
                diagnostics: &diagnostics,
                used_resources: &used_resources,
            },
            comparison_json: COMPLETE_COMPARISON_JSON,
            artifact_published: false,
        };

        assert!(
            render_scan_font_attestation_report(&context).is_err(),
            "renderer must reject {context_name}"
        );
    }
}
