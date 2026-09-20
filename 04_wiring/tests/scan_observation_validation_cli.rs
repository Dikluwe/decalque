//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cli-scan-observation-validate.md
//! @layer L4
//! @updated 2026-09-18

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_decalque")
}

fn observation_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures/scan_observation")
        .join(name)
}

fn raster_fixture() -> PathBuf {
    observation_fixture("fixture-raster-v1.pgm")
}

fn run(args: &[&OsStr]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("o binário decalque deve iniciar")
}

fn validation_args<'a>(observation: &'a Path, raster: &'a Path) -> Vec<&'a OsStr> {
    vec![
        "validate-scan-observation".as_ref(),
        observation.as_os_str(),
        "--raster".as_ref(),
        raster.as_os_str(),
    ]
}

fn temporary_path(label: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "decalque-validation-{}-{serial}-{label}",
        std::process::id()
    ))
}

#[test]
fn aceita_contrato_vinculado_e_publica_resumo_exato_deterministico() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    let raster = raster_fixture();
    let first = run(&validation_args(&observation, &raster));
    let second = run(&validation_args(&observation, &raster));

    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stderr, b"");
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        concat!(
            "{\"schema\":\"decalque.scan-observation-validation\",",
            "\"schema_version\":1,\"status\":\"valid\",",
            "\"scope\":\"contract-and-raster-identity\",",
            "\"source\":{\"page_index\":0,",
            "\"raster_sha256\":\"0e35216434d3891866924eb171c9c7f9960bc2f5aae9b1600c2aae74446eb728\",",
            "\"media_type\":\"image/x-portable-graymap\",\"width_px\":2,\"height_px\":2},",
            "\"counts\":{\"units\":4,\"provenance_records\":2,\"diagnostics\":0}}\n"
        )
    );
}

#[test]
fn rejeita_byte_alterado_media_type_divergente_e_json_invalido_sem_stdout() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    let raster = raster_fixture();

    let changed_path = temporary_path("changed.pgm");
    let mut changed = fs::read(&raster).unwrap();
    *changed.last_mut().unwrap() ^= 1;
    fs::write(&changed_path, changed).unwrap();
    let changed_result = run(&validation_args(&observation, &changed_path));
    let _ = fs::remove_file(&changed_path);
    assert_eq!(changed_result.status.code(), Some(2));
    assert!(changed_result.stdout.is_empty());

    let media_path = temporary_path("media.json");
    let source = fs::read_to_string(&observation).unwrap();
    fs::write(
        &media_path,
        source.replace("image/x-portable-graymap", "image/png"),
    )
    .unwrap();
    let media_result = run(&validation_args(&media_path, &raster));
    let _ = fs::remove_file(&media_path);
    assert_eq!(media_result.status.code(), Some(2));
    assert!(media_result.stdout.is_empty());

    let dimensions_path = temporary_path("dimensions.json");
    fs::write(
        &dimensions_path,
        source
            .replace("\"width_px\": 2", "\"width_px\": 3")
            .replace("\"extent\": [2, 2]", "\"extent\": [3, 2]"),
    )
    .unwrap();
    let dimensions_result = run(&validation_args(&dimensions_path, &raster));
    let _ = fs::remove_file(&dimensions_path);
    assert_eq!(dimensions_result.status.code(), Some(2));
    assert!(dimensions_result.stdout.is_empty());

    let invalid = observation_fixture("unknown_field.json");
    let invalid_result = run(&validation_args(&invalid, &raster));
    assert_eq!(invalid_result.status.code(), Some(2));
    assert!(invalid_result.stdout.is_empty());
}

#[test]
fn artifact_id_com_url_nao_e_dereferenciado() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    let raster = raster_fixture();
    let sentinel_path = temporary_path("sentinel.json");
    let source = fs::read_to_string(&observation).unwrap();
    fs::write(
        &sentinel_path,
        source.replace("fixture-raster-v1", "http://127.0.0.1:9/nao-acessar"),
    )
    .unwrap();

    let output = run(&validation_args(&sentinel_path, &raster));
    let _ = fs::remove_file(sentinel_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn rejeita_raster_truncado_e_formato_nao_suportado() {
    let observation = observation_fixture("valid_mixed_known_unknown.json");
    for (label, bytes) in [
        ("truncated.pgm", b"P5\n2 2\n255\n\x00".as_slice()),
        ("unsupported.gif", b"GIF89a".as_slice()),
    ] {
        let path = temporary_path(label);
        fs::write(&path, bytes).unwrap();
        let output = run(&validation_args(&observation, &path));
        let _ = fs::remove_file(path);
        assert_eq!(output.status.code(), Some(2), "{label}");
        assert!(output.stdout.is_empty(), "{label}");
    }
}

#[test]
fn help_explica_escopo_e_flags_externas_sao_rejeitadas_antes_de_io() {
    let help = run(&["validate-scan-observation".as_ref(), "--help".as_ref()]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap().to_lowercase();
    assert!(text.contains("--raster"));
    assert!(text.contains("identidade"));
    assert!(text.contains("não prova") || text.contains("nao prova"));

    for (flag, value) in [
        ("--candidate", "candidate.pdf"),
        ("--ocr", "paddle"),
        ("--model", "model"),
        ("--server", "http://127.0.0.1:9"),
        ("--dpi", "300"),
        ("--font", "Example"),
        ("--typst", "page.typ"),
    ] {
        let output = run(&[
            "validate-scan-observation".as_ref(),
            "missing.json".as_ref(),
            "--raster".as_ref(),
            "missing.pgm".as_ref(),
            flag.as_ref(),
            value.as_ref(),
        ]);
        assert_eq!(output.status.code(), Some(2), "{flag}");
        assert!(output.stdout.is_empty(), "{flag}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(flag), "{flag}: {error}");
        assert!(
            !error.contains("leitura"),
            "{flag} deveria falhar antes de I/O"
        );
    }
}
