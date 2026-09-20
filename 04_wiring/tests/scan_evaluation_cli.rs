//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-candidate-evaluation.md
//! @layer L4
//! @updated 2026-09-19
//!
//! Black-box behavior tests for the complete scan-line evaluation workflow.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "decalque-scan-evaluation-{}-{serial}-{label}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_decalque")
}

fn reconstruction_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reconstruction")
        .join(name)
}

fn evaluation_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/evaluation")
        .join(name)
}

fn observation_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures/scan_observation")
        .join(name)
}

fn infra_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures")
        .join(name)
}

fn run(args: &[OsString]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("the decalque binary must start")
}

fn run_with_marker(args: &[OsString], marker: &Path) -> Output {
    Command::new(binary())
        .args(args)
        .env("DECALQUE_ORACLE_MARKER", marker)
        .output()
        .expect("the decalque binary must start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr must be UTF-8")
}

fn evaluation_args(
    observation: &Path,
    raster: &Path,
    output_pdf: &Path,
    font_size_pt: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("evaluate-scan-lines"),
        observation.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--output-pdf"),
        output_pdf.as_os_str().to_owned(),
        OsString::from("--font-family"),
        OsString::from("Libertinus Serif"),
        OsString::from("--font-size-pt"),
        OsString::from(font_size_pt),
        OsString::from("--font-weight"),
        OsString::from("regular"),
        OsString::from("--font-style"),
        OsString::from("normal"),
        OsString::from("--tracking-pt"),
        OsString::from("0"),
        OsString::from("--granularity"),
        OsString::from("line"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("2"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("2"),
    ]
}

fn reconstruction_args(observation: &Path, raster: &Path, font_size_pt: &str) -> Vec<OsString> {
    vec![
        OsString::from("reconstruct-scan-lines"),
        observation.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--font-family"),
        OsString::from("Libertinus Serif"),
        OsString::from("--font-size-pt"),
        OsString::from(font_size_pt),
        OsString::from("--font-weight"),
        OsString::from("regular"),
        OsString::from("--font-style"),
        OsString::from("normal"),
        OsString::from("--tracking-pt"),
        OsString::from("0"),
    ]
}

fn comparison_args(observation: &Path, raster: &Path, candidate: &Path) -> Vec<OsString> {
    vec![
        OsString::from("scan-observation"),
        observation.as_os_str().to_owned(),
        candidate.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--granularity"),
        OsString::from("line"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("2"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("2"),
    ]
}

fn append_option(args: &mut Vec<OsString>, option: &str, value: &Path) {
    args.push(OsString::from(option));
    args.push(value.as_os_str().to_owned());
}

fn option_position(args: &[OsString], option: &str) -> usize {
    args.iter()
        .position(|argument| argument.as_os_str() == OsStr::new(option))
        .unwrap_or_else(|| panic!("option {option} must be present"))
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut suffixed = path.as_os_str().to_owned();
    suffixed.push(suffix);
    PathBuf::from(suffixed)
}

fn install_fake_compiler(directory: &Path, name: &str, pdf: Option<&Path>) -> PathBuf {
    let executable = directory.join(name);
    fs::copy(evaluation_fixture("fake-typst"), &executable).expect("fake compiler must be copied");

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
    }

    if let Some(pdf) = pdf {
        fs::copy(pdf, append_suffix(&executable, ".pdf"))
            .expect("fake compiler PDF must be copied");
    }
    executable
}

fn compact_json(json: &str) -> String {
    let mut compact = String::with_capacity(json.len());
    let mut in_string = false;
    let mut escaped = false;

    for character in json.chars() {
        if in_string {
            compact.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
        } else if character == '"' {
            in_string = true;
            compact.push(character);
        } else if !character.is_whitespace() {
            compact.push(character);
        }
    }

    compact
}

fn json_object<'a>(json: &'a str, key: &str) -> &'a str {
    let marker = format!("\"{key}\":");
    let start = json
        .find(&marker)
        .map(|position| position + marker.len())
        .unwrap_or_else(|| panic!("JSON field {key} must exist"));
    assert_eq!(json.as_bytes().get(start), Some(&b'{'));

    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in json.as_bytes()[start..].iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &json[start..=start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("JSON object {key} must be complete")
}

fn sha256(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum must start");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(bytes)
        .expect("hash input must be written");
    let output = child.wait_with_output().expect("sha256sum must finish");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn parser_is_complete_before_any_domain_io() {
    let directory = TestDirectory::new("parser");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let compiler = install_fake_compiler(directory.path(), "parser-must-not-run", None);
    let mut base = evaluation_args(
        Path::new("missing-evaluation-observation.json"),
        Path::new("missing-evaluation-raster.pgm"),
        &output_pdf,
        "10",
    );
    append_option(&mut base, "--typst-bin", &compiler);

    let mut missing = base.clone();
    let position = option_position(&missing, "--granularity");
    missing.drain(position..=position + 1);

    let mut repeated = base.clone();
    append_option(&mut repeated, "--typst-bin", &compiler);

    let mut unknown = base.clone();
    unknown.extend([OsString::from("--network"), OsString::from("forbidden")]);

    let mut valueless = base.clone();
    valueless.push(OsString::from("--min-text-confidence"));

    let mut invalid_number = base.clone();
    let position = option_position(&invalid_number, "--font-size-pt");
    invalid_number[position + 1] = OsString::from("NaN");

    let mut invalid_enum = base;
    let position = option_position(&invalid_enum, "--granularity");
    invalid_enum[position + 1] = OsString::from("glyph");

    for (label, args, diagnostic) in [
        ("missing", missing, "--granularity"),
        ("repeated", repeated, "--typst-bin"),
        ("unknown", unknown, "--network"),
        ("valueless", valueless, "--min-text-confidence"),
        ("invalid-number", invalid_number, "--font-size-pt"),
        ("invalid-enum", invalid_enum, "--granularity"),
    ] {
        let marker = directory.path().join(format!("{label}.marker"));
        let result = run_with_marker(&args, &marker);
        let error = stderr(&result);

        assert_eq!(result.status.code(), Some(2), "{label}: {error}");
        assert!(result.stdout.is_empty(), "{label} published stdout");
        assert!(error.contains(diagnostic), "{label}: {error}");
        assert!(!error.contains("erro de leitura"), "{label}: {error}");
        assert!(!error.contains("observação:"), "{label}: {error}");
        assert!(!marker.exists(), "{label} invoked the compiler");
        assert!(!output_pdf.exists(), "{label} published a PDF");
    }
}

#[test]
fn raster_failure_and_unknown_planning_precede_the_compiler() {
    let directory = TestDirectory::new("ordering");
    let compiler = install_fake_compiler(
        directory.path(),
        "ordering-must-not-run",
        Some(&infra_fixture("typst.pdf")),
    );

    let observation = reconstruction_fixture("observation.json");
    let mut changed_bytes = fs::read(reconstruction_fixture("page.pgm")).unwrap();
    *changed_bytes.last_mut().unwrap() ^= 1;
    let changed_raster = directory.path().join("changed.pgm");
    fs::write(&changed_raster, changed_bytes).unwrap();
    let raster_pdf = directory.path().join("raster.pdf");
    let raster_marker = directory.path().join("raster.marker");
    let mut raster_args = evaluation_args(&observation, &changed_raster, &raster_pdf, "10");
    append_option(&mut raster_args, "--typst-bin", &compiler);
    let raster_result = run_with_marker(&raster_args, &raster_marker);

    let unknown_observation = observation_fixture("valid_mixed_known_unknown.json");
    let shared_raster = observation_fixture("fixture-raster-v1.pgm");
    let planning_pdf = directory.path().join("planning.pdf");
    let planning_marker = directory.path().join("planning.marker");
    let mut planning_args =
        evaluation_args(&unknown_observation, &shared_raster, &planning_pdf, "10");
    append_option(&mut planning_args, "--typst-bin", &compiler);
    let planning_result = run_with_marker(&planning_args, &planning_marker);

    let raster_error = stderr(&raster_result);
    assert_eq!(raster_result.status.code(), Some(2), "{raster_error}");
    assert!(raster_result.stdout.is_empty());
    assert!(raster_error.contains("SHA-256") || raster_error.contains("raster:"));
    assert!(!raster_error.to_lowercase().contains("desconhecid"));
    assert!(!raster_marker.exists());
    assert!(!raster_pdf.exists());

    let planning_error = stderr(&planning_result);
    assert_eq!(planning_result.status.code(), Some(2), "{planning_error}");
    assert!(planning_result.stdout.is_empty());
    assert!(
        planning_error.contains("page_mapping")
            || planning_error.contains("mapeamento")
            || planning_error.contains("planejamento"),
        "{planning_error}"
    );
    assert!(!planning_marker.exists());
    assert!(!planning_pdf.exists());
}

#[test]
fn real_ten_point_evaluation_is_deterministic_and_embeds_the_complete_report() {
    let directory = TestDirectory::new("real-ten-point");
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");
    let first_pdf = directory.path().join("first.pdf");
    let second_pdf = directory.path().join("second.pdf");

    let with_hundred_point_horizontal_tolerance = |mut args: Vec<OsString>| {
        let position = option_position(&args, "--horizontal-tolerance-pt");
        args[position + 1] = OsString::from("100");
        args
    };

    let first_args = with_hundred_point_horizontal_tolerance(evaluation_args(
        &observation,
        &raster,
        &first_pdf,
        "10",
    ));
    let second_args = with_hundred_point_horizontal_tolerance(evaluation_args(
        &observation,
        &raster,
        &second_pdf,
        "10",
    ));
    let first = run(&first_args);
    let second = run(&second_args);

    assert!(first.status.success(), "{}", stderr(&first));
    assert!(second.status.success(), "{}", stderr(&second));
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let first_pdf_bytes = fs::read(&first_pdf).expect("first PDF must be published");
    let second_pdf_bytes = fs::read(&second_pdf).expect("second PDF must be published");
    assert!(first_pdf_bytes.starts_with(b"%PDF"));
    assert_eq!(first_pdf_bytes, second_pdf_bytes);

    let report = compact_json(String::from_utf8(first.stdout).unwrap().as_str());
    assert!(report.starts_with(
        "{\"schema\":\"decalque.scan-reconstruction-evaluation\",\"schema_version\":1,"
    ));

    let hypothesis = json_object(&report, "hypothesis");
    for field in [
        "\"font_family\":\"Libertinus Serif\"",
        "\"font_size_pt\":10",
        "\"font_weight\":\"regular\"",
        "\"font_style\":\"normal\"",
        "\"tracking_pt\":0",
    ] {
        assert!(hypothesis.contains(field), "missing {field}: {hypothesis}");
    }

    let version = Command::new("typst")
        .arg("--version")
        .output()
        .expect("real Typst must start");
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(report.contains(&format!("\"compiler_version\":\"{}\"", version.trim_end())));

    let source = run(&reconstruction_args(&observation, &raster, "10"));
    assert!(source.status.success(), "{}", stderr(&source));
    assert!(source.stderr.is_empty());
    assert!(report.contains(&format!("\"source_sha256\":\"{}\"", sha256(&source.stdout))));
    assert!(report.contains(&format!("\"source_size_bytes\":{}", source.stdout.len())));
    assert!(report.contains(&format!("\"pdf_sha256\":\"{}\"", sha256(&first_pdf_bytes))));
    assert!(report.contains(&format!("\"pdf_size_bytes\":{}", first_pdf_bytes.len())));

    let standalone_args =
        with_hundred_point_horizontal_tolerance(comparison_args(&observation, &raster, &first_pdf));
    let compared = run(&standalone_args);
    assert!(compared.status.success(), "{}", stderr(&compared));
    assert!(compared.stderr.is_empty());
    let standalone_comparison = compact_json(&String::from_utf8(compared.stdout).unwrap());
    let embedded_comparison = json_object(&report, "comparison");
    assert_eq!(embedded_comparison, standalone_comparison);
    assert!(embedded_comparison.contains("\"overall_status\":\"unknown\""));
    assert!(embedded_comparison.contains("\"baseline_status\":\"unknown\""));

    assert!(!report.contains(&first_pdf.to_string_lossy().to_string()));
    assert!(!report.contains(&second_pdf.to_string_lossy().to_string()));
}

#[test]
fn known_baseline_can_be_published_as_preserved() {
    let directory = TestDirectory::new("preserved");
    let observation = evaluation_fixture("observation-known-baseline.json");
    let raster = reconstruction_fixture("page.pgm");
    let candidate = directory.path().join("preserved.pdf");
    let mut args = evaluation_args(&observation, &raster, &candidate, "10");

    for option in ["--horizontal-tolerance-pt", "--baseline-tolerance-pt"] {
        let position = option_position(&args, option);
        args[position + 1] = OsString::from("200");
    }

    let result = run(&args);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert!(fs::read(&candidate).unwrap().starts_with(b"%PDF"));

    let report = compact_json(&String::from_utf8(result.stdout).unwrap());
    let comparison = json_object(&report, "comparison");
    assert!(comparison.contains("\"content_status\":\"preserved\""));
    assert!(comparison.contains("\"geometry_status\":\"preserved\""));
    assert!(comparison.contains("\"overall_status\":\"preserved\""));
}

#[test]
fn thirty_point_violation_is_a_successful_complete_iteration() {
    let directory = TestDirectory::new("thirty-point");
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");
    let candidate = directory.path().join("candidate.pdf");

    let result = run(&evaluation_args(&observation, &raster, &candidate, "30"));
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert!(fs::read(&candidate).unwrap().starts_with(b"%PDF"));

    let report = compact_json(&String::from_utf8(result.stdout).unwrap());
    let comparison = json_object(&report, "comparison");
    for field in [
        "\"matched_scan\":1",
        "\"total_scan\":1",
        "\"matched_candidate\":1",
        "\"total_candidate\":1",
        "\"candidate_scalar_range\":[0,16]",
        "\"content_status\":\"preserved\"",
        "\"geometry_status\":\"violated\"",
        "\"overall_status\":\"violated\"",
        "\"baseline_status\":\"unknown\"",
    ] {
        assert!(comparison.contains(field), "missing {field}: {comparison}");
    }
}

#[test]
fn an_existing_destination_is_never_changed() {
    let directory = TestDirectory::new("existing-destination");
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");
    let destination = directory.path().join("already-exists.pdf");
    let sentinel = b"pre-existing bytes must survive";
    fs::write(&destination, sentinel).unwrap();

    let result = run(&evaluation_args(&observation, &raster, &destination, "10"));
    let error = stderr(&result);

    assert_eq!(result.status.code(), Some(2), "{error}");
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap().as_slice(), sentinel);
    assert!(!error.is_empty());
}

#[test]
fn compiler_and_invalid_pdf_failures_publish_nothing() {
    let directory = TestDirectory::new("execution-failures");
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");

    for (mode, expected_diagnostic) in [("compile-nonzero", "compil"), ("invalid-pdf", "pdf")] {
        let compiler = install_fake_compiler(directory.path(), mode, None);
        let destination = directory.path().join(format!("{mode}.pdf"));
        let marker = directory.path().join(format!("{mode}.marker"));
        let mut args = evaluation_args(&observation, &raster, &destination, "10");
        append_option(&mut args, "--typst-bin", &compiler);

        let result = run_with_marker(&args, &marker);
        let error = stderr(&result);
        assert_eq!(result.status.code(), Some(2), "{mode}: {error}");
        assert!(result.stdout.is_empty(), "{mode} published stdout");
        assert!(!destination.exists(), "{mode} published a PDF");
        assert!(marker.exists(), "{mode} did not reach the compiler");
        let normalized_error = error.to_lowercase();
        assert!(
            normalized_error.contains(expected_diagnostic)
                || (mode == "compile-nonzero" && normalized_error.contains("typst")),
            "{mode}: {error}"
        );
    }
}

#[test]
fn typst_bin_is_one_literal_path_even_with_spaces_and_metacharacters() {
    let directory = TestDirectory::new("literal-compiler-path");
    let compiler = install_fake_compiler(
        directory.path(),
        "typst oracle; exit 77",
        Some(&infra_fixture("typst.pdf")),
    );
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");
    let destination = directory.path().join("literal-path.pdf");
    let marker = directory.path().join("literal-path.marker");
    let mut args = evaluation_args(&observation, &raster, &destination, "10");
    append_option(&mut args, "--typst-bin", &compiler);

    let result = run_with_marker(&args, &marker);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert!(marker.exists());
    assert_eq!(
        fs::read(&destination).unwrap(),
        fs::read(infra_fixture("typst.pdf")).unwrap()
    );

    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("\"compiler_version\":\"typst-oracle 0.15.1\""));
    assert!(!report.contains(compiler.to_string_lossy().as_ref()));
    assert!(!report.contains(destination.to_string_lossy().as_ref()));
}

#[test]
fn reconstruct_scan_lines_remains_byte_identical() {
    const EXPECTED: &str = r##"#set page(width: 144pt, height: 144pt, margin: 0pt)
#set text(font: "Libertinus Serif", size: 10pt, weight: "regular", style: "normal", tracking: 0pt, hyphenate: false, top-edge: "bounds", bottom-edge: "bounds")

#let physical-line(body) = context {
  let natural-size = measure(body)
  box(width: natural-size.width, body)
}

#place(top + left, dx: 18pt, dy: 36pt)[
  #physical-line(text("  A \\\"#[] Ω\\nB  "))
]
"##;

    let result = run(&reconstruction_args(
        &reconstruction_fixture("observation.json"),
        &reconstruction_fixture("page.pgm"),
        "10",
    ));

    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, EXPECTED.as_bytes());
}
