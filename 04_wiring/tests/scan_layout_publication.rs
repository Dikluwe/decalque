//! Testes da finalização e publicação do bundle de layout.
//!
//! Os efeitos são observados somente pela API pública de `ScanLayoutPublisher`.

use decalque::{finalize_scan_layout_profile, ScanLayoutFinalizeErrorKind, ScanLayoutPublisher};
use decalque_core::{derive_scan_layout, Claim, UnknownReason};
use decalque_infra::{load_scan_observation_json, ScanObservationLimits};
use decalque_shell::render_scan_layout_profile;
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SANDBOX_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "decalque-scan-layout-publication-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("sandbox L4 deve ser criado");
        Self { root }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct Recorder {
    calls: Vec<(PathBuf, Vec<u8>)>,
}

impl ScanLayoutPublisher for Recorder {
    type Error = std::io::Error;

    fn publish_new_file(&mut self, destination: &Path, bytes: &[u8]) -> Result<(), Self::Error> {
        self.calls.push((destination.to_path_buf(), bytes.to_vec()));
        Ok(())
    }
}

#[derive(Debug)]
struct DeliberatePublicationFailure(&'static str);

impl fmt::Display for DeliberatePublicationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for DeliberatePublicationFailure {}

#[derive(Default)]
struct Failer {
    calls: Vec<(PathBuf, Vec<u8>)>,
}

impl ScanLayoutPublisher for Failer {
    type Error = DeliberatePublicationFailure;

    fn publish_new_file(&mut self, destination: &Path, bytes: &[u8]) -> Result<(), Self::Error> {
        self.calls.push((destination.to_path_buf(), bytes.to_vec()));
        Err(DeliberatePublicationFailure("publisher-failed"))
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scan_layout_profile")
        .join(name)
}

fn limits() -> ScanObservationLimits {
    ScanObservationLimits {
        max_input_bytes: 1_000_000,
        max_units: 1_000,
        max_provenance_records: 1_000,
        max_geometry_points: 10_000,
        max_total_text_bytes: 1_000_000,
    }
}

fn known_observation() -> decalque_core::ScanObservation {
    load_scan_observation_json(&fixture("known-regions.json"), &limits())
        .expect("fixture known fixture deve carregar")
}

fn unknown_observation() -> decalque_core::ScanObservation {
    let mut observation = known_observation();
    observation.page_mapping = Claim::Unknown {
        reason: UnknownReason::NotObserved,
        evidence: Vec::new(),
        detail: Some("fixture sem calibracao fisica".to_string()),
    };
    observation
}

#[test]
fn destination_none_retorna_relatorio_nao_publicado_sem_chamar_publisher() {
    let profile = derive_scan_layout(&known_observation()).expect("derive known deve funcionar");
    let rendered = render_scan_layout_profile(&profile);
    let artifact = rendered.artifact().expect("pagina known deve ter artefato");
    let mut recorder = Recorder::default();

    let report = finalize_scan_layout_profile(&rendered, None, &mut recorder)
        .expect("finalize sem destino deve ter sucesso");

    assert_eq!(report, rendered.unpublished_report());
    assert!(recorder.calls.is_empty());
    assert_eq!(artifact.source().as_bytes(), artifact.source_bytes());
    assert_eq!(artifact.size_bytes(), artifact.source_bytes().len());
}

#[test]
fn bundle_derived_publica_exatamente_uma_vez_e_so_entao_libera_relatorio_publicado() {
    let sandbox = Sandbox::new("derived");
    let destination = sandbox.root.join("result.typ");
    let sentinel = sandbox.root.join("render-must-not-touch.typ");
    fs::write(&sentinel, b"sentinel-before-render").unwrap();

    let profile = derive_scan_layout(&known_observation()).expect("derive known deve funcionar");
    let rendered = render_scan_layout_profile(&profile);
    assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel-before-render");

    let artifact = rendered.artifact().expect("pagina known deve ter artefato");
    let unpublished: Value = serde_json::from_str(rendered.unpublished_report()).unwrap();
    let published: Value = serde_json::from_str(
        rendered
            .published_report()
            .expect("relatorio publicado precomputado"),
    )
    .unwrap();
    let mut expected_published = unpublished.clone();
    expected_published["typst_artifact"]["published"] = Value::Bool(true);
    assert_eq!(published, expected_published);

    let mut recorder = Recorder::default();
    let report = finalize_scan_layout_profile(&rendered, Some(&destination), &mut recorder)
        .expect("recorder deve aceitar publicacao");

    assert_eq!(recorder.calls.len(), 1);
    assert_eq!(recorder.calls[0].0, destination);
    assert_eq!(recorder.calls[0].1, artifact.source_bytes());
    assert_eq!(report, rendered.published_report().unwrap());
    assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel-before-render");
}

#[test]
fn bundle_unknown_ignora_destino_e_nao_chama_publisher() {
    let sandbox = Sandbox::new("unknown");
    let destination = sandbox.root.join("must-not-exist.typ");
    let profile =
        derive_scan_layout(&unknown_observation()).expect("Unknown legitimo deve derivar");
    let rendered = render_scan_layout_profile(&profile);

    assert!(rendered.artifact().is_none());
    assert!(rendered.published_report().is_none());

    let mut recorder = Recorder::default();
    let report = finalize_scan_layout_profile(&rendered, Some(&destination), &mut recorder)
        .expect("Unknown e um resultado bem-sucedido");

    assert_eq!(report, rendered.unpublished_report());
    assert!(recorder.calls.is_empty());
    assert!(!destination.exists());

    let json: Value = serde_json::from_str(report).unwrap();
    assert_eq!(json["page"]["status"], "unknown");
    assert_eq!(json["page"]["reason"], "page-mapping-unknown");
    assert_eq!(json["typst_artifact"]["status"], "unavailable");
    assert_eq!(json["typst_artifact"]["published"], false);
}

#[test]
fn falha_do_publisher_preserva_bytes_oferecidos_e_vira_erro_publication_sem_source() {
    let sandbox = Sandbox::new("failure");
    let destination = sandbox.root.join("rejected.typ");
    let profile = derive_scan_layout(&known_observation()).expect("derive known deve funcionar");
    let rendered = render_scan_layout_profile(&profile);
    let artifact_bytes = rendered.artifact().unwrap().source_bytes().to_vec();
    let mut failer = Failer::default();

    let error = finalize_scan_layout_profile(&rendered, Some(&destination), &mut failer)
        .expect_err("publisher failer deve impedir sucesso");

    assert_eq!(failer.calls, vec![(destination.clone(), artifact_bytes)]);
    assert_eq!(error.kind(), ScanLayoutFinalizeErrorKind::Publication);
    assert_eq!(error.to_string(), "publisher-failed");
    assert!(error.source().is_none());
    assert!(!destination.exists());
}
