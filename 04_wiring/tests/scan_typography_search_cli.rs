//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-typst-parameter-search.md
//! @layer L4
//! @updated 2026-09-19
//!
//! Black-box behavior tests for the complete discrete typography search.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};

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
            "decalque-typography-search-{}-{serial}-{label}",
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

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/typography_search")
        .join(name)
}

fn observation_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures/scan_observation")
        .join(name)
}

fn install_oracle(directory: &Path) -> PathBuf {
    let executable = directory.join("typst-oracle");
    fs::copy(fixture("fake-typst"), &executable).expect("oracle compiler must be copied");

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

fn run_with_oracle(args: &[OsString], state: &Path, mode: &str, fail_at: Option<usize>) -> Output {
    fs::create_dir(state).expect("oracle state directory must be created");
    let mut command = Command::new(binary());
    command
        .args(args)
        .env("DECALQUE_TYPOGRAPHY_ORACLE_DIR", state)
        .env("DECALQUE_TYPOGRAPHY_ORACLE_MODE", mode);
    if let Some(fail_at) = fail_at {
        command.env("DECALQUE_TYPOGRAPHY_ORACLE_FAIL_AT", fail_at.to_string());
    }
    command.output().expect("the decalque binary must start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr must be UTF-8")
}

fn search_args(
    observation: &Path,
    raster: &Path,
    output_pdf: &Path,
    sizes: &[&str],
    trackings: &[&str],
    compiler: Option<&Path>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("fit-scan-lines"),
        observation.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--output-pdf"),
        output_pdf.as_os_str().to_owned(),
        OsString::from("--font-family"),
        OsString::from("Libertinus Serif"),
    ];
    for size in sizes {
        args.extend([OsString::from("--font-size-pt"), OsString::from(*size)]);
    }
    args.extend([
        OsString::from("--font-weight"),
        OsString::from("regular"),
        OsString::from("--font-style"),
        OsString::from("normal"),
    ]);
    for tracking in trackings {
        args.extend([OsString::from("--tracking-pt"), OsString::from(*tracking)]);
    }
    args.extend([
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("2"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("2"),
    ]);
    if let Some(compiler) = compiler {
        args.extend([
            OsString::from("--typst-bin"),
            compiler.as_os_str().to_owned(),
        ]);
    }
    args
}

fn reconstruction_args(observation: &Path, raster: &Path) -> Vec<OsString> {
    vec![
        OsString::from("reconstruct-scan-lines"),
        observation.as_os_str().to_owned(),
        OsString::from("--raster"),
        raster.as_os_str().to_owned(),
        OsString::from("--font-family"),
        OsString::from("Libertinus Serif"),
        OsString::from("--font-size-pt"),
        OsString::from("10"),
        OsString::from("--font-weight"),
        OsString::from("regular"),
        OsString::from("--font-style"),
        OsString::from("normal"),
        OsString::from("--tracking-pt"),
        OsString::from("0"),
    ]
}

fn evaluation_args(observation: &Path, raster: &Path, output_pdf: &Path) -> Vec<OsString> {
    let mut args = reconstruction_args(observation, raster);
    args[0] = OsString::from("evaluate-scan-lines");
    args.extend([
        OsString::from("--output-pdf"),
        output_pdf.as_os_str().to_owned(),
        OsString::from("--granularity"),
        OsString::from("line"),
        OsString::from("--horizontal-tolerance-pt"),
        OsString::from("2"),
        OsString::from("--baseline-tolerance-pt"),
        OsString::from("2"),
    ]);
    args
}

fn evaluation_args_for_hypothesis(
    observation: &Path,
    raster: &Path,
    output_pdf: &Path,
    size: &str,
    tracking: &str,
) -> Vec<OsString> {
    let mut args = evaluation_args(observation, raster, output_pdf);
    let size_position = args
        .iter()
        .position(|argument| argument == "--font-size-pt")
        .expect("evaluation size option must exist");
    args[size_position + 1] = OsString::from(size);
    let tracking_position = args
        .iter()
        .position(|argument| argument == "--tracking-pt")
        .expect("evaluation tracking option must exist");
    args[tracking_position + 1] = OsString::from(tracking);
    args
}

fn compact_json(bytes: &[u8]) -> String {
    let json = String::from_utf8(bytes.to_vec()).expect("stdout report must be UTF-8");
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

fn parse_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("stdout must contain exactly one complete JSON value")
}

fn parse_exact_fit_json(bytes: &[u8]) -> Value {
    assert_eq!(
        bytes.first().copied(),
        Some(b'{'),
        "stdout must start exactly with the root JSON object opener"
    );
    assert_eq!(
        bytes.last().copied(),
        Some(b'}'),
        "stdout must end exactly with the root JSON object closer"
    );
    parse_json(bytes)
}

fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object, got {value}"))
}

fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
    value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("{context} must be an array, got {value}"))
}

fn field<'a>(object: &'a Map<String, Value>, key: &str, context: &str) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| panic!("{context}.{key} must exist"))
}

fn exact_fields(object: &Map<String, Value>, expected: &[&str], context: &str) {
    assert_eq!(
        object.len(),
        expected.len(),
        "{context} has non-v1 fields: {:?}",
        object.keys().collect::<Vec<_>>()
    );
    for key in expected {
        assert!(object.contains_key(*key), "{context}.{key} must exist");
    }
}

fn string<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a string, got {value}"))
}

fn unsigned(value: &Value, context: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{context} must be an unsigned integer, got {value}"))
}

fn finite_number(value: &Value, context: &str) -> f64 {
    let number = value
        .as_f64()
        .unwrap_or_else(|| panic!("{context} must be a number, got {value}"));
    assert!(number.is_finite(), "{context} must be finite");
    number
}

fn boolean(value: &Value, context: &str) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| panic!("{context} must be a boolean, got {value}"))
}

fn assert_enum(value: &Value, allowed: &[&str], context: &str) {
    let actual = string(value, context);
    assert!(allowed.contains(&actual), "invalid {context}: {actual}");
}

fn assert_sha256(value: &Value, context: &str) {
    let hash = string(value, context);
    assert_eq!(hash.len(), 64, "{context} must have 64 hexadecimal bytes");
    assert!(
        hash.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{context} must be lowercase hexadecimal: {hash}"
    );
}

fn assert_source_v1(value: &Value, context: &str) {
    let source = object(value, context);
    exact_fields(source, &["page_index", "raster_sha256"], context);
    unsigned(
        field(source, "page_index", context),
        &format!("{context}.page_index"),
    );
    assert_sha256(
        field(source, "raster_sha256", context),
        &format!("{context}.raster_sha256"),
    );
}

fn assert_hypothesis_v1(value: &Value, context: &str) {
    let hypothesis = object(value, context);
    exact_fields(
        hypothesis,
        &[
            "font_family",
            "font_size_pt",
            "font_weight",
            "font_style",
            "tracking_pt",
        ],
        context,
    );
    assert!(!string(field(hypothesis, "font_family", context), context).is_empty());
    finite_number(field(hypothesis, "font_size_pt", context), context);
    assert_enum(
        field(hypothesis, "font_weight", context),
        &["regular", "bold"],
        context,
    );
    assert_enum(
        field(hypothesis, "font_style", context),
        &["normal", "italic", "oblique"],
        context,
    );
    finite_number(field(hypothesis, "tracking_pt", context), context);
}

fn assert_coverage_v1(value: &Value, context: &str) {
    let coverage = object(value, context);
    exact_fields(
        coverage,
        &[
            "matched_scan",
            "total_scan",
            "matched_candidate",
            "total_candidate",
        ],
        context,
    );
    for key in [
        "matched_scan",
        "total_scan",
        "matched_candidate",
        "total_candidate",
    ] {
        unsigned(field(coverage, key, context), &format!("{context}.{key}"));
    }
}

fn assert_string_array(value: &Value, context: &str) {
    for (index, item) in array(value, context).iter().enumerate() {
        assert!(
            !string(item, &format!("{context}[{index}]")).is_empty(),
            "{context}[{index}] must be non-empty"
        );
    }
}

fn assert_descending_u64_array(value: &Value, context: &str) {
    let values: Vec<_> = array(value, context)
        .iter()
        .enumerate()
        .map(|(index, item)| unsigned(item, &format!("{context}[{index}]")))
        .collect();
    assert!(
        values.windows(2).all(|pair| pair[0] >= pair[1]),
        "{context} must be descending: {values:?}"
    );
}

fn assert_comparison_v1(value: &Value, context: &str) {
    let comparison = object(value, context);
    exact_fields(
        comparison,
        &[
            "schema",
            "schema_version",
            "source",
            "policy",
            "content_status",
            "geometry_status",
            "overall_status",
            "coverage",
            "matches",
            "unmatched_scan",
            "unmatched_candidate",
            "reflow",
            "observation_diagnostics",
            "pdf_diagnostics",
            "comparison_diagnostics",
        ],
        context,
    );
    assert_eq!(
        string(field(comparison, "schema", context), context),
        "decalque.scan-comparison-report"
    );
    assert_eq!(
        unsigned(field(comparison, "schema_version", context), context),
        1
    );
    assert_source_v1(
        field(comparison, "source", context),
        "winner.comparison.source",
    );

    let policy = object(
        field(comparison, "policy", context),
        "winner.comparison.policy",
    );
    exact_fields(
        policy,
        &[
            "granularity",
            "text_normalization",
            "horizontal_tolerance_pt",
            "baseline_tolerance_pt",
            "min_text_confidence",
            "min_geometry_confidence",
        ],
        "winner.comparison.policy",
    );
    assert_eq!(
        string(field(policy, "granularity", context), context),
        "line"
    );
    assert_eq!(
        string(field(policy, "text_normalization", context), context),
        "exact"
    );
    finite_number(field(policy, "horizontal_tolerance_pt", context), context);
    finite_number(field(policy, "baseline_tolerance_pt", context), context);
    for key in ["min_text_confidence", "min_geometry_confidence"] {
        let requirement = object(field(policy, key, context), key);
        exact_fields(requirement, &["status"], key);
        assert_eq!(
            string(field(requirement, "status", key), key),
            "not-required"
        );
    }

    for key in ["content_status", "geometry_status", "overall_status"] {
        assert_enum(
            field(comparison, key, context),
            &["preserved", "violated", "unknown"],
            &format!("{context}.{key}"),
        );
    }
    assert_coverage_v1(
        field(comparison, "coverage", context),
        "winner.comparison.coverage",
    );

    let matches = array(
        field(comparison, "matches", context),
        "winner.comparison.matches",
    );
    assert!(!matches.is_empty(), "selected winner must contain matches");
    for (index, item) in matches.iter().enumerate() {
        let match_context = format!("winner.comparison.matches[{index}]");
        let match_object = object(item, &match_context);
        let baseline_status = string(
            field(match_object, "baseline_status", &match_context),
            &match_context,
        );
        let expected = if baseline_status == "unknown" {
            vec![
                "scan_unit_id",
                "candidate_line_index",
                "candidate_scalar_range",
                "dx_start",
                "dx_end",
                "width_delta",
                "horizontal_status",
                "baseline_status",
            ]
        } else {
            vec![
                "scan_unit_id",
                "candidate_line_index",
                "candidate_scalar_range",
                "dx_start",
                "dx_end",
                "width_delta",
                "baseline_delta",
                "horizontal_status",
                "baseline_status",
            ]
        };
        exact_fields(match_object, &expected, &match_context);
        assert!(!string(
            field(match_object, "scan_unit_id", &match_context),
            &match_context
        )
        .is_empty());
        unsigned(
            field(match_object, "candidate_line_index", &match_context),
            &match_context,
        );
        let scalar_range = array(
            field(match_object, "candidate_scalar_range", &match_context),
            &match_context,
        );
        assert_eq!(
            scalar_range.len(),
            2,
            "{match_context} range must have two bounds"
        );
        unsigned(&scalar_range[0], &match_context);
        unsigned(&scalar_range[1], &match_context);
        for key in ["dx_start", "dx_end", "width_delta"] {
            finite_number(field(match_object, key, &match_context), &match_context);
        }
        if let Some(delta) = match_object.get("baseline_delta") {
            finite_number(delta, &match_context);
        }
        assert_enum(
            field(match_object, "horizontal_status", &match_context),
            &["preserved", "violated"],
            &match_context,
        );
        assert_enum(
            field(match_object, "baseline_status", &match_context),
            &["preserved", "violated", "unknown"],
            &match_context,
        );
    }

    assert!(array(field(comparison, "unmatched_scan", context), context).is_empty());
    assert!(array(field(comparison, "unmatched_candidate", context), context).is_empty());
    assert!(array(field(comparison, "reflow", context), context).is_empty());
    assert!(array(
        field(comparison, "observation_diagnostics", context),
        context
    )
    .is_empty());
    assert!(array(field(comparison, "pdf_diagnostics", context), context).is_empty());
    let diagnostics = array(
        field(comparison, "comparison_diagnostics", context),
        "winner.comparison.comparison_diagnostics",
    );
    assert_eq!(
        diagnostics.len(),
        1,
        "the reference fixture must preserve its baseline-unknown diagnostic"
    );
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        let diagnostic_context = format!("winner.comparison.comparison_diagnostics[{index}]");
        let diagnostic = object(diagnostic, &diagnostic_context);
        exact_fields(
            diagnostic,
            &["code", "scan_unit_id", "component", "reason"],
            &diagnostic_context,
        );
        for key in ["code", "scan_unit_id", "component", "reason"] {
            assert!(!string(
                field(diagnostic, key, &diagnostic_context),
                &diagnostic_context
            )
            .is_empty());
        }
        assert_eq!(
            field(diagnostic, "code", &diagnostic_context),
            "geometry-claim-unknown"
        );
        assert_eq!(
            field(diagnostic, "scan_unit_id", &diagnostic_context),
            "line-1"
        );
        assert_eq!(
            field(diagnostic, "component", &diagnostic_context),
            "baseline"
        );
        assert_eq!(
            field(diagnostic, "reason", &diagnostic_context),
            "not-observed"
        );
    }
}

fn assert_evaluation_v1(value: &Value, context: &str) {
    let winner = object(value, context);
    exact_fields(
        winner,
        &[
            "schema",
            "schema_version",
            "source",
            "hypothesis",
            "compiler_version",
            "source_sha256",
            "source_size_bytes",
            "pdf_sha256",
            "pdf_size_bytes",
            "comparison",
        ],
        context,
    );
    assert_eq!(
        string(field(winner, "schema", context), context),
        "decalque.scan-reconstruction-evaluation"
    );
    assert_eq!(
        unsigned(field(winner, "schema_version", context), context),
        1
    );
    assert_source_v1(field(winner, "source", context), "winner.source");
    assert_hypothesis_v1(field(winner, "hypothesis", context), "winner.hypothesis");
    assert!(!string(field(winner, "compiler_version", context), context).is_empty());
    assert_sha256(
        field(winner, "source_sha256", context),
        "winner.source_sha256",
    );
    assert!(unsigned(field(winner, "source_size_bytes", context), context) > 0);
    assert_sha256(field(winner, "pdf_sha256", context), "winner.pdf_sha256");
    assert!(unsigned(field(winner, "pdf_size_bytes", context), context) > 0);
    assert_comparison_v1(field(winner, "comparison", context), "winner.comparison");
    assert_eq!(
        field(
            object(field(winner, "comparison", context), "winner.comparison"),
            "source",
            "winner.comparison",
        ),
        field(winner, "source", context),
        "winner comparison must remain bound to the same source"
    );
}

fn assert_search_report_v1(value: &Value) {
    let report = object(value, "report");
    exact_fields(
        report,
        &[
            "schema",
            "schema_version",
            "source",
            "search_space",
            "compiler_version",
            "trials",
            "selection",
            "winner",
        ],
        "report",
    );
    assert_eq!(
        string(field(report, "schema", "report"), "report.schema"),
        "decalque.scan-typography-search"
    );
    assert_eq!(
        unsigned(
            field(report, "schema_version", "report"),
            "report.schema_version"
        ),
        1
    );
    assert_source_v1(field(report, "source", "report"), "report.source");

    let search_space = object(
        field(report, "search_space", "report"),
        "report.search_space",
    );
    exact_fields(
        search_space,
        &[
            "font_family",
            "font_weight",
            "font_style",
            "font_sizes_pt",
            "trackings_pt",
            "hypothesis_count",
        ],
        "report.search_space",
    );
    assert!(!string(
        field(search_space, "font_family", "report.search_space"),
        "font_family"
    )
    .is_empty());
    assert_enum(
        field(search_space, "font_weight", "report.search_space"),
        &["regular", "bold"],
        "font_weight",
    );
    assert_enum(
        field(search_space, "font_style", "report.search_space"),
        &["normal", "italic", "oblique"],
        "font_style",
    );
    let sizes = array(
        field(search_space, "font_sizes_pt", "report.search_space"),
        "font_sizes_pt",
    );
    let trackings = array(
        field(search_space, "trackings_pt", "report.search_space"),
        "trackings_pt",
    );
    assert!(!sizes.is_empty());
    assert!(!trackings.is_empty());
    for (index, value) in sizes.iter().enumerate() {
        finite_number(value, &format!("font_sizes_pt[{index}]"));
    }
    for (index, value) in trackings.iter().enumerate() {
        finite_number(value, &format!("trackings_pt[{index}]"));
    }
    let hypothesis_count = unsigned(
        field(search_space, "hypothesis_count", "report.search_space"),
        "hypothesis_count",
    );
    assert_eq!(hypothesis_count as usize, sizes.len() * trackings.len());
    assert!(!string(
        field(report, "compiler_version", "report"),
        "compiler_version"
    )
    .is_empty());

    let trials = array(field(report, "trials", "report"), "report.trials");
    assert_eq!(trials.len(), hypothesis_count as usize);
    for (index, trial) in trials.iter().enumerate() {
        let trial_context = format!("report.trials[{index}]");
        let trial = object(trial, &trial_context);
        exact_fields(
            trial,
            &[
                "index",
                "hypothesis",
                "source_sha256",
                "source_size_bytes",
                "pdf_sha256",
                "pdf_size_bytes",
                "content_status",
                "geometry_status",
                "overall_status",
                "coverage",
                "eligibility",
                "support",
                "score",
            ],
            &trial_context,
        );
        assert_eq!(
            unsigned(field(trial, "index", &trial_context), &trial_context),
            index as u64
        );
        assert_hypothesis_v1(
            field(trial, "hypothesis", &trial_context),
            &format!("{trial_context}.hypothesis"),
        );
        assert_sha256(
            field(trial, "source_sha256", &trial_context),
            &format!("{trial_context}.source_sha256"),
        );
        assert!(
            unsigned(
                field(trial, "source_size_bytes", &trial_context),
                &trial_context
            ) > 0
        );
        assert_sha256(
            field(trial, "pdf_sha256", &trial_context),
            &format!("{trial_context}.pdf_sha256"),
        );
        assert!(
            unsigned(
                field(trial, "pdf_size_bytes", &trial_context),
                &trial_context
            ) > 0
        );
        for key in ["content_status", "geometry_status", "overall_status"] {
            assert_enum(
                field(trial, key, &trial_context),
                &["preserved", "violated", "unknown"],
                &format!("{trial_context}.{key}"),
            );
        }
        assert_coverage_v1(
            field(trial, "coverage", &trial_context),
            &format!("{trial_context}.coverage"),
        );

        let eligibility = object(
            field(trial, "eligibility", &trial_context),
            &format!("{trial_context}.eligibility"),
        );
        match string(field(eligibility, "status", &trial_context), &trial_context) {
            "eligible" => {
                exact_fields(
                    eligibility,
                    &["status"],
                    &format!("{trial_context}.eligibility"),
                );
                let support = object(
                    field(trial, "support", &trial_context),
                    &format!("{trial_context}.support"),
                );
                exact_fields(
                    support,
                    &["horizontal_scan_unit_ids", "known_baseline_scan_unit_ids"],
                    &format!("{trial_context}.support"),
                );
                let horizontal = field(support, "horizontal_scan_unit_ids", &trial_context);
                let baseline = field(support, "known_baseline_scan_unit_ids", &trial_context);
                assert_string_array(
                    horizontal,
                    &format!("{trial_context}.support.horizontal_scan_unit_ids"),
                );
                assert_string_array(
                    baseline,
                    &format!("{trial_context}.support.known_baseline_scan_unit_ids"),
                );
                let score = object(
                    field(trial, "score", &trial_context),
                    &format!("{trial_context}.score"),
                );
                exact_fields(
                    score,
                    &[
                        "horizontal_violations",
                        "horizontal_residuals_desc",
                        "baseline_violations",
                        "baseline_residuals_desc",
                    ],
                    &format!("{trial_context}.score"),
                );
                unsigned(
                    field(score, "horizontal_violations", &trial_context),
                    &trial_context,
                );
                unsigned(
                    field(score, "baseline_violations", &trial_context),
                    &trial_context,
                );
                let horizontal_residuals =
                    field(score, "horizontal_residuals_desc", &trial_context);
                let baseline_residuals = field(score, "baseline_residuals_desc", &trial_context);
                assert_descending_u64_array(
                    horizontal_residuals,
                    &format!("{trial_context}.score.horizontal_residuals_desc"),
                );
                assert_descending_u64_array(
                    baseline_residuals,
                    &format!("{trial_context}.score.baseline_residuals_desc"),
                );
                assert_eq!(
                    array(horizontal, &trial_context).len(),
                    array(horizontal_residuals, &trial_context).len()
                );
                assert_eq!(
                    array(baseline, &trial_context).len(),
                    array(baseline_residuals, &trial_context).len()
                );
            }
            "ineligible" => {
                exact_fields(
                    eligibility,
                    &["status", "reason"],
                    &format!("{trial_context}.eligibility"),
                );
                assert_enum(
                    field(eligibility, "reason", &trial_context),
                    &[
                        "empty-scan-scope",
                        "content-not-preserved",
                        "partial-coverage",
                        "unmatched-units",
                        "missing-horizontal-evidence",
                    ],
                    &format!("{trial_context}.eligibility.reason"),
                );
                assert!(field(trial, "support", &trial_context).is_null());
                assert!(field(trial, "score", &trial_context).is_null());
            }
            status => panic!("invalid {trial_context}.eligibility.status: {status}"),
        }
    }

    let selection = object(field(report, "selection", "report"), "report.selection");
    match string(
        field(selection, "status", "report.selection"),
        "report.selection.status",
    ) {
        "selected" => {
            exact_fields(
                selection,
                &[
                    "status",
                    "selected_index",
                    "evidence_scope",
                    "artifact_published",
                ],
                "report.selection",
            );
            let selected_index = unsigned(
                field(selection, "selected_index", "report.selection"),
                "report.selection.selected_index",
            ) as usize;
            assert!(selected_index < trials.len());
            assert_enum(
                field(selection, "evidence_scope", "report.selection"),
                &["horizontal-only", "horizontal-and-observed-baseline"],
                "report.selection.evidence_scope",
            );
            assert!(boolean(
                field(selection, "artifact_published", "report.selection"),
                "report.selection.artifact_published"
            ));
            let winner = field(report, "winner", "report");
            assert_evaluation_v1(winner, "winner");
            let trial = object(&trials[selected_index], "selected trial");
            let winner = object(winner, "winner");
            assert_eq!(
                field(winner, "source", "winner"),
                field(report, "source", "report")
            );
            assert_eq!(
                field(winner, "hypothesis", "winner"),
                field(trial, "hypothesis", "selected trial")
            );
            assert_eq!(
                field(winner, "compiler_version", "winner"),
                field(report, "compiler_version", "report")
            );
            for key in [
                "source_sha256",
                "source_size_bytes",
                "pdf_sha256",
                "pdf_size_bytes",
            ] {
                assert_eq!(
                    field(winner, key, "winner"),
                    field(trial, key, "selected trial")
                );
            }
            let comparison = object(field(winner, "comparison", "winner"), "winner.comparison");
            for key in [
                "content_status",
                "geometry_status",
                "overall_status",
                "coverage",
            ] {
                assert_eq!(
                    field(comparison, key, "winner.comparison"),
                    field(trial, key, "selected trial")
                );
            }
        }
        "tied" => {
            exact_fields(
                selection,
                &["status", "indices", "evidence_scope", "artifact_published"],
                "report.selection",
            );
            let indices = array(
                field(selection, "indices", "report.selection"),
                "report.selection.indices",
            );
            assert!(indices.len() >= 2);
            for (position, index) in indices.iter().enumerate() {
                assert!((unsigned(index, &format!("indices[{position}]")) as usize) < trials.len());
            }
            assert_enum(
                field(selection, "evidence_scope", "report.selection"),
                &["horizontal-only", "horizontal-and-observed-baseline"],
                "report.selection.evidence_scope",
            );
            assert!(!boolean(
                field(selection, "artifact_published", "report.selection"),
                "report.selection.artifact_published"
            ));
            assert!(field(report, "winner", "report").is_null());
        }
        "inconclusive" => {
            exact_fields(
                selection,
                &["status", "reason", "evidence_scope", "artifact_published"],
                "report.selection",
            );
            assert_enum(
                field(selection, "reason", "report.selection"),
                &["no-eligible-trial", "incomparable-support"],
                "report.selection.reason",
            );
            assert!(field(selection, "evidence_scope", "report.selection").is_null());
            assert!(!boolean(
                field(selection, "artifact_published", "report.selection"),
                "report.selection.artifact_published"
            ));
            assert!(field(report, "winner", "report").is_null());
        }
        status => panic!("invalid report.selection.status: {status}"),
    }
}

fn counter(state: &Path, name: &str) -> usize {
    fs::read_to_string(state.join(name))
        .unwrap_or_else(|_| String::from("0"))
        .trim()
        .parse()
        .unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum must start");
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn assert_hypothesis(trial: &Value, index: usize, size: &str, tracking: &str) {
    let trial = object(trial, "trial");
    assert_eq!(
        unsigned(field(trial, "index", "trial"), "trial.index"),
        index as u64
    );
    let hypothesis = object(field(trial, "hypothesis", "trial"), "trial.hypothesis");
    assert_eq!(
        string(
            field(hypothesis, "font_family", "trial.hypothesis"),
            "font_family"
        ),
        "Libertinus Serif"
    );
    assert_eq!(
        finite_number(
            field(hypothesis, "font_size_pt", "trial.hypothesis"),
            "font_size_pt"
        ),
        size.parse::<f64>().unwrap()
    );
    assert_eq!(
        string(
            field(hypothesis, "font_weight", "trial.hypothesis"),
            "font_weight"
        ),
        "regular"
    );
    assert_eq!(
        string(
            field(hypothesis, "font_style", "trial.hypothesis"),
            "font_style"
        ),
        "normal"
    );
    assert_eq!(
        finite_number(
            field(hypothesis, "tracking_pt", "trial.hypothesis"),
            "tracking_pt"
        ),
        tracking.parse::<f64>().unwrap()
    );
}

fn typst_version() -> String {
    let output = Command::new("typst")
        .arg("--version")
        .output()
        .expect("delegated Typst must start");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

fn expected_source(size: &str, tracking: &str) -> Vec<u8> {
    const BASE: &str = r##"#set page(width: 144pt, height: 144pt, margin: 0pt)
#set text(font: "Libertinus Serif", size: 10pt, weight: "regular", style: "normal", tracking: 0pt, hyphenate: false, top-edge: "bounds", bottom-edge: "bounds")

#let physical-line(body) = context {
  let natural-size = measure(body)
  box(width: natural-size.width, body)
}

#place(top + left, dx: 18pt, dy: 36pt)[
  #physical-line(text("  A \\\"#[] Ω\\nB  "))
]
"##;
    BASE.replace("size: 10pt", &format!("size: {size}pt"))
        .replace("tracking: 0pt", &format!("tracking: {tracking}pt"))
        .into_bytes()
}

#[test]
fn real_grid_selects_confirms_and_publishes_only_18_negative_tracking_deterministically() {
    let directory = TestDirectory::new("real-winner");
    let observation = fixture("observation.json");
    let raster = fixture("page.pgm");
    let first_pdf = directory.path().join("first.pdf");
    let second_pdf = directory.path().join("second.pdf");
    let first_args = search_args(
        &observation,
        &raster,
        &first_pdf,
        &["10", "18", "30"],
        &["-0.15", "0"],
        None,
    );
    let second_args = search_args(
        &observation,
        &raster,
        &second_pdf,
        &["10", "18", "30"],
        &["-0.15", "0"],
        None,
    );

    let first = run(&first_args);
    let second = run(&second_args);
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(second.status.success(), "{}", stderr(&second));
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(
        first.stdout, second.stdout,
        "reports must be byte-identical"
    );

    let first_bytes = fs::read(&first_pdf).expect("winner PDF must be published");
    let second_bytes = fs::read(&second_pdf).expect("second winner PDF must be published");
    assert!(first_bytes.starts_with(b"%PDF"));
    assert_eq!(
        first_bytes, second_bytes,
        "winner PDFs must be byte-identical"
    );

    let compact_report = compact_json(&first.stdout);
    assert!(compact_report
        .starts_with("{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1,"));
    let report = parse_exact_fit_json(&first.stdout);
    assert_search_report_v1(&report);
    let report_object = object(&report, "report");
    let space = object(
        field(report_object, "search_space", "report"),
        "report.search_space",
    );
    assert_eq!(
        field(space, "font_sizes_pt", "report.search_space"),
        &serde_json::json!([10, 18, 30])
    );
    assert_eq!(
        field(space, "trackings_pt", "report.search_space"),
        &serde_json::json!([-0.15, 0])
    );

    let trials = array(field(report_object, "trials", "report"), "report.trials");
    assert_eq!(trials.len(), 6);
    for (index, (size, tracking)) in [
        ("10", "-0.15"),
        ("10", "0"),
        ("18", "-0.15"),
        ("18", "0"),
        ("30", "-0.15"),
        ("30", "0"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_hypothesis(&trials[index], index, size, tracking);
    }

    let selection = object(
        field(report_object, "selection", "report"),
        "report.selection",
    );
    assert_eq!(
        string(field(selection, "status", "selection"), "status"),
        "selected"
    );
    assert_eq!(
        unsigned(
            field(selection, "selected_index", "selection"),
            "selected_index"
        ),
        2
    );
    assert_eq!(
        string(
            field(selection, "evidence_scope", "selection"),
            "evidence_scope"
        ),
        "horizontal-only"
    );
    assert!(boolean(
        field(selection, "artifact_published", "selection"),
        "artifact_published"
    ));

    let winner = field(report_object, "winner", "report");
    let winner_object = object(winner, "winner");
    let published_hash = sha256(&first_bytes);
    assert_eq!(
        string(field(winner_object, "pdf_sha256", "winner"), "pdf_sha256"),
        published_hash
    );
    assert_eq!(
        field(object(&trials[2], "trial 2"), "pdf_sha256", "trial 2"),
        &Value::String(published_hash)
    );

    let standalone_directory = TestDirectory::new("standalone-winner");
    let standalone_pdf = standalone_directory.path().join("winner.pdf");
    let standalone = run(&evaluation_args_for_hypothesis(
        &observation,
        &raster,
        &standalone_pdf,
        "18",
        "-0.15",
    ));
    assert!(standalone.status.success(), "{}", stderr(&standalone));
    assert!(standalone.stderr.is_empty());
    let standalone_report = parse_json(&standalone.stdout);
    assert_evaluation_v1(&standalone_report, "standalone evaluation");
    assert_eq!(
        winner, &standalone_report,
        "winner must be the complete confirmed evaluation report"
    );
    assert_eq!(
        first_bytes,
        fs::read(standalone_pdf).expect("standalone winner PDF must exist")
    );

    let published: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(
        published.len(),
        2,
        "only the two requested winners may exist"
    );
}

#[test]
fn canonical_grid_uses_one_version_and_n_plus_one_compilations_only_for_selected() {
    let directory = TestDirectory::new("counts-and-order");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("winner.pdf");
    let args = search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &output_pdf,
        &["30", "10", "18"],
        &["0", "-0.15"],
        Some(&compiler),
    );

    let result = run_with_oracle(&args, &state, "trace", None);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 7);
    assert!(output_pdf.exists());

    for (number, (size, tracking)) in [
        ("10", "-0.15"),
        ("10", "0"),
        ("18", "-0.15"),
        ("18", "0"),
        ("30", "-0.15"),
        ("30", "0"),
    ]
    .into_iter()
    .enumerate()
    {
        let source = fs::read(state.join(format!("source-{:02}.typ", number + 1))).unwrap();
        assert_eq!(source, expected_source(size, tracking));
    }
    assert_eq!(
        fs::read(state.join("source-07.typ")).unwrap(),
        fs::read(state.join("source-03.typ")).unwrap(),
        "confirmation must repeat the selected source exactly"
    );

    let report = parse_exact_fit_json(&result.stdout);
    assert_search_report_v1(&report);
    let report = object(&report, "report");
    assert_eq!(
        string(
            field(report, "compiler_version", "report"),
            "compiler_version"
        ),
        typst_version()
    );
    let space = object(field(report, "search_space", "report"), "search_space");
    assert_eq!(
        field(space, "font_sizes_pt", "search_space"),
        &serde_json::json!([10, 18, 30])
    );
    assert_eq!(
        field(space, "trackings_pt", "search_space"),
        &serde_json::json!([-0.15, 0])
    );
}

#[test]
fn identical_pdf_for_distinct_hypotheses_is_tied_without_confirmation_or_publication() {
    let directory = TestDirectory::new("tied");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let args = search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &output_pdf,
        &["18"],
        &["0", "-0.15"],
        Some(&compiler),
    );

    let result = run_with_oracle(&args, &state, "identical", None);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert!(!output_pdf.exists());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(
        counter(&state, "compile.count"),
        2,
        "tied has no confirmation"
    );

    let report = parse_exact_fit_json(&result.stdout);
    assert_search_report_v1(&report);
    let report = object(&report, "report");
    let trials = array(field(report, "trials", "report"), "trials");
    assert_eq!(trials.len(), 2);
    assert_eq!(
        field(object(&trials[0], "trial 0"), "pdf_sha256", "trial 0"),
        field(object(&trials[1], "trial 1"), "pdf_sha256", "trial 1")
    );
    let selection = object(field(report, "selection", "report"), "selection");
    assert_eq!(
        string(field(selection, "status", "selection"), "status"),
        "tied"
    );
    assert_eq!(
        field(selection, "indices", "selection"),
        &serde_json::json!([0, 1])
    );
    assert!(!boolean(
        field(selection, "artifact_published", "selection"),
        "artifact_published"
    ));
    assert!(field(report, "winner", "report").is_null());
}

#[test]
fn valid_but_ineligible_candidate_is_inconclusive_without_confirmation_or_publication() {
    let directory = TestDirectory::new("no-eligible-trial");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let destination = directory.path().join("already-exists.pdf");
    let sentinel = b"pre-existing bytes must survive inconclusive search";
    fs::write(&destination, sentinel).unwrap();
    let args = search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &destination,
        &["18"],
        &["-0.15"],
        Some(&compiler),
    );

    let result = run_with_oracle(&args, &state, "ineligible", None);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(fs::read(&destination).unwrap().as_slice(), sentinel);
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(
        counter(&state, "compile.count"),
        1,
        "inconclusive search must not confirm an ineligible trial"
    );
    assert!(!state.join("source-02.typ").exists());

    let report = parse_exact_fit_json(&result.stdout);
    assert_search_report_v1(&report);
    let report = object(&report, "report");
    let trials = array(field(report, "trials", "report"), "trials");
    assert_eq!(trials.len(), 1);
    let trial = object(&trials[0], "trial 0");
    let eligibility = object(field(trial, "eligibility", "trial 0"), "eligibility");
    assert_eq!(
        string(field(eligibility, "status", "eligibility"), "status"),
        "ineligible"
    );
    assert!(field(trial, "support", "trial 0").is_null());
    assert!(field(trial, "score", "trial 0").is_null());

    let selection = object(field(report, "selection", "report"), "selection");
    assert_eq!(
        string(field(selection, "status", "selection"), "status"),
        "inconclusive"
    );
    assert_eq!(
        string(field(selection, "reason", "selection"), "reason"),
        "no-eligible-trial"
    );
    assert!(field(selection, "evidence_scope", "selection").is_null());
    assert!(!boolean(
        field(selection, "artifact_published", "selection"),
        "artifact_published"
    ));
    assert!(field(report, "winner", "report").is_null());
}

#[test]
fn failure_in_the_last_trial_discards_every_partial_result() {
    let directory = TestDirectory::new("last-failure");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let args = search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &output_pdf,
        &["10", "18", "30"],
        &["-0.15", "0"],
        Some(&compiler),
    );

    let result = run_with_oracle(&args, &state, "fail-last", Some(6));
    assert_eq!(result.status.code(), Some(2), "{}", stderr(&result));
    assert!(result.stdout.is_empty());
    assert!(!output_pdf.exists());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 6);
}

#[test]
fn divergent_confirmation_is_an_execution_error_and_publishes_nothing() {
    let directory = TestDirectory::new("divergent-confirmation");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("state");
    let output_pdf = directory.path().join("must-not-exist.pdf");
    let args = search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &output_pdf,
        &["18"],
        &["-0.15"],
        Some(&compiler),
    );

    let result = run_with_oracle(&args, &state, "divergent-confirmation", None);
    let error = stderr(&result);
    assert_eq!(result.status.code(), Some(2), "{error}");
    assert!(result.stdout.is_empty());
    assert!(!output_pdf.exists());
    assert_eq!(counter(&state, "version.count"), 1);
    assert_eq!(counter(&state, "compile.count"), 2);
    assert!(
        error.contains("non-deterministic-winner") || error.to_lowercase().contains("determin"),
        "{error}"
    );
}

#[test]
fn raster_and_unknown_plan_fail_before_the_compiler_and_existing_destination_is_intact() {
    let directory = TestDirectory::new("ordering-and-destination");
    let compiler = install_oracle(directory.path());

    let mut changed = fs::read(fixture("page.pgm")).unwrap();
    *changed.last_mut().unwrap() ^= 1;
    let changed_raster = directory.path().join("changed.pgm");
    fs::write(&changed_raster, changed).unwrap();
    let raster_state = directory.path().join("raster-state");
    let raster_pdf = directory.path().join("raster.pdf");
    let raster_args = search_args(
        &fixture("observation.json"),
        &changed_raster,
        &raster_pdf,
        &["18"],
        &["-0.15"],
        Some(&compiler),
    );
    let raster_result = run_with_oracle(&raster_args, &raster_state, "trace", None);
    let raster_error = stderr(&raster_result);
    assert_eq!(raster_result.status.code(), Some(2), "{raster_error}");
    assert!(raster_result.stdout.is_empty());
    assert!(
        raster_error.contains("SHA-256") || raster_error.contains("raster:"),
        "{raster_error}"
    );
    assert_eq!(counter(&raster_state, "version.count"), 0);
    assert_eq!(counter(&raster_state, "compile.count"), 0);
    assert!(!raster_pdf.exists());

    let plan_state = directory.path().join("plan-state");
    let plan_pdf = directory.path().join("plan.pdf");
    let plan_args = search_args(
        &observation_fixture("valid_mixed_known_unknown.json"),
        &observation_fixture("fixture-raster-v1.pgm"),
        &plan_pdf,
        &["18"],
        &["-0.15"],
        Some(&compiler),
    );
    let plan_result = run_with_oracle(&plan_args, &plan_state, "trace", None);
    let plan_error = stderr(&plan_result);
    assert_eq!(plan_result.status.code(), Some(2), "{plan_error}");
    assert!(plan_result.stdout.is_empty());
    assert!(
        plan_error.contains("page_mapping")
            || plan_error.contains("mapeamento")
            || plan_error.contains("planejamento"),
        "{plan_error}"
    );
    assert_eq!(counter(&plan_state, "version.count"), 0);
    assert_eq!(counter(&plan_state, "compile.count"), 0);
    assert!(!plan_pdf.exists());

    let destination = directory.path().join("already-exists.pdf");
    let sentinel = b"pre-existing bytes must survive";
    fs::write(&destination, sentinel).unwrap();
    let destination_result = run(&search_args(
        &fixture("observation.json"),
        &fixture("page.pgm"),
        &destination,
        &["18"],
        &["-0.15"],
        None,
    ));
    let destination_error = stderr(&destination_result);
    assert_eq!(
        destination_result.status.code(),
        Some(2),
        "{destination_error}"
    );
    assert!(destination_result.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap().as_slice(), sentinel);
    let normalized = destination_error.to_lowercase();
    assert!(
        normalized.contains("destin")
            || normalized.contains("exist")
            || normalized.contains("publica"),
        "{destination_error}"
    );
}

#[test]
fn closed_grid_rejects_33_hypotheses_before_io_and_existing_commands_do_not_regress() {
    let directory = TestDirectory::new("closed-grid-and-regression");
    let compiler = install_oracle(directory.path());
    let state = directory.path().join("closed-state");
    let oversized = search_args(
        Path::new("missing-observation.json"),
        Path::new("missing-raster.pgm"),
        &directory.path().join("must-not-exist.pdf"),
        &["4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14"],
        &["-1", "0", "1"],
        Some(&compiler),
    );
    let rejected = run_with_oracle(&oversized, &state, "trace", None);
    let rejection = stderr(&rejected);
    assert_eq!(rejected.status.code(), Some(2), "{rejection}");
    assert!(rejected.stdout.is_empty());
    assert!(
        rejection.contains("32")
            || rejection.to_lowercase().contains("hipótes")
            || rejection.to_lowercase().contains("grade"),
        "{rejection}"
    );
    assert_eq!(counter(&state, "version.count"), 0);
    assert_eq!(counter(&state, "compile.count"), 0);

    let observation = fixture("observation.json");
    let raster = fixture("page.pgm");
    let reconstructed = run(&reconstruction_args(&observation, &raster));
    assert!(reconstructed.status.success(), "{}", stderr(&reconstructed));
    assert!(reconstructed.stderr.is_empty());
    assert_eq!(reconstructed.stdout, expected_source("10", "0"));

    let evaluation_pdf = directory.path().join("evaluation.pdf");
    let evaluated = run(&evaluation_args(&observation, &raster, &evaluation_pdf));
    assert!(evaluated.status.success(), "{}", stderr(&evaluated));
    assert!(evaluated.stderr.is_empty());
    assert!(fs::read(evaluation_pdf).unwrap().starts_with(b"%PDF"));
    assert!(compact_json(&evaluated.stdout).starts_with(
        "{\"schema\":\"decalque.scan-reconstruction-evaluation\",\"schema_version\":1,"
    ));
}
