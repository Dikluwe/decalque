//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-font-attestation.md
//! @layer L3
//! @updated 2026-09-19
//!
//! Oraculo L3 para a variante estrita da emissao Typst. Este arquivo fixa
//! somente bytes emitidos e consequencias estruturais observaveis no PDF;
//! nenhum veredito de atestacao pertence a esta camada.

use decalque_core::entities::resolve_page_geometry;
use decalque_core::{
    build_font_model, interpret_text, Claim, FontStyleHypothesis, FontWeightHypothesis, FramedBbox,
    FramedPolyline, GlyphInstance, RawFontData, ReconstructionCoverage, ReconstructionLine,
    ReconstructionPlan, TextInterpretationInput, TypographyHypothesis, UnknownReason,
};
use decalque_infra::{
    compile_typst_candidate, load_single_page_source_from_pdf_bytes, render_strict_typst_source,
    render_typst_source, PageSource, TypstExecutionLimits,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static NEXT_BOUNDARY_DIR: AtomicU64 = AtomicU64::new(0);

struct BoundaryDirectory {
    root: PathBuf,
}

impl BoundaryDirectory {
    fn new() -> Self {
        let serial = NEXT_BOUNDARY_DIR.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "decalque-font-boundary-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    #[cfg(unix)]
    fn install(&self, label: &str, mode: &str) -> PathBuf {
        let directory = self.root.join(label);
        fs::create_dir(&directory).unwrap();
        let executable = directory.join(mode);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../04_wiring/tests/fixtures/font_attestation/fake-typst");
        fs::copy(fixture, &executable).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        executable
    }
}

impl Drop for BoundaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn plan(family: &str, text: &str) -> ReconstructionPlan {
    ReconstructionPlan {
        page: decalque_core::PageFrame {
            id: "source-page-pt".to_string(),
            extent: (144.0, 144.0),
        },
        mapping_max_error_pt: 0.0,
        typography: TypographyHypothesis {
            font_family: family.to_string(),
            size_pt: 10.0,
            weight: FontWeightHypothesis::Regular,
            style: FontStyleHypothesis::Normal,
            tracking_pt: 0.0,
        },
        lines: vec![ReconstructionLine {
            source_unit_id: "line-1".to_string(),
            reading_key: vec![0],
            text: text.to_string(),
            target_bbox: FramedBbox {
                frame_id: "source-page-pt".to_string(),
                x0: -0.0,
                y0: 36.0,
                x1: 126.0,
                y1: 90.0,
            },
            target_baseline: Claim::<FramedPolyline>::Unknown {
                reason: UnknownReason::NotObserved,
                evidence: Vec::new(),
                detail: Some("baseline permanece desconhecida".to_string()),
            },
        }],
        coverage: ReconstructionCoverage {
            planned_lines: 1,
            total_lines: 1,
        },
    }
}

fn execution_limits() -> TypstExecutionLimits {
    TypstExecutionLimits {
        max_source_bytes: 16 * 1024 * 1024,
        max_pdf_stdout_bytes: 128 * 1024 * 1024,
        max_stderr_bytes: 1024 * 1024,
        max_version_stdout_bytes: 64 * 1024,
        timeout: Duration::from_secs(30),
    }
}

fn scaled_boundary_limits() -> TypstExecutionLimits {
    TypstExecutionLimits {
        max_source_bytes: 8,
        max_pdf_stdout_bytes: 8,
        max_stderr_bytes: 8,
        max_version_stdout_bytes: 8,
        timeout: Duration::from_secs(2),
    }
}

fn compile_strict(plan: &ReconstructionPlan) -> Vec<u8> {
    let source = render_strict_typst_source(plan).expect("strict source must render");
    assert!(source.contains("fallback: false"));
    compile_typst_candidate(Path::new("typst"), source.as_bytes(), &execution_limits())
        .expect("Typst 0.15.x must compile the strict source")
        .pdf_bytes
}

#[cfg(unix)]
#[test]
fn byte_limits_are_inclusive_at_n_and_reject_n_plus_one_with_scaled_profiles() {
    let directory = BoundaryDirectory::new();
    let limits = scaled_boundary_limits();

    let source_exact = directory.install("source-exact", "boundary-source");
    compile_typst_candidate(&source_exact, b"12345678", &limits)
        .expect("a source exactly at the inclusive limit must compile");
    assert!(source_exact.with_extension("invoked").exists());

    let source_over = directory.install("source-over", "boundary-source");
    assert!(compile_typst_candidate(&source_over, b"123456789", &limits).is_err());
    assert!(
        !source_over.with_extension("invoked").exists(),
        "source N + 1 must fail before compiler identification"
    );

    for (stage, exact_mode, over_mode) in [
        ("version", "boundary-version-exact", "boundary-version-over"),
        ("stderr", "boundary-stderr-exact", "boundary-stderr-over"),
        ("pdf", "boundary-pdf-exact", "boundary-pdf-over"),
    ] {
        let exact = directory.install(&format!("{stage}-exact"), exact_mode);
        compile_typst_candidate(&exact, b"source", &limits)
            .unwrap_or_else(|error| panic!("{stage} N must be accepted: {error}"));

        let over = directory.install(&format!("{stage}-over"), over_mode);
        assert!(
            compile_typst_candidate(&over, b"source", &limits).is_err(),
            "{stage} N + 1 must be rejected"
        );
    }
}

fn extract_structural_text(pdf: &[u8]) -> (Vec<GlyphInstance>, Vec<RawFontData>) {
    let PageSource {
        box_model,
        operations,
        fonts: raw_fonts,
        xobjects,
        ..
    } = load_single_page_source_from_pdf_bytes(pdf)
        .expect("compiled strict source must yield one structurally readable page");
    let (page, _) = resolve_page_geometry(&box_model);
    let fonts = raw_fonts
        .iter()
        .map(|raw| build_font_model(raw).0)
        .collect();
    let interpreted = interpret_text(&TextInterpretationInput {
        page,
        operations,
        fonts,
        xobjects,
    });
    (interpreted.glyphs, raw_fonts)
}

#[test]
fn strict_fallback_policy_is_unconditional_across_every_face_and_multiple_lines() {
    let faces = [
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Normal),
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Normal),
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Italic),
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Italic),
        (FontWeightHypothesis::Regular, FontStyleHypothesis::Oblique),
        (FontWeightHypothesis::Bold, FontStyleHypothesis::Oblique),
    ];

    for (weight, style) in faces {
        let mut case = plan("Libertinus Serif", "first Ω line");
        case.typography.weight = weight;
        case.typography.style = style;
        let mut second = case.lines[0].clone();
        second.source_unit_id = "line-2".to_string();
        second.reading_key = vec![1];
        second.text = "second fi line".to_string();
        second.target_bbox.y0 = 92.0;
        second.target_bbox.y1 = 112.0;
        case.lines.push(second);
        case.coverage.planned_lines = 2;
        case.coverage.total_lines = 2;

        let legacy = render_typst_source(&case).unwrap();
        let strict = render_strict_typst_source(&case).unwrap();
        assert_eq!(
            strict.matches("fallback: false, ").count(),
            1,
            "strict policy changed for {weight:?}/{style:?}"
        );
        assert_eq!(
            strict.replacen("fallback: false, ", "", 1),
            legacy,
            "strict source gained another delta for {weight:?}/{style:?}"
        );
        assert_eq!(strict.matches("#physical-line(").count(), 2);
    }
}

#[test]
fn strict_emission_keeps_escaping_physical_lines_and_rejects_visual_shortcuts() {
    let escaped = render_strict_typst_source(&plan("Font \"#[]\\", "#rect[] [raw] \" \\"))
        .expect("escaped strings remain valid source data");
    assert!(escaped.contains(r##"font: "Font \"#[]\\""##));
    assert!(escaped.contains(r##"#physical-line(text("#rect[] [raw] \" \\"))"##));

    for (name, separator) in [
        ("LF", '\n'),
        ("CR", '\r'),
        ("VT", '\u{000b}'),
        ("FF", '\u{000c}'),
        ("NEL", '\u{0085}'),
        ("LINE SEPARATOR", '\u{2028}'),
        ("PARAGRAPH SEPARATOR", '\u{2029}'),
    ] {
        let invalid = plan("Libertinus Serif", &format!("a{separator}b"));
        let strict_error = render_strict_typst_source(&invalid)
            .expect_err(&format!("{name} must remain an invalid physical line"));
        let legacy_error = render_typst_source(&invalid).unwrap_err();
        assert_eq!(strict_error, legacy_error, "{name}");
    }

    let plain = render_strict_typst_source(&plan("Libertinus Serif", "literal")).unwrap();
    for forbidden in [
        "10000pt",
        "1000000pt",
        "calc.inf",
        "infinity",
        "#scale(",
        "stretch:",
        "#clip(",
        "clip:",
        "#image(",
        "#hide(",
        "hide:",
        "#raw(",
        "background:",
        "#read(",
        "#import ",
        "#include ",
        "http://",
        "https://",
    ] {
        assert!(
            !plain.contains(forbidden),
            "strict source contains forbidden visual shortcut: {forbidden}"
        );
    }
}

#[test]
fn real_strict_source_compiles_and_exposes_mapped_glyph_resources() {
    let pdf = compile_strict(&plan("Libertinus Serif", "Strict glyph resources"));
    let (glyphs, resources) = extract_structural_text(&pdf);

    assert!(!glyphs.is_empty(), "real text must materialize glyphs");
    assert!(
        glyphs.iter().all(|glyph| glyph.codepoints.is_some()),
        "the supported real fixture must have complete Unicode mappings"
    );

    let used_resources = glyphs
        .iter()
        .map(|glyph| glyph.font_ref.as_str())
        .collect::<BTreeSet<_>>();
    assert!(!used_resources.is_empty());
    for resource_name in used_resources {
        let resource = resources
            .iter()
            .find(|resource| resource.resource_name == resource_name)
            .unwrap_or_else(|| panic!("glyph refers to missing resource {resource_name}"));
        assert!(
            resource
                .base_font
                .as_deref()
                .is_some_and(|base_font| !base_font.is_empty()),
            "used resource {resource_name} must preserve /BaseFont"
        );
    }
}

#[test]
fn nonexistent_strict_family_cannot_fall_back_to_mapped_glyphs() {
    let pdf = compile_strict(&plan(
        "Decalque Definitely Missing Font 7E6DBAF3",
        "Fallback must not appear",
    ));
    let (glyphs, resources) = extract_structural_text(&pdf);
    let mapped = glyphs
        .iter()
        .filter(|glyph| glyph.codepoints.is_some())
        .collect::<Vec<_>>();

    assert!(
        mapped.is_empty(),
        "fallback: false must not materialize mapped glyphs from another family; resources: {:?}",
        resources
            .iter()
            .map(|resource| (&resource.resource_name, &resource.base_font))
            .collect::<Vec<_>>()
    );
}
