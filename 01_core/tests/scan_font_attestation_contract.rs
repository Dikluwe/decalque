//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-font-attestation.md
//! @layer L1
//! @updated 2026-09-19
//!
//! Oraculos L1 independentes para a atestacao estrutural de fonte.
//! Congelados no baseline de intencao `592ecfe` antes da implementacao.
//!
//! API publica pura presumida em `decalque_core`:
//! - `attest_scan_font(&ScanFontAttestationInput) -> ScanFontAttestationReport`;
//! - `PdfFontResource`, contrato irmao de `DocumentGeometry`;
//! - tipos de relatorio, testemunha e face importados abaixo.

use decalque_core::entities::PageRotation;
use decalque_core::{
    attest_scan_font, DocumentGeometry, EvidenceStatus, FontModelDiagnostic, FontStyleHypothesis,
    FontWeightHypothesis, GlyphInstance, PageGeometry, PdfFontResource,
    ScanFontAttestationDiagnostic, ScanFontAttestationInput, ScanFontAttestationReport,
    ScanFontFace, TextInterpreterDiagnostic, TextMappingStatus, TypographyHypothesis,
};

fn hypothesis(
    family: &str,
    weight: FontWeightHypothesis,
    style: FontStyleHypothesis,
) -> TypographyHypothesis {
    TypographyHypothesis {
        font_family: family.to_string(),
        size_pt: 11.0,
        weight,
        style,
        tracking_pt: 0.0,
    }
}

fn regular_hypothesis(family: &str) -> TypographyHypothesis {
    hypothesis(
        family,
        FontWeightHypothesis::Regular,
        FontStyleHypothesis::Normal,
    )
}

fn mapped(font_ref: &str, codepoints: &[char]) -> GlyphInstance {
    GlyphInstance {
        glyph_code: 1,
        codepoints: Some(codepoints.to_vec()),
        position: (10.0, 20.0),
        advance: 5.0,
        font_ref: font_ref.to_string(),
        font_size_pt: 11.0,
        mapping_status: TextMappingStatus::Mapped,
        render_mode: 0,
    }
}

fn unmapped(font_ref: &str) -> GlyphInstance {
    GlyphInstance {
        glyph_code: 2,
        codepoints: None,
        position: (15.0, 20.0),
        advance: 5.0,
        font_ref: font_ref.to_string(),
        font_size_pt: 11.0,
        mapping_status: TextMappingStatus::Unmapped,
        render_mode: 0,
    }
}

fn scalar_glyphs(text: &str, font_ref: &str) -> Vec<GlyphInstance> {
    text.chars()
        .map(|scalar| mapped(font_ref, &[scalar]))
        .collect()
}

fn document(glyphs: Vec<GlyphInstance>) -> DocumentGeometry {
    DocumentGeometry {
        page: PageGeometry {
            width: 420.0,
            height: 595.0,
            origin: (0.0, 0.0),
            rotation: PageRotation::Deg0,
            user_unit: 1.0,
        },
        glyphs,
        diagnostics: Vec::new(),
    }
}

fn resource(resource_name: &str, base_font: Option<&str>) -> PdfFontResource {
    PdfFontResource {
        resource_name: resource_name.to_string(),
        base_font: base_font.map(str::to_string),
    }
}

fn attest(
    expected_lines: &[&str],
    typography: TypographyHypothesis,
    glyphs: Vec<GlyphInstance>,
    font_resources: Vec<PdfFontResource>,
    font_model_diagnostics: Vec<FontModelDiagnostic>,
    text_interpreter_diagnostics: Vec<TextInterpreterDiagnostic>,
) -> ScanFontAttestationReport {
    let expected_line_texts = expected_lines
        .iter()
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    let candidate = document(glyphs);
    let input = ScanFontAttestationInput {
        expected_line_texts: &expected_line_texts,
        typography: &typography,
        candidate: &candidate,
        font_resources: &font_resources,
        font_model_diagnostics: &font_model_diagnostics,
        text_interpreter_diagnostics: &text_interpreter_diagnostics,
    };

    attest_scan_font(&input)
}

fn assert_has_diagnostic(
    report: &ScanFontAttestationReport,
    expected: ScanFontAttestationDiagnostic,
) {
    assert!(
        report.diagnostics.contains(&expected),
        "missing {expected:?} in {:?}",
        report.diagnostics
    );
}

fn face_cases() -> [(
    FontWeightHypothesis,
    FontStyleHypothesis,
    &'static str,
    ScanFontFace,
); 6] {
    [
        (
            FontWeightHypothesis::Regular,
            FontStyleHypothesis::Normal,
            "Regular",
            ScanFontFace::Regular,
        ),
        (
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Normal,
            "Bold",
            ScanFontFace::Bold,
        ),
        (
            FontWeightHypothesis::Regular,
            FontStyleHypothesis::Italic,
            "Italic",
            ScanFontFace::Italic,
        ),
        (
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Italic,
            "BoldItalic",
            ScanFontFace::BoldItalic,
        ),
        (
            FontWeightHypothesis::Regular,
            FontStyleHypothesis::Oblique,
            "Oblique",
            ScanFontFace::Oblique,
        ),
        (
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Oblique,
            "BoldOblique",
            ScanFontFace::BoldOblique,
        ),
    ]
}

#[test]
fn complete_text_preserves_spaces_unicode_and_ligature_scalars() {
    let report = attest(
        &["A fi", " Ω"],
        regular_hypothesis("Libertinus Serif"),
        vec![
            mapped("F1", &['A']),
            mapped("F1", &[' ']),
            mapped("F1", &['f', 'i']),
            mapped("F1", &[' ']),
            mapped("F1", &['Ω']),
        ],
        vec![resource("F1", Some("LibertinusSerif"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Preserved);
    assert_eq!(report.expected_scalar_count, 6);
    assert_eq!(report.candidate_scalar_count, Some(6));
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.used_resources.len(), 1);
    assert_eq!(report.used_resources[0].resource_name, "F1");
    assert_eq!(
        report.used_resources[0].raw_base_font.as_deref(),
        Some("LibertinusSerif")
    );
    assert_eq!(
        report.used_resources[0].normalized_stem.as_deref(),
        Some("libertinusserif")
    );
    assert_eq!(report.used_resources[0].face, Some(ScanFontFace::Regular));
    assert_eq!(report.used_resources[0].glyph_count, 5);
}

#[test]
fn subset_and_identity_suffixes_are_removed_only_for_comparison() {
    for identity in ["H", "V"] {
        let raw_base_font = format!("ABCDEF+Libertinus_Serif-Regular-Identity-{identity}");
        let report = attest(
            &["x"],
            regular_hypothesis("LIBERTINUS-serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some(&raw_base_font))],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(report.status, EvidenceStatus::Preserved);
        assert_eq!(
            report.used_resources[0].raw_base_font.as_deref(),
            Some(raw_base_font.as_str())
        );
        assert_eq!(
            report.used_resources[0].normalized_stem.as_deref(),
            Some("libertinusserif")
        );
    }

    let inexact_prefix = "AbCDEF+LibertinusSerif-Regular-Identity-H";
    let report = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some(inexact_prefix))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
            resource_name: "F1".to_string(),
            raw_base_font: inexact_prefix.to_string(),
        },
    );
}

#[test]
fn subset_prefix_requires_exactly_six_ascii_uppercase_letters() {
    for raw_base_font in [
        "ABCDE+LibertinusSerif-Regular",
        "ABCDEFG+LibertinusSerif-Regular",
    ] {
        let report = attest(
            &["x"],
            regular_hypothesis("Libertinus Serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some(raw_base_font))],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(report.status, EvidenceStatus::Unknown);
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
                resource_name: "F1".to_string(),
                raw_base_font: raw_base_font.to_string(),
            },
        );
    }
}

#[test]
fn identity_and_face_tokens_are_case_sensitive() {
    for raw_base_font in [
        "LibertinusSerif-Regular-identity-H",
        "LibertinusSerif-Regular-Identity-h",
        "LibertinusSerif-regular",
        "LibertinusSerif-BOLD",
        "LibertinusSerif-Bolditalic",
    ] {
        let report = attest(
            &["x"],
            regular_hypothesis("Libertinus Serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some(raw_base_font))],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            report.status,
            EvidenceStatus::Unknown,
            "accepted case-insensitive structural token {raw_base_font:?}"
        );
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
                resource_name: "F1".to_string(),
                raw_base_font: raw_base_font.to_string(),
            },
        );
    }
}

#[test]
fn all_six_faces_accept_only_the_matching_weight_and_style() {
    let cases = face_cases();

    for &(weight, style, _, expected_face) in &cases {
        for &(_, _, actual_suffix, actual_face) in &cases {
            let raw_base_font = format!("SourceSerif-{actual_suffix}");
            let report = attest(
                &["x"],
                hypothesis("Source Serif", weight, style),
                scalar_glyphs("x", "F1"),
                vec![resource("F1", Some(&raw_base_font))],
                Vec::new(),
                Vec::new(),
            );

            if expected_face == actual_face {
                assert_eq!(
                    report.status,
                    EvidenceStatus::Preserved,
                    "{weight:?}/{style:?} rejected {raw_base_font}"
                );
                assert_eq!(report.used_resources[0].face, Some(actual_face));
            } else {
                assert_eq!(
                    report.status,
                    EvidenceStatus::Violated,
                    "{weight:?}/{style:?} accepted {raw_base_font}"
                );
                assert_has_diagnostic(
                    &report,
                    ScanFontAttestationDiagnostic::FaceMismatch {
                        resource_name: "F1".to_string(),
                        expected: expected_face,
                        actual: actual_face,
                    },
                );
            }
        }
    }
}

#[test]
fn compatible_resources_are_all_counted_sorted_and_catalog_order_independent() {
    let glyphs = vec![
        mapped("F2", &['a']),
        mapped("F1", &['b']),
        mapped("F2", &['c']),
        mapped("F1", &['d']),
    ];
    let first = attest(
        &["abcd"],
        regular_hypothesis("Libertinus Serif"),
        glyphs.clone(),
        vec![
            resource("F2", Some("UVWXYZ+LibertinusSerif-Regular")),
            resource("Unused", Some("FonteNãoClassificável-Regular")),
            resource("F1", Some("ABCDEF+LibertinusSerif-Regular")),
        ],
        Vec::new(),
        Vec::new(),
    );
    let reordered = attest(
        &["abcd"],
        regular_hypothesis("Libertinus Serif"),
        glyphs,
        vec![
            resource("F1", Some("ABCDEF+LibertinusSerif-Regular")),
            resource("F2", Some("UVWXYZ+LibertinusSerif-Regular")),
            resource("Unused", Some("FonteNãoClassificável-Regular")),
        ],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(first, reordered);
    assert_eq!(first.status, EvidenceStatus::Preserved);
    assert_eq!(first.used_resources.len(), 2);
    assert_eq!(first.used_resources[0].resource_name, "F1");
    assert_eq!(first.used_resources[0].glyph_count, 2);
    assert_eq!(first.used_resources[1].resource_name, "F2");
    assert_eq!(first.used_resources[1].glyph_count, 2);
}

#[test]
fn normalized_family_mismatch_is_violated_with_a_public_witness() {
    let report = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("NimbusRoman-Regular"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Violated);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::FamilyMismatch {
            resource_name: "F1".to_string(),
            expected_normalized_family: "libertinusserif".to_string(),
            actual_normalized_stem: "nimbusroman".to_string(),
        },
    );
}

#[test]
fn recognized_variant_mismatch_is_violated_not_unknown() {
    let report = attest(
        &["x"],
        hypothesis(
            "Libertinus Serif",
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Italic,
        ),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif-Italic"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Violated);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::FaceMismatch {
            resource_name: "F1".to_string(),
            expected: ScanFontFace::BoldItalic,
            actual: ScanFontFace::Italic,
        },
    );
}

#[test]
fn nonempty_expected_text_with_an_empty_candidate_is_violated() {
    let report = attest(
        &["abc"],
        regular_hypothesis("Libertinus Serif"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Violated);
    assert_eq!(report.expected_scalar_count, 3);
    assert_eq!(report.candidate_scalar_count, Some(0));
    assert!(report.used_resources.is_empty());
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::TextSequenceMismatch {
            expected_scalar_count: 3,
            candidate_scalar_count: 0,
            first_differing_scalar_index: 0,
        },
    );
}

#[test]
fn sequence_mismatch_reports_the_first_unicode_scalar_index() {
    let report = attest(
        &["éfi", "Z"],
        regular_hypothesis("Libertinus Serif"),
        vec![
            mapped("F1", &['é']),
            mapped("F1", &['f', 'i']),
            mapped("F1", &['Y']),
        ],
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Violated);
    assert_eq!(report.expected_scalar_count, 4);
    assert_eq!(report.candidate_scalar_count, Some(4));
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::TextSequenceMismatch {
            expected_scalar_count: 4,
            candidate_scalar_count: 4,
            first_differing_scalar_index: 3,
        },
    );
}

#[test]
fn an_unmapped_glyph_is_unknown_and_is_never_silently_skipped() {
    let report = attest(
        &["ab"],
        regular_hypothesis("Libertinus Serif"),
        vec![mapped("F1", &['a']), unmapped("F1")],
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Unknown);
    assert_eq!(report.expected_scalar_count, 2);
    assert_eq!(report.candidate_scalar_count, None);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::UnmappedGlyph { glyph_index: 1 },
    );
    assert!(!report.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        ScanFontAttestationDiagnostic::TextSequenceMismatch { .. }
    )));
}

#[test]
fn a_used_resource_must_resolve_exactly_once() {
    let missing = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "Missing"),
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(missing.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &missing,
        ScanFontAttestationDiagnostic::MissingFontResource {
            resource_name: "Missing".to_string(),
            first_glyph_index: 0,
        },
    );

    let duplicate = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![
            resource("F1", Some("LibertinusSerif-Regular")),
            resource("F1", Some("LibertinusSerif-Regular")),
        ],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(duplicate.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &duplicate,
        ScanFontAttestationDiagnostic::DuplicateFontResource {
            resource_name: "F1".to_string(),
            occurrences: 2,
        },
    );
}

#[test]
fn missing_empty_base_font_and_empty_resource_identifiers_are_unknown() {
    let missing = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", None)],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(missing.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &missing,
        ScanFontAttestationDiagnostic::MissingBaseFont {
            resource_name: "F1".to_string(),
        },
    );

    let empty = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some(""))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(empty.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &empty,
        ScanFontAttestationDiagnostic::EmptyBaseFont {
            resource_name: "F1".to_string(),
        },
    );

    let empty_resource_name = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", ""),
        vec![resource("", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(empty_resource_name.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &empty_resource_name,
        ScanFontAttestationDiagnostic::EmptyResourceName,
    );
}

#[test]
fn non_ascii_and_unclassifiable_names_are_unknown() {
    let non_ascii_base = "LíbertinusSerif-Regular";
    let base_report = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some(non_ascii_base))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(base_report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &base_report,
        ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
            resource_name: "F1".to_string(),
            raw_base_font: non_ascii_base.to_string(),
        },
    );

    let family_report = attest(
        &["x"],
        regular_hypothesis("Líbertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(family_report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &family_report,
        ScanFontAttestationDiagnostic::UnclassifiableRequestedFamily {
            raw_family: "Líbertinus Serif".to_string(),
        },
    );

    let unknown_suffix = "LibertinusSerif-Medium";
    let suffix_report = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some(unknown_suffix))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(suffix_report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &suffix_report,
        ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
            resource_name: "F1".to_string(),
            raw_base_font: unknown_suffix.to_string(),
        },
    );
}

#[test]
fn a_required_face_without_a_suffix_is_unknown() {
    let report = attest(
        &["x"],
        hypothesis(
            "Libertinus Serif",
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Oblique,
        ),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif"))],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::MissingRequiredFace {
            resource_name: "F1".to_string(),
            expected: ScanFontFace::BoldOblique,
        },
    );
}

#[test]
fn every_non_regular_face_requires_its_closed_suffix() {
    for &(weight, style, _, expected) in face_cases().iter().skip(1) {
        let report = attest(
            &["x"],
            hypothesis("Libertinus Serif", weight, style),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some("LibertinusSerif"))],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            report.status,
            EvidenceStatus::Unknown,
            "{weight:?}/{style:?} accepted a suffixless /BaseFont"
        );
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::MissingRequiredFace {
                resource_name: "F1".to_string(),
                expected,
            },
        );
    }
}

#[test]
fn nominal_normalization_rejects_punctuation_and_repeated_structural_affixes() {
    let invalid_base_fonts = [
        "Libertinus.Serif-Regular",
        "ABCDEF+GHIJKL+LibertinusSerif-Regular",
        "LibertinusSerif-Regular-Identity-H-Identity-H",
        "LibertinusSerif-Bold-Bold",
    ];

    for raw_base_font in invalid_base_fonts {
        let report = attest(
            &["x"],
            regular_hypothesis("Libertinus Serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some(raw_base_font))],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            report.status,
            EvidenceStatus::Unknown,
            "accepted non-closed nominal structure {raw_base_font:?}"
        );
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::UnclassifiableBaseFont {
                resource_name: "F1".to_string(),
                raw_base_font: raw_base_font.to_string(),
            },
        );
    }

    let requested = "Libertinus.Serif";
    let report = attest(
        &["x"],
        regular_hypothesis(requested),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(report.status, EvidenceStatus::Unknown);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::UnclassifiableRequestedFamily {
            raw_family: requested.to_string(),
        },
    );
}

#[test]
fn every_text_interpreter_diagnostic_makes_clean_evidence_opaque() {
    let diagnostics = [
        TextInterpreterDiagnostic::UnmappedGlyphs,
        TextInterpreterDiagnostic::TextStateNotFullyApplied,
        TextInterpreterDiagnostic::FormXObjectNotTraversed,
        TextInterpreterDiagnostic::MissingFont,
        TextInterpreterDiagnostic::UnsupportedTextOperator,
        TextInterpreterDiagnostic::TextOutsideTextObject,
        TextInterpreterDiagnostic::TrailingGlyphCodeBytes,
        TextInterpreterDiagnostic::InvisibleTextPresent,
    ];

    for diagnostic in diagnostics {
        let report = attest(
            &["x"],
            regular_hypothesis("Libertinus Serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some("LibertinusSerif-Regular"))],
            Vec::new(),
            vec![diagnostic],
        );

        assert_eq!(
            report.status,
            EvidenceStatus::Unknown,
            "{diagnostic:?} did not make extraction opaque"
        );
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::OpaqueTextInterpretation { diagnostic },
        );
    }
}

#[test]
fn incomplete_font_diagnostics_are_opaque_but_no_widths_alone_is_not() {
    for diagnostic in [
        FontModelDiagnostic::UnsupportedEncoding,
        FontModelDiagnostic::PartialTounicode,
    ] {
        let report = attest(
            &["x"],
            regular_hypothesis("Libertinus Serif"),
            scalar_glyphs("x", "F1"),
            vec![resource("F1", Some("LibertinusSerif-Regular"))],
            vec![diagnostic],
            Vec::new(),
        );

        assert_eq!(report.status, EvidenceStatus::Unknown);
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::OpaqueFontModel { diagnostic },
        );
    }

    let no_widths = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif-Regular"))],
        vec![FontModelDiagnostic::NoWidths],
        Vec::new(),
    );
    assert_eq!(no_widths.status, EvidenceStatus::Preserved);
}

#[test]
fn diagnostic_output_is_canonical_not_input_order() {
    let first = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![
            resource("Unused", Some("NimbusRoman-Regular")),
            resource("F1", Some("LibertinusSerif-Regular")),
        ],
        vec![
            FontModelDiagnostic::PartialTounicode,
            FontModelDiagnostic::UnsupportedEncoding,
        ],
        vec![
            TextInterpreterDiagnostic::MissingFont,
            TextInterpreterDiagnostic::FormXObjectNotTraversed,
        ],
    );
    let reversed = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![
            resource("F1", Some("LibertinusSerif-Regular")),
            resource("Unused", Some("NimbusRoman-Regular")),
        ],
        vec![
            FontModelDiagnostic::UnsupportedEncoding,
            FontModelDiagnostic::PartialTounicode,
        ],
        vec![
            TextInterpreterDiagnostic::FormXObjectNotTraversed,
            TextInterpreterDiagnostic::MissingFont,
        ],
    );

    assert_eq!(first.status, EvidenceStatus::Unknown);
    assert_eq!(first, reversed);
}

#[test]
fn independent_violation_takes_precedence_over_opacity() {
    let family_violation = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("NimbusRoman-Regular"))],
        vec![FontModelDiagnostic::PartialTounicode],
        vec![TextInterpreterDiagnostic::FormXObjectNotTraversed],
    );
    assert_eq!(family_violation.status, EvidenceStatus::Violated);
    assert_has_diagnostic(
        &family_violation,
        ScanFontAttestationDiagnostic::FamilyMismatch {
            resource_name: "F1".to_string(),
            expected_normalized_family: "libertinusserif".to_string(),
            actual_normalized_stem: "nimbusroman".to_string(),
        },
    );
    assert_has_diagnostic(
        &family_violation,
        ScanFontAttestationDiagnostic::OpaqueFontModel {
            diagnostic: FontModelDiagnostic::PartialTounicode,
        },
    );
    assert_has_diagnostic(
        &family_violation,
        ScanFontAttestationDiagnostic::OpaqueTextInterpretation {
            diagnostic: TextInterpreterDiagnostic::FormXObjectNotTraversed,
        },
    );

    let face_violation = attest(
        &["x"],
        hypothesis(
            "Libertinus Serif",
            FontWeightHypothesis::Bold,
            FontStyleHypothesis::Normal,
        ),
        scalar_glyphs("x", "F1"),
        vec![resource("F1", Some("LibertinusSerif-Italic"))],
        vec![FontModelDiagnostic::UnsupportedEncoding],
        vec![TextInterpreterDiagnostic::MissingFont],
    );
    assert_eq!(face_violation.status, EvidenceStatus::Violated);
    assert_has_diagnostic(
        &face_violation,
        ScanFontAttestationDiagnostic::FaceMismatch {
            resource_name: "F1".to_string(),
            expected: ScanFontFace::Bold,
            actual: ScanFontFace::Italic,
        },
    );
    assert_has_diagnostic(
        &face_violation,
        ScanFontAttestationDiagnostic::OpaqueFontModel {
            diagnostic: FontModelDiagnostic::UnsupportedEncoding,
        },
    );
    assert_has_diagnostic(
        &face_violation,
        ScanFontAttestationDiagnostic::OpaqueTextInterpretation {
            diagnostic: TextInterpreterDiagnostic::MissingFont,
        },
    );

    let text_violation = attest(
        &["x"],
        regular_hypothesis("Libertinus Serif"),
        scalar_glyphs("y", "Missing"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(text_violation.status, EvidenceStatus::Violated);
    assert_has_diagnostic(
        &text_violation,
        ScanFontAttestationDiagnostic::TextSequenceMismatch {
            expected_scalar_count: 1,
            candidate_scalar_count: 1,
            first_differing_scalar_index: 0,
        },
    );
    assert_has_diagnostic(
        &text_violation,
        ScanFontAttestationDiagnostic::MissingFontResource {
            resource_name: "Missing".to_string(),
            first_glyph_index: 0,
        },
    );
}

#[test]
fn one_incompatible_resource_defeats_a_compatible_majority() {
    let mut glyphs = Vec::new();
    for _ in 0..10 {
        glyphs.push(mapped("Fgood", &['a']));
    }
    glyphs.push(mapped("Zbad", &['b']));

    let report = attest(
        &["aaaaaaaaaa", "b"],
        regular_hypothesis("Libertinus Serif"),
        glyphs,
        vec![
            resource("Fgood", Some("LibertinusSerif-Regular")),
            resource("Zbad", Some("NimbusRoman-Regular")),
        ],
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(report.status, EvidenceStatus::Violated);
    assert_eq!(report.used_resources.len(), 2);
    assert_eq!(report.used_resources[0].resource_name, "Fgood");
    assert_eq!(report.used_resources[0].glyph_count, 10);
    assert_eq!(report.used_resources[1].resource_name, "Zbad");
    assert_eq!(report.used_resources[1].glyph_count, 1);
    assert_has_diagnostic(
        &report,
        ScanFontAttestationDiagnostic::FamilyMismatch {
            resource_name: "Zbad".to_string(),
            expected_normalized_family: "libertinusserif".to_string(),
            actual_normalized_stem: "nimbusroman".to_string(),
        },
    );
}

#[test]
fn incompatible_resources_used_only_by_space_or_ligature_still_violate() {
    let cases = [
        (
            "space",
            "a b",
            vec![
                mapped("Fgood", &['a']),
                mapped("Zbad", &[' ']),
                mapped("Fgood", &['b']),
            ],
        ),
        (
            "ligature",
            "Afi",
            vec![mapped("Fgood", &['A']), mapped("Zbad", &['f', 'i'])],
        ),
    ];

    for (label, expected, glyphs) in cases {
        let report = attest(
            &[expected],
            regular_hypothesis("Libertinus Serif"),
            glyphs,
            vec![
                resource("Fgood", Some("LibertinusSerif-Regular")),
                resource("Zbad", Some("NimbusRoman-Regular")),
            ],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(report.status, EvidenceStatus::Violated, "{label}");
        let bad = report
            .used_resources
            .iter()
            .find(|resource| resource.resource_name == "Zbad")
            .expect("the fallback-only resource must remain visible");
        assert_eq!(bad.glyph_count, 1, "{label}");
        assert_has_diagnostic(
            &report,
            ScanFontAttestationDiagnostic::FamilyMismatch {
                resource_name: "Zbad".to_string(),
                expected_normalized_family: "libertinusserif".to_string(),
                actual_normalized_stem: "nimbusroman".to_string(),
            },
        );
    }
}
