//! Testes black-box do comando `derive-scan-layout`.
//!
//! O comportamento é observado por exit/stdout/stderr e efeitos no destino.

use serde_json::{json, Value};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const RASTER_SHA256: &str = "99e7a39e216cc95a77a7f32e5fd138af04157cf9a78fa58d3bb5af1f305f80b3";
const USAGE_ERROR: &[u8] = b"derive-scan-layout: usage-error\n";
const OBSERVATION_ERROR: &[u8] = b"derive-scan-layout: observation-error\n";
const RASTER_BIND_ERROR: &[u8] = b"derive-scan-layout: raster-bind-error\n";
const PUBLICATION_ERROR: &[u8] = b"derive-scan-layout: publication-error\n";

static SANDBOX_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "decalque-scan-layout-cli-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("sandbox CLI deve ser criado");
        Self { root }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_decalque")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scan_layout_profile")
        .join(name)
}

fn fixture_raster() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reconstruction/page.pgm")
}

fn run(args: &[OsString]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("processo decalque deve iniciar")
}

fn derive_args(observation: &Path, raster: &Path, destination: Option<&Path>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("derive-scan-layout"),
        observation.as_os_str().to_os_string(),
        OsString::from("--raster"),
        raster.as_os_str().to_os_string(),
    ];
    if let Some(destination) = destination {
        args.push(OsString::from("--output-typst"));
        args.push(destination.as_os_str().to_os_string());
    }
    args
}

fn assert_first_marker(output: &Output, marker: &[u8]) {
    assert_eq!(output.status.code(), Some(2), "stderr={:?}", output.stderr);
    assert!(output.stdout.is_empty(), "falha nao pode vazar stdout");
    let first_lf = output
        .stderr
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("falha deve terminar o marcador inicial com LF");
    assert_eq!(&output.stderr[..=first_lf], marker);
}

fn assert_success_json(output: &Output) -> Value {
    assert_eq!(output.status.code(), Some(0), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.first(), Some(&b'{'));
    assert_eq!(output.stdout.last(), Some(&b'}'));
    assert!(!output.stdout.ends_with(b"\n"));
    serde_json::from_slice(&output.stdout).expect("stdout deve ser um unico JSON canonico")
}

#[test]
fn parser_aceita_permutacoes_validas_e_canoniza_fixture_permutada() {
    let observation = fixture("known-regions.json");
    let raster = fixture_raster();
    let canonical = run(&derive_args(&observation, &raster, None));
    let permuted = run(&derive_args(
        &fixture("known-regions-permuted.json"),
        &raster,
        None,
    ));
    let canonical_json = assert_success_json(&canonical);
    assert_success_json(&permuted);
    assert_eq!(canonical.stdout, permuted.stdout);
    assert_eq!(canonical_json["source"]["page_index"], 37);
    assert_eq!(canonical_json["source"]["raster_sha256"], RASTER_SHA256);

    let sandbox = Sandbox::new("valid-permutations");
    let without_destination = [
        vec![
            OsString::from("derive-scan-layout"),
            observation.clone().into_os_string(),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
            observation.clone().into_os_string(),
        ],
    ];
    let mut unpublished_reports = Vec::new();
    for (index, args) in without_destination.into_iter().enumerate() {
        let output = run(&args);
        let json = assert_success_json(&output);
        assert_eq!(
            json["typst_artifact"]["published"], false,
            "permutacao sem destino {index}"
        );
        unpublished_reports.push(output.stdout);
    }
    assert_eq!(unpublished_reports[0], unpublished_reports[1]);

    let with_destination = [
        vec![
            OsString::from("derive-scan-layout"),
            observation.clone().into_os_string(),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
            OsString::from("--output-typst"),
            sandbox.root.join("ord.typ").into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            observation.clone().into_os_string(),
            OsString::from("--output-typst"),
            sandbox.root.join("odr.typ").into_os_string(),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
            observation.clone().into_os_string(),
            OsString::from("--output-typst"),
            sandbox.root.join("rod.typ").into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
            OsString::from("--output-typst"),
            sandbox.root.join("rdo.typ").into_os_string(),
            observation.clone().into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            OsString::from("--output-typst"),
            sandbox.root.join("dor.typ").into_os_string(),
            observation.clone().into_os_string(),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
        ],
        vec![
            OsString::from("derive-scan-layout"),
            OsString::from("--output-typst"),
            sandbox.root.join("dro.typ").into_os_string(),
            OsString::from("--raster"),
            raster.clone().into_os_string(),
            observation.clone().into_os_string(),
        ],
    ];
    let mut published_reports = Vec::new();
    for (index, args) in with_destination.into_iter().enumerate() {
        let destination = args
            .iter()
            .position(|arg| arg == "--output-typst")
            .and_then(|position| args.get(position + 1))
            .map(PathBuf::from)
            .expect("permutacao publicada deve conter destino");
        let output = run(&args);
        let json = assert_success_json(&output);
        assert_eq!(
            json["typst_artifact"]["published"], true,
            "permutacao com destino {index}"
        );
        assert!(destination.exists());
        published_reports.push(output.stdout);
    }
    assert!(published_reports.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn argv_invalido_precede_toda_entrada_de_dominio_e_preserva_destino() {
    let sandbox = Sandbox::new("usage");
    let malformed = sandbox.root.join("malformed.json");
    let absent_raster = sandbox.root.join("absent.pgm");
    let destination = sandbox.root.join("sentinel.typ");
    fs::write(&malformed, b"{").unwrap();
    fs::write(&destination, b"usage-sentinel").unwrap();

    let cases = [
        (
            "opcao desconhecida",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
                "--unknown-layout".into(),
            ],
        ),
        (
            "raster repetido",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
            ],
        ),
        (
            "raster sem valor",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
                "--raster".into(),
            ],
        ),
        (
            "raster ausente",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
            ],
        ),
        (
            "output repetido",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
            ],
        ),
        (
            "output sem valor",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
            ],
        ),
        (
            "posicional ausente",
            vec![
                "derive-scan-layout".into(),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
            ],
        ),
        (
            "segundo posicional",
            vec![
                "derive-scan-layout".into(),
                malformed.clone().into_os_string(),
                OsString::from("second-positional"),
                "--raster".into(),
                absent_raster.clone().into_os_string(),
                "--output-typst".into(),
                destination.clone().into_os_string(),
            ],
        ),
    ];
    for (label, args) in cases {
        let output = run(&args);
        assert_first_marker(&output, USAGE_ERROR);
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"usage-sentinel",
            "{label}"
        );
    }
}

#[test]
fn bind_distingue_mismatch_de_sha256_e_de_dimensoes_com_atomicidade() {
    let sandbox = Sandbox::new("bind-mismatch");
    let fixture_value: Value =
        serde_json::from_slice(&fs::read(fixture("known-regions.json")).unwrap()).unwrap();

    let mut wrong_sha = fixture_value.clone();
    wrong_sha["source"]["raster"]["sha256"] = Value::String("00".repeat(32));
    let wrong_sha_path = sandbox.root.join("wrong-sha.json");
    fs::write(&wrong_sha_path, serde_json::to_vec(&wrong_sha).unwrap()).unwrap();
    let absent_destination = sandbox.root.join("sha-must-stay-absent.typ");
    let sha_failure = run(&derive_args(
        &wrong_sha_path,
        &fixture_raster(),
        Some(&absent_destination),
    ));
    assert_first_marker(&sha_failure, RASTER_BIND_ERROR);
    assert!(!absent_destination.exists());

    let mut wrong_dimensions = fixture_value;
    wrong_dimensions["source"]["raster"]["width_px"] = Value::from(3);
    wrong_dimensions["raster_frame"]["extent"][0] = Value::from(3);
    let wrong_dimensions_path = sandbox.root.join("wrong-dimensions.json");
    fs::write(
        &wrong_dimensions_path,
        serde_json::to_vec(&wrong_dimensions).unwrap(),
    )
    .unwrap();
    let sentinel_destination = sandbox.root.join("dimensions-sentinel.typ");
    fs::write(&sentinel_destination, b"dimensions-sentinel").unwrap();
    let dimensions_failure = run(&derive_args(
        &wrong_dimensions_path,
        &fixture_raster(),
        Some(&sentinel_destination),
    ));
    assert_first_marker(&dimensions_failure, RASTER_BIND_ERROR);
    assert_eq!(
        fs::read(&sentinel_destination).unwrap(),
        b"dimensions-sentinel"
    );
}

#[test]
fn marcadores_expoem_precedencia_observation_bind_publication_sem_trace() {
    let sandbox = Sandbox::new("precedence");
    let malformed = sandbox.root.join("malformed.json");
    let absent_raster = sandbox.root.join("absent.pgm");
    let destination = sandbox.root.join("sentinel.typ");
    fs::write(&malformed, b"{").unwrap();
    fs::write(&destination, b"precedence-sentinel").unwrap();

    let observation_failure = run(&derive_args(&malformed, &absent_raster, Some(&destination)));
    assert_first_marker(&observation_failure, OBSERVATION_ERROR);
    assert_eq!(fs::read(&destination).unwrap(), b"precedence-sentinel");

    let bind_failure = run(&derive_args(
        &fixture("known-regions.json"),
        &absent_raster,
        Some(&destination),
    ));
    assert_first_marker(&bind_failure, RASTER_BIND_ERROR);
    assert_eq!(fs::read(&destination).unwrap(), b"precedence-sentinel");

    let publication_failure = run(&derive_args(
        &fixture("known-regions.json"),
        &fixture_raster(),
        Some(&destination),
    ));
    assert_first_marker(&publication_failure, PUBLICATION_ERROR);
    assert_eq!(fs::read(&destination).unwrap(), b"precedence-sentinel");
}

#[test]
fn publicacao_e_atomica_e_unknown_nao_toca_destino() {
    let sandbox = Sandbox::new("publication");
    let destination = sandbox.root.join("fresh.typ");
    let output = run(&derive_args(
        &fixture("known-regions.json"),
        &fixture_raster(),
        Some(&destination),
    ));
    let report = assert_success_json(&output);
    let artifact = fs::read(&destination).expect("sucesso deve criar artefato");
    assert_eq!(report["typst_artifact"]["published"], true);
    assert_eq!(report["typst_artifact"]["size_bytes"], artifact.len());

    let unknown_path = sandbox.root.join("unknown.json");
    let mut unknown: Value =
        serde_json::from_slice(&fs::read(fixture("known-regions.json")).unwrap()).unwrap();
    unknown["page_mapping"] = json!({
        "status": "unknown",
        "reason": "not-observed",
        "evidence": [],
        "detail": "sem calibracao fisica"
    });
    fs::write(&unknown_path, serde_json::to_vec(&unknown).unwrap()).unwrap();

    let sentinel = sandbox.root.join("unknown-sentinel.typ");
    fs::write(&sentinel, b"unknown-sentinel").unwrap();
    let unknown_output = run(&derive_args(
        &unknown_path,
        &fixture_raster(),
        Some(&sentinel),
    ));
    let unknown_report = assert_success_json(&unknown_output);
    assert_eq!(unknown_report["page"]["status"], "unknown");
    assert_eq!(unknown_report["typst_artifact"]["status"], "unavailable");
    assert_eq!(unknown_report["typst_artifact"]["published"], false);
    assert_eq!(fs::read(&sentinel).unwrap(), b"unknown-sentinel");
}

#[cfg(unix)]
#[test]
fn caminhos_nativos_e_decoys_nao_substituem_source_da_observacao() {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;

    let sandbox =
        Sandbox::new("decoy-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd");
    let observation = sandbox
        .root
        .join(OsString::from_vec(b"observation-\xff.json".to_vec()));
    let raster = sandbox
        .root
        .join(OsString::from_vec(b"raster-\xfe.pgm".to_vec()));
    let destination = sandbox
        .root
        .join(OsString::from_vec(b"output-\xfd.typ".to_vec()));
    let lossy_decoy = sandbox.root.join("output-�.typ");
    fs::write(
        &observation,
        fs::read(fixture("known-regions.json")).unwrap(),
    )
    .unwrap();
    symlink(fixture_raster(), &raster).expect("raster deve ser referenciado, nao copiado");
    fs::write(&lossy_decoy, b"lossy-decoy").unwrap();

    let output = Command::new(binary())
        .current_dir(&sandbox.root)
        .args(derive_args(&observation, &raster, Some(&destination)))
        .output()
        .expect("processo deve aceitar argv nativo");
    let report = assert_success_json(&output);

    assert_eq!(report["source"]["page_index"], 37);
    assert_eq!(report["source"]["raster_sha256"], RASTER_SHA256);
    assert!(!output.stdout.windows(64).any(
        |window| window == b"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    ));
    assert!(destination.exists());
    assert_eq!(fs::read(&lossy_decoy).unwrap(), b"lossy-decoy");
}
