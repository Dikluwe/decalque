//! Testes das fronteiras usadas pelo perfil geométrico.
//!
//! Este arquivo exercita apenas as fronteiras L3 preexistentes: strict load,
//! bind de raster e publicacao atomica no-clobber. Nao define semantica de
//! layout nem conhece o renderer ou a composição L4.

use decalque_infra::{
    bind_scan_observation_raster, load_scan_observation_json, publish_new_file,
    RasterIdentityError, ScanObservationJsonError, ScanObservationLimits,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const RASTER_SHA256: &str = "99e7a39e216cc95a77a7f32e5fd138af04157cf9a78fa58d3bb5af1f305f80b3";

static SANDBOX_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "decalque-scan-layout-infra-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("sandbox L3 deve ser criado");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn wiring_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("03_infra deve ter raiz do workspace")
        .join("04_wiring")
}

fn fixture(name: &str) -> PathBuf {
    wiring_root()
        .join("tests/fixtures/scan_layout_profile")
        .join(name)
}

fn fixture_raster() -> PathBuf {
    wiring_root().join("tests/fixtures/reconstruction/page.pgm")
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

#[test]
fn strict_load_e_bind_reusam_as_fronteiras_existentes() {
    for name in ["known-regions.json", "known-regions-permuted.json"] {
        let observation = load_scan_observation_json(&fixture(name), &generous_limits())
            .expect("fixture deve atravessar o strict load existente");

        assert_eq!(observation.source.page_index, 37);
        assert_eq!(observation.source.raster.sha256, RASTER_SHA256);
        assert_eq!(observation.units.len(), 5);

        let bound = bind_scan_observation_raster(&observation, &fixture_raster(), 1_000_000)
            .expect("fixture deve vincular o raster baseline pelos bytes reais");
        assert_eq!(bound.sha256, RASTER_SHA256);
        assert_eq!(bound.metadata.media_type, "image/x-portable-graymap");
        assert_eq!((bound.metadata.width_px, bound.metadata.height_px), (2, 2));
    }
}

#[test]
fn strict_load_e_bind_rejeitam_entradas_sem_reparo_ou_fallback() {
    let sandbox = Sandbox::new("strict-bind");
    let canonical =
        fs::read_to_string(fixture("known-regions.json")).expect("fixture deve ser legivel");

    let malformed = sandbox.path("unknown-field.json");
    fs::write(
        &malformed,
        canonical.replacen('{', "{\n  \"unexpected_field\": true,", 1),
    )
    .expect("entrada JSON alterada deve ser escrito");
    assert!(matches!(
        load_scan_observation_json(&malformed, &generous_limits()),
        Err(ScanObservationJsonError::UnknownField { field, .. })
            if field == "unexpected_field"
    ));

    let mut wrong_hash =
        load_scan_observation_json(&fixture("known-regions.json"), &generous_limits())
            .expect("controle deve carregar");
    wrong_hash.source.raster.sha256 = "00".repeat(32);
    assert!(matches!(
        bind_scan_observation_raster(&wrong_hash, &fixture_raster(), 1_000_000),
        Err(RasterIdentityError::Sha256Mismatch { .. })
    ));

    let mut wrong_dimensions =
        load_scan_observation_json(&fixture("known-regions.json"), &generous_limits())
            .expect("controle deve carregar");
    wrong_dimensions.source.raster.width_px = 3;
    assert!(matches!(
        bind_scan_observation_raster(&wrong_dimensions, &fixture_raster(), 1_000_000),
        Err(RasterIdentityError::DimensionsMismatch { .. })
    ));
}

#[test]
fn publish_new_file_transporta_bytes_exatos_e_nunca_sobrescreve() {
    let sandbox = Sandbox::new("publish");
    let destination = sandbox.path("layout.typ");
    let payload = b"#let scan_layout_profile = (schema: \"0013\",)\n";

    publish_new_file(&destination, payload).expect("destino ausente deve ser publicado");
    assert_eq!(fs::read(&destination).unwrap(), payload);

    let before = fs::read(&destination).unwrap();
    assert!(publish_new_file(&destination, b"replacement").is_err());
    assert_eq!(fs::read(&destination).unwrap(), before);

    let mut entries = fs::read_dir(&sandbox.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        vec![destination.file_name().unwrap().to_os_string()]
    );
}
