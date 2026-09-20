//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/scan-observation-json-adapter.md
//! @layer L3
//! @updated 2026-09-18
//!
//! Oráculos independentes da implementação: exercitam somente a fronteira JSON pública.

use decalque_infra::{
    load_scan_observation_json, parse_scan_observation_json, ScanObservationJsonError,
    ScanObservationLimits,
};
use std::path::{Path, PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scan_observation")
        .join(name)
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(fixture_path(name)).expect("fixture ScanObservation deve existir")
}

fn generous_limits() -> ScanObservationLimits {
    ScanObservationLimits {
        max_input_bytes: 1_000_000,
        max_units: 1_000,
        max_provenance_records: 1_000,
        max_geometry_points: 10_000,
        max_total_text_bytes: 1_000_000,
    }
}

fn assert_contract_violation(name: &str, path_prefix: &str) {
    let bytes = fixture(name);
    match parse_scan_observation_json(&bytes, &generous_limits()) {
        Err(ScanObservationJsonError::ContractViolation { path, .. }) => {
            assert!(
                path.starts_with(path_prefix),
                "{name}: caminho {path:?} deveria começar por {path_prefix:?}"
            );
        }
        _ => panic!("{name}: deveria produzir ContractViolation"),
    }
}

fn assert_budget(name: &str, limits: ScanObservationLimits, expected_resource: &str) {
    let bytes = fixture(name);
    match parse_scan_observation_json(&bytes, &limits) {
        Err(ScanObservationJsonError::BudgetExceeded {
            resource,
            limit,
            actual,
        }) => {
            assert_eq!(resource, expected_resource);
            assert!(actual > limit, "o consumo real deve exceder o limite");
        }
        _ => panic!("{name}: deveria exceder o orçamento {expected_resource}"),
    }
}

#[test]
fn json_valido_com_claims_known_e_unknown_e_aceito() {
    let bytes = fixture("valid_mixed_known_unknown.json");

    assert!(parse_scan_observation_json(&bytes, &generous_limits()).is_ok());
    assert!(load_scan_observation_json(
        &fixture_path("valid_mixed_known_unknown.json"),
        &generous_limits()
    )
    .is_ok());
}

#[test]
fn reordenar_campos_json_preserva_o_mesmo_modelo() {
    let canonical_bytes = fixture("valid_mixed_known_unknown.json");
    let canonical_json =
        std::str::from_utf8(&canonical_bytes).expect("fixture válida deve ser UTF-8");
    let original_header = concat!(
        "{\n",
        "  \"schema\": \"decalque.scan-observation\",\n",
        "  \"schema_version\": 1,"
    );
    let reordered_header = concat!(
        "{\n",
        "  \"schema_version\": 1,\n",
        "  \"schema\": \"decalque.scan-observation\","
    );
    assert!(canonical_json.starts_with(original_header));
    let reordered_json = canonical_json.replacen(original_header, reordered_header, 1);

    let canonical_model = match parse_scan_observation_json(&canonical_bytes, &generous_limits()) {
        Ok(model) => model,
        Err(_) => panic!("fixture canônica deveria ser aceita"),
    };
    let reordered_model =
        match parse_scan_observation_json(reordered_json.as_bytes(), &generous_limits()) {
            Ok(model) => model,
            Err(_) => panic!("reordenação de campos deveria ser aceita"),
        };

    assert!(
        canonical_model == reordered_model,
        "ordem de campos JSON não pode alterar o modelo de domínio"
    );
}

#[test]
fn schema_e_versao_nao_suportados_nao_fazem_fallback() {
    for (name, expected_schema, expected_version) in [
        ("invalid_schema.json", "vendor.scan-observation", 1),
        ("invalid_version.json", "decalque.scan-observation", 2),
    ] {
        let bytes = fixture(name);
        match parse_scan_observation_json(&bytes, &generous_limits()) {
            Err(ScanObservationJsonError::UnsupportedSchema { schema, version }) => {
                assert_eq!(schema, expected_schema);
                assert_eq!(version, expected_version);
            }
            _ => panic!("{name}: deveria produzir UnsupportedSchema"),
        }
    }
}

#[test]
fn campo_desconhecido_e_rejeitado_com_caminho_exato() {
    let bytes = fixture("unknown_field.json");

    match parse_scan_observation_json(&bytes, &generous_limits()) {
        Err(ScanObservationJsonError::UnknownField { path, field }) => {
            assert_eq!(path, "$");
            assert_eq!(field, "ocr_model");
        }
        _ => panic!("campo desconhecido deveria produzir UnknownField"),
    }
}

#[test]
fn chave_json_duplicada_e_rejeitada_mesmo_com_valores_iguais() {
    let bytes = fixture("duplicate_key.json");

    assert!(matches!(
        parse_scan_observation_json(&bytes, &generous_limits()),
        Err(ScanObservationJsonError::JsonSyntax { .. })
    ));
}

#[test]
fn null_nao_e_convertido_em_unknown() {
    assert_contract_violation("null_claim.json", "$.units[0].geometry.bbox");
}

#[test]
fn known_com_campo_de_unknown_e_rejeitado() {
    let bytes = fixture("contaminated_known_claim.json");

    match parse_scan_observation_json(&bytes, &generous_limits()) {
        Err(ScanObservationJsonError::UnknownField { path, field }) => {
            assert_eq!(path, "$.units[0].text");
            assert_eq!(field, "reason");
        }
        _ => panic!("Known contaminado deveria produzir UnknownField"),
    }
}

#[test]
fn nan_e_erro_de_sintaxe_e_nunca_vira_valor() {
    let bytes = fixture("nan_coordinate.json");

    assert!(matches!(
        parse_scan_observation_json(&bytes, &generous_limits()),
        Err(ScanObservationJsonError::JsonSyntax { .. })
    ));
}

#[test]
fn overflow_numerico_e_violacao_de_contrato() {
    assert_contract_violation(
        "overflow_coordinate.json",
        "$.units[0].geometry.bbox.value.x0",
    );
}

#[test]
fn referencias_de_unidade_e_proveniencia_precisam_existir() {
    assert_contract_violation("missing_parent_reference.json", "$.units[0].parent_id");
    assert_contract_violation(
        "missing_provenance_reference.json",
        "$.units[0].text.evidence",
    );
}

#[test]
fn ciclos_de_unidades_e_proveniencia_sao_rejeitados() {
    assert_contract_violation("unit_cycle.json", "$.units");
    assert_contract_violation("provenance_cycle.json", "$.provenance");
}

#[test]
fn raiz_manual_sem_referencia_ao_raster_e_rejeitada_na_fronteira_json() {
    assert_contract_violation("manual_root_without_raster.json", "$.provenance");
}

#[test]
fn limite_de_bytes_e_verificado_antes_da_construcao_do_modelo() {
    let bytes = fixture("valid_mixed_known_unknown.json");
    let mut limits = generous_limits();
    limits.max_input_bytes = bytes.len() - 1;

    match parse_scan_observation_json(&bytes, &limits) {
        Err(ScanObservationJsonError::InputTooLarge { limit, actual }) => {
            assert_eq!(limit, bytes.len() - 1);
            assert_eq!(actual, bytes.len());
        }
        _ => panic!("entrada grande deveria produzir InputTooLarge"),
    }
}

#[test]
fn budgets_de_unidades_proveniencia_pontos_e_texto_sao_independentes() {
    let mut units = generous_limits();
    units.max_units = 1;
    assert_budget("valid_mixed_known_unknown.json", units, "units");

    let mut provenance = generous_limits();
    provenance.max_provenance_records = 1;
    assert_budget(
        "valid_mixed_known_unknown.json",
        provenance,
        "provenance_records",
    );

    let mut points = generous_limits();
    points.max_geometry_points = 5;
    assert_budget("valid_geometry_points.json", points, "geometry_points");

    let mut text = generous_limits();
    text.max_total_text_bytes = 1;
    assert_budget("valid_mixed_known_unknown.json", text, "text_bytes");
}
