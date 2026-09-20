//! Oracle-first contract for deterministic Typst source emission, including
//! the corrective physical-line-integrity obligation.

use decalque_core::{
    Claim, FontStyleHypothesis, FontWeightHypothesis, FramedBbox, FramedPolyline, PageFrame,
    ReconstructionCoverage, ReconstructionLine, ReconstructionPlan, TypographyHypothesis,
    UnknownReason,
};
use decalque_infra::render_typst_source;
use std::io::Write;
use std::process::{Command, Stdio};

fn typography(family: &str) -> TypographyHypothesis {
    TypographyHypothesis {
        font_family: family.to_string(),
        size_pt: 10.0,
        weight: FontWeightHypothesis::Regular,
        style: FontStyleHypothesis::Normal,
        tracking_pt: 0.0,
    }
}

fn plan(family: &str, text: &str) -> ReconstructionPlan {
    ReconstructionPlan {
        page: PageFrame {
            id: "source-page-pt".to_string(),
            extent: (144.0, 144.0),
        },
        mapping_max_error_pt: 0.0,
        typography: typography(family),
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

#[test]
fn source_is_a_byte_stable_snapshot_and_preserves_text_exactly() {
    let plan = plan("Libertinus Serif", "  A \"#[] Ω\\B  ");

    let first = render_typst_source(&plan).unwrap();
    let second = render_typst_source(&plan).unwrap();

    let expected = concat!(
        "#set page(width: 144pt, height: 144pt, margin: 0pt)\n",
        "#set text(font: \"Libertinus Serif\", size: 10pt, weight: \"regular\", ",
        "style: \"normal\", tracking: 0pt, hyphenate: false, ",
        "top-edge: \"bounds\", bottom-edge: \"bounds\")\n\n",
        "#let physical-line(body) = context {\n",
        "  let natural-size = measure(body)\n",
        "  box(width: natural-size.width, body)\n",
        "}\n\n",
        "#place(top + left, dx: 0pt, dy: 36pt)[\n",
        "  #physical-line(text(\"  A \\\"#[] Ω\\\\B  \"))\n",
        "]\n",
    );
    assert_eq!(first, expected);
    assert_eq!(second, expected);
}

#[test]
fn strings_cannot_inject_typst_and_source_contains_no_visual_shortcut() {
    let escaped = render_typst_source(&plan("Font \"#[]\\", "#rect[] [raw] \" \\")).unwrap();
    assert!(escaped.contains(r##"font: "Font \"#[]\\""##));
    assert!(escaped.contains(r##"#physical-line(text("#rect[] [raw] \" \\"))"##));

    let source = render_typst_source(&plan("Libertinus Serif", "literal")).unwrap();
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
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden materialization shortcut: {forbidden}"
        );
    }
}

#[test]
fn emitted_source_compiles_with_the_supported_typst_cli() {
    let source = render_typst_source(&plan("Libertinus Serif", "Linha compilável")).unwrap();
    let mut child = Command::new("typst")
        .args([
            "compile",
            "--format",
            "pdf",
            "--creation-timestamp",
            "0",
            "-",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Typst 0.15.x must be available for this integration contract");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "Typst rejected generated source: {}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.starts_with(b"%PDF-"));
}

#[test]
fn explicit_physical_line_separators_are_rejected_defensively() {
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
        let result = render_typst_source(&plan("Libertinus Serif", &format!("a{separator}b")));
        let error = result.expect_err(&format!(
            "{name} must be rejected even in a manually constructed plan"
        ));
        let code_point = format!("U+{:04X}", u32::from(separator));
        assert!(error.contains("line-1"), "{name}: {error}");
        assert!(error.contains(&code_point), "{name}: {error}");
    }
}
