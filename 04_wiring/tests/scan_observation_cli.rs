//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cli-scan-observation-compare.md
//! @layer L4
//! @updated 2026-09-19
//!
//! Testes de caixa-preta: observam somente processo, stdout, stderr e código de saída.

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
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

fn pdf_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures")
        .join(name)
}

fn raster_fixture() -> PathBuf {
    observation_fixture("fixture-raster-v1.pgm")
}

fn temporary_path(label: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "decalque-bound-comparison-{}-{serial}-{label}",
        std::process::id()
    ))
}

fn run(args: &[&OsStr]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("o binário decalque deve iniciar")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout deve ser UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr deve ser UTF-8")
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

fn comparison_args<'a>(
    observation: &'a OsStr,
    candidate: &'a OsStr,
    raster: &'a OsStr,
) -> Vec<&'a OsStr> {
    vec![
        "scan-observation".as_ref(),
        observation,
        candidate,
        "--raster".as_ref(),
        raster,
        "--granularity".as_ref(),
        "word".as_ref(),
        "--horizontal-tolerance-pt".as_ref(),
        "2".as_ref(),
        "--baseline-tolerance-pt".as_ref(),
        "2".as_ref(),
    ]
}

#[test]
fn help_documenta_unknown_indice_do_artefato_e_codigo_zero() {
    let output = run(&["scan-observation".as_ref(), "--help".as_ref()]);

    assert!(output.status.success());
    assert!(stderr(&output).is_empty());
    let help = stdout(&output);
    assert!(help.contains("scan-observation"));
    assert!(help.contains("--granularity"));
    assert!(help.contains("--horizontal-tolerance-pt"));
    assert!(help.contains("--baseline-tolerance-pt"));
    assert!(help.contains("--raster"));
    assert!(help.to_lowercase().contains("unknown"));
    assert!(help.contains("source.page_index"));
    assert!(help.contains("0"));

    for forbidden in [
        "--page",
        "--ocr",
        "--model",
        "--server",
        "--device",
        "--dpi",
        "--rasterizer",
        "--font",
        "--typst",
    ] {
        assert!(
            !help.contains(forbidden),
            "help anunciou flag proibida {forbidden}"
        );
    }
}

#[test]
fn flags_de_ocr_modelo_rede_e_override_de_pagina_sao_rejeitadas() {
    for (flag, value) in [
        ("--page", "0"),
        ("--ocr", "paddle"),
        ("--model", "provider-model"),
        ("--server", "http://127.0.0.1:8080"),
        ("--device", "gpu"),
        ("--dpi", "300"),
        ("--rasterizer", "external"),
        ("--font", "Example Sans"),
        ("--typst", "template.typ"),
    ] {
        let mut args = comparison_args(
            "missing-observation.json".as_ref(),
            "missing.pdf".as_ref(),
            "missing.pgm".as_ref(),
        );
        args.push(flag.as_ref());
        args.push(value.as_ref());
        let output = run(&args);

        assert_eq!(output.status.code(), Some(2), "flag {flag}");
        assert!(output.stdout.is_empty(), "flag {flag} publicou stdout");
        let error = stderr(&output);
        assert!(error.contains(flag), "erro não identificou {flag}");
        assert!(
            error.to_lowercase().contains("desconhecid"),
            "{flag} deveria ser diagnosticada como opção desconhecida"
        );
    }
}

#[test]
fn nan_infinito_overflow_e_confianca_fora_da_faixa_sao_erros_de_uso() {
    for (option, value) in [
        ("--horizontal-tolerance-pt", "NaN"),
        ("--baseline-tolerance-pt", "inf"),
        ("--horizontal-tolerance-pt", "1e400"),
        ("--min-text-confidence", "-0.01"),
        ("--min-geometry-confidence", "1.01"),
    ] {
        let mut args = comparison_args(
            "missing-observation.json".as_ref(),
            "missing.pdf".as_ref(),
            "missing.pgm".as_ref(),
        );
        if let Some(position) = args.iter().position(|arg| *arg == OsStr::new(option)) {
            args[position + 1] = value.as_ref();
        } else {
            args.push(option.as_ref());
            args.push(value.as_ref());
        }
        let output = run(&args);

        assert_eq!(output.status.code(), Some(2), "{option}={value}");
        assert!(output.stdout.is_empty(), "{option}={value} publicou stdout");
        let error = stderr(&output);
        assert!(
            error.contains(option) || error.contains(value),
            "{option}={value} deveria ser identificado no diagnóstico"
        );
        assert!(
            !error.contains("erro de leitura"),
            "{option}={value} deveria falhar antes de I/O"
        );
    }
}

#[test]
fn cobertura_parcial_e_unknown_nao_sao_publicados_como_preserved() {
    // `typst.pdf` possui ToUnicode. O fixture associa "Decalque" pelo texto
    // estrutural e pelo intervalo escalar real da linha "O Decalque"; assim,
    // este oráculo não depende de inferência a partir de `glyph_code`.
    let observation = observation_fixture("cli_typst_partial_unicode.json");
    let candidate = pdf_fixture("typst.pdf");
    let raster = raster_fixture();
    let output = run(&comparison_args(
        observation.as_os_str(),
        candidate.as_os_str(),
        raster.as_os_str(),
    ));

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report = compact_json(&stdout(&output));

    assert!(report.starts_with('{') && report.ends_with('}'));
    assert!(report.contains("\"schema\":\"decalque.scan-comparison-report\""));
    assert!(report.contains("\"schema_version\":1"));
    assert!(report.contains("\"source\":{\"page_index\":0,"));
    assert!(report.contains("\"granularity\":\"word\""));
    assert!(report.contains("\"matched_scan\":1"));
    assert!(report.contains("\"total_scan\":2"));
    assert!(report.contains("\"content_status\":\"unknown\""));
    assert!(report.contains("\"geometry_status\":\"unknown\""));
    assert!(report.contains("\"overall_status\":\"unknown\""));
    assert!(!report.contains("\"overall_status\":\"preserved\""));
    assert!(report.contains("\"unmatched_scan\":[\"word-absent\"]"));
    assert!(report.contains("\"observation_diagnostics\":["));
    assert!(report.contains("\"pdf_diagnostics\":["));
    assert!(report.contains("\"comparison_diagnostics\":["));
}

#[test]
fn escopo_vazio_publica_tres_status_unknown_e_diagnostico() {
    let observation = observation_fixture("valid_empty_unknown_mapping.json");
    let candidate = pdf_fixture("scan.pdf");
    let raster = raster_fixture();
    let output = run(&comparison_args(
        observation.as_os_str(),
        candidate.as_os_str(),
        raster.as_os_str(),
    ));

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report = compact_json(&stdout(&output));

    assert!(report.contains("\"matched_scan\":0"));
    assert!(report.contains("\"total_scan\":0"));
    assert!(report.contains("\"matched_candidate\":0"));
    assert!(report.contains("\"total_candidate\":0"));
    assert!(report.contains("\"content_status\":\"unknown\""));
    assert!(report.contains("\"geometry_status\":\"unknown\""));
    assert!(report.contains("\"overall_status\":\"unknown\""));
    assert!(!report.contains("\"content_status\":\"preserved\""));
    assert!(!report.contains("\"geometry_status\":\"preserved\""));
    assert!(!report.contains("\"overall_status\":\"preserved\""));
    assert!(report.contains("\"code\":\"empty-comparison-scope\""));
}

#[test]
fn erro_da_observacao_retorna_dois_sem_relatorio_parcial() {
    let observation = observation_fixture("unknown_field.json");
    let candidate = pdf_fixture("textops.pdf");
    let raster = raster_fixture();
    let output = run(&comparison_args(
        observation.as_os_str(),
        candidate.as_os_str(),
        raster.as_os_str(),
    ));

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("observa"));
    assert!(error.contains("ocr_model"));
}

#[test]
fn raster_ausente_ou_repetido_falha_antes_de_io() {
    let without_raster = run(&[
        "scan-observation".as_ref(),
        "missing-observation.json".as_ref(),
        "missing.pdf".as_ref(),
        "--granularity".as_ref(),
        "word".as_ref(),
        "--horizontal-tolerance-pt".as_ref(),
        "2".as_ref(),
        "--baseline-tolerance-pt".as_ref(),
        "2".as_ref(),
    ]);
    assert_eq!(without_raster.status.code(), Some(2));
    assert!(without_raster.stdout.is_empty());
    assert!(stderr(&without_raster).contains("--raster"));
    assert!(!stderr(&without_raster).contains("leitura"));

    let mut repeated = comparison_args(
        "missing-observation.json".as_ref(),
        "missing.pdf".as_ref(),
        "one.pgm".as_ref(),
    );
    repeated.push("--raster".as_ref());
    repeated.push("two.pgm".as_ref());
    let repeated = run(&repeated);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(repeated.stdout.is_empty());
    assert!(stderr(&repeated).contains("repetida"));
    assert!(!stderr(&repeated).contains("leitura"));
}

#[test]
fn raster_e_vinculado_antes_de_abrir_o_candidato() {
    let observation = observation_fixture("valid_empty_unknown_mapping.json");
    let missing_candidate = temporary_path("missing.pdf");

    let truncated = temporary_path("truncated.pgm");
    fs::write(&truncated, b"P5\n2 2\n255\n\x00").unwrap();
    let truncated_output = run(&comparison_args(
        observation.as_os_str(),
        missing_candidate.as_os_str(),
        truncated.as_os_str(),
    ));
    let _ = fs::remove_file(&truncated);
    assert_eq!(truncated_output.status.code(), Some(2));
    assert!(truncated_output.stdout.is_empty());
    assert!(stderr(&truncated_output).contains("raster:"));
    assert!(!stderr(&truncated_output).contains("candidato:"));

    let changed = temporary_path("changed.pgm");
    let mut bytes = fs::read(raster_fixture()).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&changed, bytes).unwrap();
    let changed_output = run(&comparison_args(
        observation.as_os_str(),
        missing_candidate.as_os_str(),
        changed.as_os_str(),
    ));
    let _ = fs::remove_file(&changed);
    assert_eq!(changed_output.status.code(), Some(2));
    assert!(changed_output.stdout.is_empty());
    assert!(stderr(&changed_output).contains("SHA-256"));
    assert!(!stderr(&changed_output).contains("candidato:"));
}
