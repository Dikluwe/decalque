//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cli-digital-compare.md
//! @layer L4
//! @updated 2026-09-14
//!
//! Testes de caixa-preta: executam o binário e observam somente processo,
//! stdout, stderr e código de saída.

use std::path::PathBuf;
use std::process::{Command, Output};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_decalque")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures")
        .join(name)
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("o binário decalque deve iniciar")
}

fn run_from(current_dir: &std::path::Path, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(binary())
        .current_dir(current_dir)
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

#[test]
fn help_exibe_uso_e_termina_com_sucesso() {
    let output = run(&["--help".as_ref()]);

    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "uso: decalque <referencia.pdf> <candidato.pdf> [--page <indice>]\n"
    );
    assert!(stderr(&output).is_empty());
}

#[test]
fn argumentos_ausentes_sao_erro_de_uso() {
    let output = run(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: esperados dois caminhos"));
    assert!(error.contains("uso: decalque"));
}

#[test]
fn compara_fixture_real_consigo_mesmo() {
    let pdf = fixture("textops.pdf");
    let output = run(&[pdf.as_os_str(), pdf.as_os_str()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stderr(&output).is_empty());
    assert_eq!(
        stdout(&output),
        concat!(
            "pares: 10\n",
            "não emparelhados: A=0 B=0\n",
            "cobertura: A=10/10 B=10/10\n",
            "mediana |dx|: 0.000 pt\n",
            "mediana |dy|: 0.000 pt\n",
            "máximo |dx|: 0.000 pt\n",
            "máximo |dy|: 0.000 pt\n",
        )
    );
}

#[test]
fn pagina_inexistente_identifica_o_lado_de_referencia() {
    let pdf = fixture("textops.pdf");
    let output = run(&[
        pdf.as_os_str(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "999".as_ref(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.contains("erro: referência:"));
    assert!(error.contains("página 999 não existe no documento"));
}

#[test]
fn pdf_criptografado_e_reportado_como_erro_de_leitura() {
    let encrypted = fixture("typst_encrypted.pdf");
    let candidate = fixture("typst.pdf");
    let output = run(&[encrypted.as_os_str(), candidate.as_os_str()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.contains("erro: referência:"));
    assert!(error.contains("PDF criptografado"));
}

#[test]
fn pagina_sem_texto_exibe_metricas_indisponiveis() {
    let scan = fixture("scan.pdf");
    let output = run(&[scan.as_os_str(), scan.as_os_str()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("pares: 0\n"));
    assert!(report.contains("cobertura: A=0/0 B=0/0"));
    assert!(report.contains("mediana |dx|: n/a pt"));
    assert!(report.contains("máximo |dy|: n/a pt"));
    assert!(!report.contains("mediana |dx|: 0.000 pt"));
}

#[test]
fn documentos_divergentes_continuam_sendo_medicao_bem_sucedida() {
    let reference = fixture("textops.pdf");
    let candidate = fixture("typst.pdf");
    let output = run(&[reference.as_os_str(), candidate.as_os_str()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stderr(&output).is_empty());
    let report = stdout(&output);
    assert!(report.contains("não emparelhados: A=10 B=211"));
    assert!(report.contains("cobertura: A=0/10 B=0/211"));
}

#[test]
fn indice_de_pagina_nao_numerico_e_erro_de_uso() {
    let pdf = fixture("textops.pdf");
    let output = run(&[
        pdf.as_os_str(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "não-é-número".as_ref(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: índice de página inválido"));
    assert!(error.contains("uso: decalque"));
}

#[test]
fn falha_no_segundo_pdf_identifica_o_candidato() {
    let reference = fixture("textops.pdf");
    let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("arquivo-ausente.pdf");
    let output = run(&[reference.as_os_str(), missing.as_os_str()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).starts_with("erro: candidato: erro de leitura"));
}

#[test]
fn pagina_zero_explicita_equivale_ao_default() {
    let pdf = fixture("textops.pdf");
    let default = run(&[pdf.as_os_str(), pdf.as_os_str()]);
    let explicit = run(&[
        pdf.as_os_str(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "0".as_ref(),
    ]);

    assert!(default.status.success());
    assert!(explicit.status.success());
    assert_eq!(explicit.stdout, default.stdout);
    assert_eq!(explicit.stderr, default.stderr);
}

#[test]
fn pdf_com_xref_stream_e_processado_pelo_executavel() {
    let pdf = fixture("typst_xrefstream.pdf");
    let output = run(&[
        pdf.as_os_str(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "0".as_ref(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("pares: 211"));
    assert!(report.contains("cobertura: A=211/211 B=211/211"));
    assert!(report.contains("máximo |dx|: 0.000 pt"));
}

#[test]
fn opcao_desconhecida_e_rejeitada_na_fronteira_do_processo() {
    let pdf = fixture("textops.pdf");
    let output = run(&[
        pdf.as_os_str(),
        pdf.as_os_str(),
        "--unknown".as_ref(),
        "1".as_ref(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: opção desconhecida: --unknown"));
    assert!(error.contains("uso: decalque"));
}

#[test]
fn argumentos_excedentes_sao_rejeitados() {
    let output = run(&["a.pdf".as_ref(), "b.pdf".as_ref(), "c.pdf".as_ref()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("esperados dois caminhos"));
}

#[test]
fn arquivo_que_nao_e_pdf_e_erro_de_parse_da_referencia() {
    let invalid = fixture("SOURCES.md");
    let candidate = fixture("textops.pdf");
    let output = run(&[invalid.as_os_str(), candidate.as_os_str()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: referência: estrutura de PDF inválida:"));
    assert!(error.contains("invalid file header"));
}

#[test]
fn executavel_nao_depende_do_diretorio_de_trabalho() {
    let pdf = fixture("textops.pdf")
        .canonicalize()
        .expect("fixture deve existir");
    let output = run_from(
        std::env::temp_dir().as_path(),
        &[pdf.as_os_str(), pdf.as_os_str()],
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("cobertura: A=10/10 B=10/10"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn page_sem_valor_e_erro_de_uso() {
    let pdf = fixture("textops.pdf");
    let output = run(&[pdf.as_os_str(), pdf.as_os_str(), "--page".as_ref()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: esperados dois caminhos"));
    assert!(error.contains("uso: decalque"));
}

#[test]
fn indice_negativo_e_rejeitado_antes_de_ler_arquivos() {
    let output = run(&[
        "arquivo-que-não-existe-a.pdf".as_ref(),
        "arquivo-que-não-existe-b.pdf".as_ref(),
        "--page".as_ref(),
        "-1".as_ref(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(error.starts_with("erro: índice de página inválido"));
    assert!(!error.contains("erro de leitura"));
}

#[cfg(unix)]
#[test]
fn caminho_nao_utf8_chega_ao_carregador_sem_virar_erro_de_uso() {
    use std::os::unix::ffi::OsStrExt;

    let invalid_path = std::ffi::OsStr::from_bytes(b"inexistente-\xff.pdf");
    let candidate = fixture("textops.pdf");
    let output = run(&[invalid_path, candidate.as_os_str()]);

    assert_eq!(output.status.code(), Some(2));
    let error = stderr(&output);
    assert!(error.starts_with("erro: referência: erro de leitura"));
    assert!(!error.contains("índice de página inválido"));
    assert!(!error.contains("esperados dois caminhos"));
}
