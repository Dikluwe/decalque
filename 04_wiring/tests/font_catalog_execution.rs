//! Testes da execução do exportador de evidência tipográfica.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../03_infra/tests/fixtures")
        .join(name)
}

#[test]
fn argumentos_ausentes_sao_erro_de_uso() {
    let output = Command::new(env!("CARGO_BIN_EXE_decalque-font-catalog"))
        .output()
        .expect("exportador deve executar");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("uso:"));
    assert!(output.stdout.is_empty());
}

#[test]
fn exporta_glifos_e_fontes_do_pdf_real_em_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_decalque-font-catalog"))
        .arg(fixture("typst.pdf"))
        .output()
        .expect("exportador deve executar");
    let stdout = String::from_utf8(output.stdout).expect("saída deve ser UTF-8");

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.starts_with("{\"schema_version\":2,"));
    assert!(stdout.contains("\"page\":{\"width_pt\":"));
    assert!(stdout.contains("\"base_font\":\"TARKWA+LibertinusSerif-Bold-Identity-H\""));
    assert!(stdout.contains("\"font_size_pt\":24"));
    assert!(stdout.ends_with("]}\n"));
}
