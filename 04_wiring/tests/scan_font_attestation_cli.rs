//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-font-attestation.md
//! @layer L4
//! @updated 2026-09-19
//!
//! Black-box behavior tests for structural Typst font attestation.

use serde_json::{Map, Value};
use std::collections::BTreeSet;
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
        let path = PathBuf::from("/tmp").join(format!(
            "decalque-font-attestation-{}-{serial}-{label}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory must be created below /tmp");
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

fn fixture(area: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(area)
        .join(name)
}

fn infra_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures")
        .join(name)
}

fn install_oracle(directory: &Path) -> PathBuf {
    let executable = directory.join("typst-font-oracle");
    fs::copy(fixture("font_attestation", "fake-typst"), &executable)
        .expect("font oracle compiler must be copied");

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
    }

    executable
}

fn run(args: &[OsString]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("the decalque binary must start")
}

fn run_with_oracle(
    args: &[OsString],
    state: &Path,
    mode: &str,
    fixed_pdf: Option<&Path>,
) -> Output {
    fs::create_dir_all(state).expect("oracle state directory must be created");
    let mut command = Command::new(binary());
    command
        .args(args)
        .env("DECALQUE_FONT_ORACLE_DIR", state)
        .env("DECALQUE_FONT_ORACLE_MODE", mode);
    if let Some(fixed_pdf) = fixed_pdf {
        command.env("DECALQUE_FONT_ORACLE_PDF", fixed_pdf);
    }
    command.output().expect("the decalque binary must start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr must be UTF-8")
}

fn attestation_args(
    observation: &Path,
    raster: &Path,
    output_pdf: &Path,
    family: &str,
    compiler: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("attest-scan-font"),
        observation.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--output-pdf"),
        output_pdf.as_os_str().to_owned(),
        OsString::from("--font-family"),
        OsString::from(family),
        OsString::from("--font-size-pt"),
        OsString::from("10"),
        OsString::from("--font-weight"),
        OsString::from("regular"),
        OsString::from("--font-style"),
        OsString::from("normal"),
        OsString::from("--tracking-pt"),
        OsString::from("0"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("200"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("200"),
        OsString::from("--typst-bin"),
        compiler.as_os_str().to_owned(),
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
        OsString::from("200"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("200"),
    ]
}

fn option_position(args: &[OsString], option: &str) -> usize {
    args.iter()
        .position(|argument| argument.as_os_str() == OsStr::new(option))
        .unwrap_or_else(|| panic!("option {option} must be present"))
}

fn counter(state: &Path, name: &str) -> usize {
    fs::read_to_string(state.join(name))
        .ok()
        .map(|value| value.trim().parse().expect("counter must be numeric"))
        .unwrap_or(0)
}

fn captured_source(state: &Path, number: usize) -> Vec<u8> {
    fs::read(state.join(format!("source-{number:02}.typ")))
        .expect("the requested Typst source must have been captured")
}

fn events(state: &Path) -> String {
    fs::read_to_string(state.join("events.log")).unwrap_or_default()
}

fn parse_exact_json(bytes: &[u8]) -> Value {
    assert_eq!(bytes.first(), Some(&b'{'), "JSON must start at byte zero");
    assert_eq!(bytes.last(), Some(&b'}'), "JSON must have no trailing LF");
    serde_json::from_slice(bytes).expect("stdout must contain exactly one JSON value")
}

fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object"))
}

fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{context} must be an array"))
}

fn field<'a>(value: &'a Map<String, Value>, name: &str, context: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| panic!("{context}.{name} must exist"))
}

fn exact_fields(value: &Map<String, Value>, expected: &[&str], context: &str) {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{context} fields changed");
}

fn string<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a string"))
}

fn unsigned(value: &Value, context: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{context} must be an unsigned integer"))
}

fn boolean(value: &Value, context: &str) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| panic!("{context} must be a boolean"))
}

fn assert_sha256(value: &Value, context: &str) {
    let value = string(value, context);
    assert_eq!(value.len(), 64, "{context} must have 64 hexadecimal bytes");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{context} must be hexadecimal"
    );
}

fn assert_attestation_report_v1(value: &Value, family: &str, status: &str, published: bool) {
    let root = object(value, "report");
    exact_fields(
        root,
        &[
            "schema",
            "schema_version",
            "source",
            "hypothesis",
            "fallback_policy",
            "compiler_version",
            "source_sha256",
            "source_size_bytes",
            "pdf_sha256",
            "pdf_size_bytes",
            "attestation",
            "comparison",
            "artifact_published",
        ],
        "report",
    );
    assert_eq!(
        string(field(root, "schema", "report"), "report.schema"),
        "decalque.scan-font-attestation"
    );
    assert_eq!(
        unsigned(
            field(root, "schema_version", "report"),
            "report.schema_version"
        ),
        1
    );

    let source = object(field(root, "source", "report"), "report.source");
    exact_fields(source, &["page_index", "raster_sha256"], "report.source");
    assert_eq!(
        unsigned(
            field(source, "page_index", "report.source"),
            "report.source.page_index"
        ),
        0
    );
    assert_sha256(
        field(source, "raster_sha256", "report.source"),
        "report.source.raster_sha256",
    );

    let hypothesis = object(field(root, "hypothesis", "report"), "report.hypothesis");
    exact_fields(
        hypothesis,
        &[
            "font_family",
            "font_size_pt",
            "font_weight",
            "font_style",
            "tracking_pt",
        ],
        "report.hypothesis",
    );
    assert_eq!(
        string(
            field(hypothesis, "font_family", "report.hypothesis"),
            "report.hypothesis.font_family"
        ),
        family
    );
    assert_eq!(
        field(hypothesis, "font_size_pt", "report.hypothesis"),
        &Value::from(10)
    );
    assert_eq!(
        string(
            field(hypothesis, "font_weight", "report.hypothesis"),
            "report.hypothesis.font_weight"
        ),
        "regular"
    );
    assert_eq!(
        string(
            field(hypothesis, "font_style", "report.hypothesis"),
            "report.hypothesis.font_style"
        ),
        "normal"
    );
    assert_eq!(
        field(hypothesis, "tracking_pt", "report.hypothesis"),
        &Value::from(0)
    );

    assert_eq!(
        string(
            field(root, "fallback_policy", "report"),
            "report.fallback_policy"
        ),
        "forbidden"
    );
    assert!(!string(
        field(root, "compiler_version", "report"),
        "report.compiler_version"
    )
    .is_empty());
    assert_sha256(
        field(root, "source_sha256", "report"),
        "report.source_sha256",
    );
    assert!(
        unsigned(
            field(root, "source_size_bytes", "report"),
            "report.source_size_bytes"
        ) > 0
    );
    assert_sha256(field(root, "pdf_sha256", "report"), "report.pdf_sha256");
    assert!(
        unsigned(
            field(root, "pdf_size_bytes", "report"),
            "report.pdf_size_bytes"
        ) > 0
    );

    let attestation = object(field(root, "attestation", "report"), "report.attestation");
    exact_fields(
        attestation,
        &[
            "status",
            "expected_scalar_count",
            "candidate_scalar_count",
            "diagnostics",
            "used_resources",
        ],
        "report.attestation",
    );
    assert_eq!(
        string(
            field(attestation, "status", "report.attestation"),
            "report.attestation.status"
        ),
        status
    );
    unsigned(
        field(attestation, "expected_scalar_count", "report.attestation"),
        "report.attestation.expected_scalar_count",
    );
    let candidate_scalar_count = field(attestation, "candidate_scalar_count", "report.attestation");
    assert!(
        candidate_scalar_count.is_null() || candidate_scalar_count.as_u64().is_some(),
        "report.attestation.candidate_scalar_count must be null or an unsigned integer"
    );
    for (index, diagnostic) in array(
        field(attestation, "diagnostics", "report.attestation"),
        "report.attestation.diagnostics",
    )
    .iter()
    .enumerate()
    {
        let diagnostic = object(diagnostic, &format!("diagnostic {index}"));
        assert!(
            diagnostic
                .get("code")
                .and_then(Value::as_str)
                .is_some_and(|code| !code.is_empty()),
            "diagnostic {index} must expose a stable code"
        );
    }
    for (index, resource) in array(
        field(attestation, "used_resources", "report.attestation"),
        "report.attestation.used_resources",
    )
    .iter()
    .enumerate()
    {
        let context = format!("report.attestation.used_resources[{index}]");
        let resource = object(resource, &context);
        exact_fields(
            resource,
            &[
                "resource_name",
                "base_font",
                "normalized_stem",
                "glyph_count",
            ],
            &context,
        );
        assert!(!string(field(resource, "resource_name", &context), &context).is_empty());
        let base_font = field(resource, "base_font", &context);
        assert!(
            base_font.is_null()
                || base_font
                    .as_str()
                    .is_some_and(|base_font| !base_font.is_empty()),
            "{context}.base_font must be null or a non-empty string"
        );
        let normalized_stem = field(resource, "normalized_stem", &context);
        assert!(normalized_stem.is_null() || normalized_stem.is_string());
        assert!(unsigned(field(resource, "glyph_count", &context), &context) > 0);
    }

    let comparison = object(field(root, "comparison", "report"), "report.comparison");
    assert_eq!(
        string(
            field(comparison, "schema", "report.comparison"),
            "report.comparison.schema"
        ),
        "decalque.scan-comparison-report"
    );
    assert_eq!(
        unsigned(
            field(comparison, "schema_version", "report.comparison"),
            "report.comparison.schema_version"
        ),
        1
    );
    assert_eq!(
        boolean(
            field(root, "artifact_published", "report"),
            "report.artifact_published"
        ),
        published
    );
}

fn attestation(report: &Value) -> &Map<String, Value> {
    let root = object(report, "report");
    object(field(root, "attestation", "report"), "report.attestation")
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

fn assert_no_temporary_publication(directory: &Path) {
    let leftovers = fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("decalque-tmp"))
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "temporary publications leaked: {leftovers:?}"
    );
}

#[test]
fn help_and_closed_parser_finish_before_domain_io_or_compiler_invocation() {
    let directory = TestDirectory::new("parser");
    let compiler = install_oracle(directory.path());
    let help_state = directory.path().join("help-state");
    let help = run_with_oracle(
        &[OsString::from("attest-scan-font"), OsString::from("--help")],
        &help_state,
        "trace",
        None,
    );
    let help_text = String::from_utf8(help.stdout.clone()).expect("help must be UTF-8");
    assert!(help.status.success(), "{}", stderr(&help));
    assert!(help.stderr.is_empty());
    assert!(help_text.starts_with("uso: decalque attest-scan-font <observacao.json> "));
    for option in [
        "--raster",
        "--output-pdf",
        "--font-family",
        "--font-size-pt",
        "--horizontal-tolerance-pt",
        "--baseline-tolerance-pt",
        "--typst-bin",
    ] {
        assert!(
            help_text.contains(option),
            "help omitted {option}: {help_text}"
        );
    }
    assert!(
        !help_text.contains("--granularity"),
        "line granularity must not become configurable"
    );
    assert_eq!(counter(&help_state, "version.count"), 0);
    assert_eq!(counter(&help_state, "compile.count"), 0);

    let output_pdf = directory.path().join("must-not-exist.pdf");
    let base = attestation_args(
        Path::new("missing-font-observation.json"),
        Path::new("missing-font-raster.pgm"),
        &output_pdf,
        "Libertinus Serif",
        &compiler,
    );

    let mut missing = base.clone();
    let position = option_position(&missing, "--baseline-tolerance-pt");
    missing.drain(position..=position + 1);

    let mut repeated = base.clone();
    repeated.extend([
        OsString::from("--typst-bin"),
        compiler.as_os_str().to_owned(),
    ]);

    let mut unknown = base.clone();
    unknown.extend([OsString::from("--network"), OsString::from("forbidden")]);

    let mut valueless = base.clone();
    valueless.push(OsString::from("--min-text-confidence"));

    let mut empty_family = base.clone();
    let position = option_position(&empty_family, "--font-family");
    empty_family[position + 1] = OsString::new();

    let mut invalid_number = base.clone();
    let position = option_position(&invalid_number, "--font-size-pt");
    invalid_number[position + 1] = OsString::from("NaN");

    let mut invalid_size = base.clone();
    let position = option_position(&invalid_size, "--font-size-pt");
    invalid_size[position + 1] = OsString::from("0");

    let mut negative_tolerance = base.clone();
    let position = option_position(&negative_tolerance, "--horizontal-tolerance-pt");
    negative_tolerance[position + 1] = OsString::from("-0.01");

    let mut invalid_confidence = base.clone();
    invalid_confidence.extend([
        OsString::from("--min-geometry-confidence"),
        OsString::from("1.01"),
    ]);

    let mut configurable_granularity = base;
    configurable_granularity.extend([OsString::from("--granularity"), OsString::from("line")]);

    for (label, args, diagnostic) in [
        ("missing", missing, "--baseline-tolerance-pt"),
        ("repeated", repeated, "--typst-bin"),
        ("unknown", unknown, "--network"),
        ("valueless", valueless, "--min-text-confidence"),
        ("empty-family", empty_family, "--font-family"),
        ("invalid-number", invalid_number, "--font-size-pt"),
        ("invalid-size", invalid_size, "--font-size-pt"),
        (
            "negative-tolerance",
            negative_tolerance,
            "--horizontal-tolerance-pt",
        ),
        (
            "invalid-confidence",
            invalid_confidence,
            "--min-geometry-confidence",
        ),
        (
            "configurable-granularity",
            configurable_granularity,
            "--granularity",
        ),
    ] {
        let state = directory.path().join(format!("{label}-state"));
        let result = run_with_oracle(&args, &state, "trace", None);
        let error = stderr(&result);
        assert_eq!(result.status.code(), Some(2), "{label}: {error}");
        assert!(result.stdout.is_empty(), "{label} published stdout");
        assert!(error.contains(diagnostic), "{label}: {error}");
        assert!(!error.contains("observação:"), "{label}: {error}");
        assert_eq!(counter(&state, "version.count"), 0, "{label}");
        assert_eq!(counter(&state, "compile.count"), 0, "{label}");
        assert!(!output_pdf.exists(), "{label} published a PDF");
    }
}

#[test]
fn raster_binding_and_planning_finish_before_compiler_identification() {
    let directory = TestDirectory::new("causal-prefix");
    let compiler = install_oracle(directory.path());

    let missing_raster_state = directory.path().join("missing-raster-state");
    let missing_raster_output = directory.path().join("missing-raster.pdf");
    let missing_raster = run_with_oracle(
        &attestation_args(
            &fixture("evaluation", "observation-known-baseline.json"),
            &directory.path().join("absent-raster.pgm"),
            &missing_raster_output,
            "Libertinus Serif",
            &compiler,
        ),
        &missing_raster_state,
        "trace",
        None,
    );
    assert_eq!(missing_raster.status.code(), Some(2));
    assert!(missing_raster.stdout.is_empty());
    assert_eq!(counter(&missing_raster_state, "version.count"), 0);
    assert_eq!(counter(&missing_raster_state, "compile.count"), 0);
    assert!(!missing_raster_output.exists());

    let unknown_plan_state = directory.path().join("unknown-plan-state");
    let unknown_plan_output = directory.path().join("unknown-plan.pdf");
    let unknown_plan = run_with_oracle(
        &attestation_args(
            &infra_fixture("scan_observation/valid_mixed_known_unknown.json"),
            &infra_fixture("scan_observation/fixture-raster-v1.pgm"),
            &unknown_plan_output,
            "Libertinus Serif",
            &compiler,
        ),
        &unknown_plan_state,
        "trace",
        None,
    );
    assert_eq!(unknown_plan.status.code(), Some(2));
    assert!(unknown_plan.stdout.is_empty());
    assert_eq!(counter(&unknown_plan_state, "version.count"), 0);
    assert_eq!(counter(&unknown_plan_state, "compile.count"), 0);
    assert!(!unknown_plan_output.exists());
}

#[test]
fn real_libertinus_preserved_is_published_and_reported() {
    let directory = TestDirectory::new("real-preserved");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let observation = fixture("evaluation", "observation-known-baseline.json");
    let raster = fixture("reconstruction", "page.pgm");
    let output_pdf = directory.path().join("verified.pdf");
    let args = attestation_args(
        &observation,
        &raster,
        &output_pdf,
        "Libertinus Serif",
        &compiler,
    );

    let result = run_with_oracle(&args, &state, "trace", None);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert!(result.stdout.starts_with(
        b"{\"schema\":\"decalque.scan-font-attestation\",\"schema_version\":1,\"source\":"
    ));
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 1);
    assert_eq!(events(&state), "version-1\ncompile-1\n");

    let first_source = captured_source(&state, 1);
    let expected_strict_source = br##"#set page(width: 144pt, height: 144pt, margin: 0pt)
#set text(font: "Libertinus Serif", size: 10pt, weight: "regular", style: "normal", tracking: 0pt, fallback: false, hyphenate: false, top-edge: "bounds", bottom-edge: "bounds")

#let physical-line(body) = context {
  let natural-size = measure(body)
  box(width: natural-size.width, body)
}

#place(top + left, dx: 18pt, dy: 36pt)[
  #physical-line(text("Decalque"))
]
"##
    .to_vec();
    assert_eq!(first_source, expected_strict_source);
    assert_eq!(
        String::from_utf8_lossy(&first_source)
            .matches("fallback: false")
            .count(),
        1,
        "the strict source must forbid fallback exactly once"
    );

    let pdf = fs::read(&output_pdf).expect("verified PDF must be published");
    assert!(pdf.starts_with(b"%PDF"));
    let report = parse_exact_json(&result.stdout);
    assert_attestation_report_v1(&report, "Libertinus Serif", "preserved", true);

    let root = object(&report, "report");
    assert_eq!(
        string(field(root, "source_sha256", "report"), "source_sha256"),
        sha256(&first_source)
    );
    assert_eq!(
        unsigned(field(root, "source_size_bytes", "report"), "source_size"),
        first_source.len() as u64
    );
    assert_eq!(
        string(field(root, "pdf_sha256", "report"), "pdf_sha256"),
        sha256(&pdf)
    );
    assert_eq!(
        unsigned(field(root, "pdf_size_bytes", "report"), "pdf_size"),
        pdf.len() as u64
    );

    let attestation = attestation(&report);
    assert_eq!(
        unsigned(
            field(attestation, "expected_scalar_count", "attestation"),
            "expected_scalar_count"
        ),
        8
    );
    assert_eq!(
        unsigned(
            field(attestation, "candidate_scalar_count", "attestation"),
            "candidate_scalar_count"
        ),
        8
    );
    assert!(array(
        field(attestation, "diagnostics", "attestation"),
        "diagnostics"
    )
    .is_empty());
    let resources = array(
        field(attestation, "used_resources", "attestation"),
        "used_resources",
    );
    assert!(!resources.is_empty());
    for resource in resources {
        let resource = object(resource, "used resource");
        assert!(
            string(field(resource, "base_font", "resource"), "base_font")
                .contains("LibertinusSerif")
        );
        assert_eq!(
            string(
                field(resource, "normalized_stem", "resource"),
                "normalized_stem"
            ),
            "libertinusserif"
        );
    }

    let standalone = run(&comparison_args(&observation, &raster, &output_pdf));
    assert!(standalone.status.success(), "{}", stderr(&standalone));
    assert!(standalone.stderr.is_empty());
    let standalone: Value = serde_json::from_slice(&standalone.stdout).unwrap();
    assert_eq!(field(root, "comparison", "report"), &standalone);

    let report_text = String::from_utf8(result.stdout).unwrap();
    for path in [&observation, &raster, &output_pdf, &compiler, &state] {
        assert!(
            !report_text.contains(path.to_string_lossy().as_ref()),
            "report leaked path {}",
            path.display()
        );
    }
}

#[test]
fn a_multi_page_candidate_is_rejected_instead_of_selecting_one_page() {
    let directory = TestDirectory::new("multi-page");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let result = run_with_oracle(
        &attestation_args(
            &fixture("evaluation", "observation-known-baseline.json"),
            &fixture("reconstruction", "page.pgm"),
            &output_pdf,
            "Libertinus Serif",
            &compiler,
        ),
        &state,
        "multi-page",
        None,
    );

    assert_eq!(result.status.code(), Some(2), "{}", stderr(&result));
    assert!(result.stdout.is_empty());
    assert!(!output_pdf.exists());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 1);
    assert_no_temporary_publication(directory.path());
}

#[test]
fn missing_family_is_violated_once_and_never_published() {
    let directory = TestDirectory::new("missing-family");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let observation = fixture("evaluation", "observation-known-baseline.json");
    let raster = fixture("reconstruction", "page.pgm");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let family = "Decalque Definitely Missing Font 7F9A";

    let result = run_with_oracle(
        &attestation_args(&observation, &raster, &output_pdf, family, &compiler),
        &state,
        "trace",
        None,
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 1);
    assert!(!output_pdf.exists());
    assert!(String::from_utf8_lossy(&captured_source(&state, 1)).contains("fallback: false"));

    let report = parse_exact_json(&result.stdout);
    assert_attestation_report_v1(&report, family, "violated", false);
    let attestation = attestation(&report);
    assert_eq!(
        unsigned(
            field(attestation, "expected_scalar_count", "attestation"),
            "expected_scalar_count"
        ),
        8
    );
    assert_eq!(
        unsigned(
            field(attestation, "candidate_scalar_count", "attestation"),
            "candidate_scalar_count"
        ),
        0
    );
    assert!(!array(
        field(attestation, "diagnostics", "attestation"),
        "diagnostics"
    )
    .is_empty());
    assert!(array(
        field(attestation, "used_resources", "attestation"),
        "used_resources"
    )
    .is_empty());
}

#[test]
fn unmapped_glyphs_are_unknown_after_one_compilation_and_never_published() {
    let directory = TestDirectory::new("unknown");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let observation = fixture("evaluation", "observation-known-baseline.json");
    let raster = fixture("reconstruction", "page.pgm");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let fixed_pdf = infra_fixture("textops.pdf");

    let result = run_with_oracle(
        &attestation_args(&observation, &raster, &output_pdf, "Helvetica", &compiler),
        &state,
        "fixed-pdf",
        Some(&fixed_pdf),
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 1);
    assert!(!output_pdf.exists());

    let report = parse_exact_json(&result.stdout);
    assert_attestation_report_v1(&report, "Helvetica", "unknown", false);
    let attestation = attestation(&report);
    assert!(field(attestation, "candidate_scalar_count", "attestation").is_null());
    assert!(!array(
        field(attestation, "diagnostics", "attestation"),
        "diagnostics"
    )
    .is_empty());
    let resources = array(
        field(attestation, "used_resources", "attestation"),
        "used_resources",
    );
    assert_eq!(resources.len(), 1);
    let resource = object(&resources[0], "Helvetica resource");
    assert_eq!(
        string(field(resource, "base_font", "resource"), "base_font"),
        "Helvetica"
    );
    assert_eq!(
        string(
            field(resource, "normalized_stem", "resource"),
            "normalized_stem"
        ),
        "helvetica"
    );
    assert_eq!(
        unsigned(field(resource, "glyph_count", "resource"), "glyph_count"),
        10
    );
}

#[test]
fn existing_destination_is_never_overwritten() {
    let directory = TestDirectory::new("no-clobber");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("already-exists.pdf");
    let sentinel = b"pre-existing attestation artifact must survive";
    fs::write(&output_pdf, sentinel).unwrap();
    let args = attestation_args(
        &fixture("evaluation", "observation-known-baseline.json"),
        &fixture("reconstruction", "page.pgm"),
        &output_pdf,
        "Libertinus Serif",
        &compiler,
    );

    let result = run_with_oracle(&args, &state, "trace", None);
    let error = stderr(&result);
    assert_eq!(result.status.code(), Some(2), "{error}");
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&output_pdf).unwrap().as_slice(), sentinel);
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 1);
    assert_no_temporary_publication(directory.path());
}
