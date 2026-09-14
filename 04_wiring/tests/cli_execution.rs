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
