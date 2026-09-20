//! Oracle-first black-box contract for the line reconstruction workflow and
//! its corrective physical-line-integrity obligation.

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_decalque")
}

fn reconstruction_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reconstruction")
        .join(name)
}

fn observation_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures/scan_observation")
        .join(name)
}

fn shared_raster_fixture() -> PathBuf {
    observation_fixture("fixture-raster-v1.pgm")
}

fn temporary_path(label: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "decalque-scan-reconstruction-{}-{serial}-{label}",
        std::process::id()
    ))
}

fn run(args: &[&OsStr]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("the decalque binary must start")
}

fn args<'a>(observation: &'a Path, raster: &'a Path) -> Vec<&'a OsStr> {
    vec![
        "reconstruct-scan-lines".as_ref(),
        observation.as_os_str(),
        "--raster".as_ref(),
        raster.as_os_str(),
        "--font-family".as_ref(),
        "Libertinus Serif".as_ref(),
        "--font-size-pt".as_ref(),
        "10".as_ref(),
        "--font-weight".as_ref(),
        "regular".as_ref(),
        "--font-style".as_ref(),
        "normal".as_ref(),
        "--tracking-pt".as_ref(),
        "0".as_ref(),
    ]
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
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

fn compile_typst(source: &[u8], candidate: &Path) -> Output {
    let mut child = Command::new("typst")
        .args([
            "compile",
            "--format",
            "pdf",
            "--creation-timestamp",
            "0",
            "-",
        ])
        .arg(candidate)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Typst 0.15.x must be available for this integration contract");
    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(source).unwrap();
    }
    child.wait_with_output().unwrap()
}

#[test]
fn known_mapping_and_bound_raster_emit_only_deterministic_typst() {
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");

    let first = run(&args(&observation, &raster));
    let second = run(&args(&observation, &raster));

    assert!(first.status.success(), "{}", stderr(&first));
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let source = String::from_utf8(first.stdout).unwrap();
    assert!(source.starts_with("#set page(width: 144pt, height: 144pt, margin: 0pt)"));
    assert!(source.contains("font: \"Libertinus Serif\""));
    assert!(source.contains("size: 10pt"));
    assert!(source.contains("dx: 18pt, dy: 36pt"));
    assert!(source.contains("#physical-line(text("));
    assert!(!source.contains("#image("));
    assert!(!source.contains("#scale("));
}

#[test]
fn required_and_repeated_options_fail_before_domain_io() {
    let missing_font = run(&[
        "reconstruct-scan-lines".as_ref(),
        "missing.json".as_ref(),
        "--raster".as_ref(),
        "missing.pgm".as_ref(),
        "--font-size-pt".as_ref(),
        "10".as_ref(),
    ]);
    assert_eq!(missing_font.status.code(), Some(2));
    assert!(missing_font.stdout.is_empty());
    assert!(stderr(&missing_font).contains("--font-family"));
    assert!(!stderr(&missing_font).contains("observação:"));

    let repeated = run(&[
        "reconstruct-scan-lines".as_ref(),
        "missing.json".as_ref(),
        "--raster".as_ref(),
        "one.pgm".as_ref(),
        "--raster".as_ref(),
        "two.pgm".as_ref(),
        "--font-family".as_ref(),
        "Font".as_ref(),
        "--font-size-pt".as_ref(),
        "10".as_ref(),
    ]);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(repeated.stdout.is_empty());
    assert!(stderr(&repeated).contains("repetida"));
    assert!(!stderr(&repeated).contains("observação:"));
}

#[test]
fn invalid_hypothesis_values_are_usage_errors() {
    for (option, value) in [
        ("--font-size-pt", "0"),
        ("--font-size-pt", "NaN"),
        ("--tracking-pt", "inf"),
        ("--font-weight", "heavy"),
        ("--font-style", "slanted-ish"),
    ] {
        let observation = reconstruction_fixture("observation.json");
        let raster = reconstruction_fixture("page.pgm");
        let mut command = args(&observation, &raster);
        let position = command
            .iter()
            .position(|argument| *argument == OsStr::new(option))
            .unwrap();
        command[position + 1] = value.as_ref();
        let output = run(&command);

        assert_eq!(output.status.code(), Some(2), "{option}={value}");
        assert!(output.stdout.is_empty(), "{option}={value}");
        assert!(stderr(&output).contains(option) || stderr(&output).contains(value));
        assert!(!stderr(&output).contains("observação:"));
    }
}

#[test]
fn raster_is_bound_before_unknown_mapping_is_evaluated() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    let missing_raster = PathBuf::from("definitely-missing-reconstruction-raster.pgm");

    let output = run(&args(&observation, &missing_raster));

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("raster:"));
    assert!(!stderr(&output).contains("page_mapping"));
}

#[test]
fn unknown_mapping_emits_no_partial_typst() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    let raster = shared_raster_fixture();

    let output = run(&args(&observation, &raster));

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("page_mapping") || error.contains("mapeamento"));
}

#[test]
fn candidate_pdf_and_provider_flags_are_not_part_of_this_command() {
    for (flag, value) in [
        ("--candidate", "candidate.pdf"),
        ("--ocr", "paddle"),
        ("--model", "provider"),
        ("--typst", "typst"),
        ("--page", "0"),
    ] {
        let output = run(&[
            "reconstruct-scan-lines".as_ref(),
            "missing.json".as_ref(),
            "--raster".as_ref(),
            "missing.pgm".as_ref(),
            "--font-family".as_ref(),
            "Font".as_ref(),
            "--font-size-pt".as_ref(),
            "10".as_ref(),
            flag.as_ref(),
            value.as_ref(),
        ]);
        assert_eq!(output.status.code(), Some(2), "{flag}");
        assert!(output.stdout.is_empty(), "{flag}");
        assert!(stderr(&output).contains(flag), "{flag}");
        assert!(!stderr(&output).contains("observação:"), "{flag}");
    }
}

#[test]
fn thirty_point_fixture_remains_one_complete_candidate_line() {
    let observation = reconstruction_fixture("observation.json");
    let raster = reconstruction_fixture("page.pgm");
    let mut reconstruction = args(&observation, &raster);
    let size = reconstruction
        .iter()
        .position(|argument| *argument == OsStr::new("--font-size-pt"))
        .unwrap();
    reconstruction[size + 1] = "30".as_ref();

    let emitted = run(&reconstruction);
    assert!(emitted.status.success(), "{}", stderr(&emitted));
    assert!(emitted.stderr.is_empty());

    let candidate = temporary_path("physical-line.pdf");
    let compiled = compile_typst(&emitted.stdout, &candidate);
    assert!(
        compiled.status.success(),
        "Typst rejected generated source: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let compared = run(&[
        "scan-observation".as_ref(),
        observation.as_os_str(),
        candidate.as_os_str(),
        "--raster".as_ref(),
        raster.as_os_str(),
        "--granularity".as_ref(),
        "line".as_ref(),
        "--horizontal-tolerance-pt".as_ref(),
        "2".as_ref(),
        "--baseline-tolerance-pt".as_ref(),
        "2".as_ref(),
    ]);
    let _ = fs::remove_file(&candidate);

    assert!(compared.status.success(), "{}", stderr(&compared));
    assert!(compared.stderr.is_empty());
    let report = compact_json(&String::from_utf8(compared.stdout).unwrap());
    assert!(report.contains("\"matched_scan\":1"), "{report}");
    assert!(report.contains("\"total_scan\":1"), "{report}");
    assert!(report.contains("\"matched_candidate\":1"), "{report}");
    assert!(report.contains("\"total_candidate\":1"), "{report}");
    assert!(
        report.contains("\"candidate_scalar_range\":[0,16]"),
        "{report}"
    );
    assert!(
        report.contains("\"content_status\":\"preserved\""),
        "{report}"
    );
    assert!(
        report.contains("\"geometry_status\":\"violated\""),
        "{report}"
    );
    assert!(
        report.contains("\"overall_status\":\"violated\""),
        "{report}"
    );
    assert!(
        report.contains("\"baseline_status\":\"unknown\""),
        "{report}"
    );
}
