//! Oracle-first contract for the L3 Typst process, in-memory PDF, digest and
//! no-overwrite publication boundary used by candidate evaluation.
//!
//! The limits are deliberately supplied by the caller here. Production L2/L4
//! must use the closed v1 values, while this public L3 surface lets the oracle
//! exercise every budget without waiting 30 seconds or allocating 128 MiB.

use decalque_infra::{
    compile_typst_candidate, load_single_page_source_from_pdf_bytes, publish_new_file,
    TypstExecutionLimits,
};
use lopdf::{dictionary, Document, Object, Stream};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "decalque-typst-candidate-boundary-{}-{serial}-{label}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create isolated oracle directory");
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

fn evaluation_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../04_wiring/tests/fixtures/evaluation")
        .join(name)
}

fn existing_typst_pdf() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typst.pdf")
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(unix)]
fn install_fake_typst(sandbox: &Sandbox, name: &str) -> PathBuf {
    let executable = sandbox.path(name);
    fs::copy(evaluation_fixture("fake-typst"), &executable)
        .expect("copy controlled Typst executable");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap();
    executable
}

fn generous_limits() -> TypstExecutionLimits {
    TypstExecutionLimits {
        max_source_bytes: 1024 * 1024,
        max_pdf_stdout_bytes: 8 * 1024 * 1024,
        max_stderr_bytes: 4096,
        max_version_stdout_bytes: 4096,
        timeout: Duration::from_secs(10),
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn compile_error(executable: &Path, source: &[u8], limits: &TypstExecutionLimits) -> String {
    match compile_typst_candidate(executable, source, limits) {
        Ok(_) => panic!("controlled compiler failure unexpectedly succeeded"),
        Err(error) => error.to_string(),
    }
}

fn directory_entries(path: &Path) -> Vec<OsString> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn pdf_with_pages(page_count: usize) -> Vec<u8> {
    let mut document = Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let mut kids = Vec::with_capacity(page_count);

    for _ in 0..page_count {
        let content = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 72.into(), 72.into()],
            "Contents" => Object::Reference(content),
            "Resources" => dictionary! {},
        });
        kids.push(Object::Reference(page));
    }

    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => page_count as i64,
            "Kids" => kids,
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    document.trailer.set("Root", Object::Reference(catalog));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn real_typst_compile_is_byte_deterministic_and_reports_independent_hashes() {
    let source = concat!(
        "#set page(width: 72pt, height: 72pt, margin: 0pt)\n",
        "#place(top + left, dx: 9pt, dy: 18pt)[Decalque]\n",
    )
    .as_bytes();
    let limits = generous_limits();

    let first = compile_typst_candidate(Path::new("typst"), source, &limits)
        .expect("real supported Typst must compile");
    let second = compile_typst_candidate(Path::new("typst"), source, &limits)
        .expect("repeated real Typst compilation must compile");

    assert!(first.compiler_version.to_lowercase().contains("typst"));
    assert_eq!(first.compiler_version, second.compiler_version);
    assert_eq!(first.source_sha256, sha256(source));
    assert_eq!(first.source_size_bytes as usize, source.len());
    assert!(first.pdf_bytes.starts_with(b"%PDF-"));
    assert_eq!(first.pdf_sha256, sha256(&first.pdf_bytes));
    assert_eq!(first.pdf_size_bytes as usize, first.pdf_bytes.len());
    assert_eq!(first.pdf_bytes, second.pdf_bytes);
    assert_eq!(first.pdf_sha256, second.pdf_sha256);
    load_single_page_source_from_pdf_bytes(&first.pdf_bytes)
        .expect("real compilation must be one structurally readable page");
}

#[cfg(unix)]
#[test]
fn configured_executable_is_one_native_path_and_compile_arguments_are_fixed() {
    let sandbox = Sandbox::new("direct-process");
    let executable = install_fake_typst(&sandbox, "typst oracle; false");
    let sidecar = append_suffix(&executable, ".pdf");
    fs::copy(existing_typst_pdf(), &sidecar).unwrap();
    let source = b"#set page(width: 72pt, height: 72pt)\nDirect process";

    let compiled = compile_typst_candidate(&executable, source, &generous_limits())
        .expect("metacharacters must remain part of one executable path");

    assert_eq!(compiled.compiler_version, "typst-oracle 0.15.1");
    assert_eq!(compiled.source_sha256, sha256(source));
    assert_eq!(compiled.pdf_bytes, fs::read(sidecar).unwrap());
    load_single_page_source_from_pdf_bytes(&compiled.pdf_bytes).unwrap();
}

#[cfg(unix)]
#[test]
fn nonzero_version_nonzero_compile_and_empty_pdf_are_distinct_failures() {
    let sandbox = Sandbox::new("process-status");
    let limits = generous_limits();

    let version = install_fake_typst(&sandbox, "version-nonzero");
    let version_error = compile_error(&version, b"source", &limits);
    assert!(version_error.to_lowercase().contains("version"));
    assert!(version_error.contains("oracle-version-status"));

    let compile = install_fake_typst(&sandbox, "compile-nonzero");
    let compile_status_error = compile_error(&compile, b"source", &limits);
    assert!(compile_status_error.to_lowercase().contains("compil"));
    assert!(compile_status_error.contains("oracle-compile-status"));

    let empty = install_fake_typst(&sandbox, "empty-pdf");
    let empty_error = compile_error(&empty, b"source", &limits).to_lowercase();
    assert!(empty_error.contains("pdf") || empty_error.contains("stdout"));
    assert!(empty_error.contains("empty") || empty_error.contains("vazio"));
}

#[cfg(unix)]
#[test]
fn timeout_terminates_and_reaps_the_compiler_within_the_configured_budget() {
    let sandbox = Sandbox::new("timeout");
    let executable = install_fake_typst(&sandbox, "timeout");
    let mut limits = generous_limits();
    limits.timeout = Duration::from_millis(120);
    let started = Instant::now();

    let error = compile_error(&executable, b"source", &limits);

    assert!(started.elapsed() < Duration::from_secs(2), "{error}");
    let error = error.to_lowercase();
    assert!(
        error.contains("timeout") || error.contains("tempo"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn version_pdf_and_stderr_overflow_fail_early_with_bounded_diagnostics() {
    let sandbox = Sandbox::new("output-limits");
    let mut limits = generous_limits();
    limits.max_pdf_stdout_bytes = 64;
    limits.max_stderr_bytes = 64;
    limits.max_version_stdout_bytes = 64;
    limits.timeout = Duration::from_secs(4);

    for (mode, stage) in [
        ("version-overflow", "version"),
        ("pdf-overflow", "pdf"),
        ("stderr-overflow", "stderr"),
    ] {
        let executable = install_fake_typst(&sandbox, mode);
        let started = Instant::now();
        let error = compile_error(&executable, b"source", &limits);

        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{mode} exhausted the timeout instead of its byte budget: {error}"
        );
        let lower = error.to_lowercase();
        assert!(lower.contains(stage), "{mode}: {error}");
        assert!(
            lower.contains("limit")
                || lower.contains("limite")
                || lower.contains("exceed")
                || lower.contains("maximum"),
            "{mode}: {error}"
        );
        assert!(error.len() <= 4096, "unbounded diagnostic for {mode}");
    }
}

#[cfg(unix)]
#[test]
fn oversized_source_is_rejected_before_the_version_process_is_spawned() {
    let sandbox = Sandbox::new("source-limit");
    let executable = install_fake_typst(&sandbox, "source-limit-must-not-run");
    let invoked = append_suffix(&executable, ".invoked");
    let mut limits = generous_limits();
    limits.max_source_bytes = 8;

    let error = compile_error(&executable, b"nine bytes!", &limits).to_lowercase();

    assert!(
        !invoked.exists(),
        "compiler ran before source budget rejection"
    );
    assert!(error.contains("source") || error.contains("fonte"));
    assert!(error.contains("limit") || error.contains("limite") || error.contains("maximum"));
}

#[test]
fn invalid_and_multi_page_pdf_bytes_are_rejected_by_the_memory_adapter() {
    let invalid = b"%PDF-1.7\nthis is not a document";
    assert!(load_single_page_source_from_pdf_bytes(invalid).is_err());

    let two_pages = pdf_with_pages(2);
    let parsed = Document::load_mem(&two_pages).expect("oracle must construct a valid PDF");
    assert_eq!(parsed.get_pages().len(), 2);
    assert!(load_single_page_source_from_pdf_bytes(&two_pages).is_err());
}

#[test]
fn existing_destination_is_unchanged_and_leaves_no_temporary_file() {
    let sandbox = Sandbox::new("existing-destination");
    let destination = sandbox.path("candidate.pdf");
    fs::write(&destination, b"pre-existing sentinel").unwrap();
    let before = directory_entries(&sandbox.root);

    let result = publish_new_file(&destination, b"replacement bytes");

    assert!(result.is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"pre-existing sentinel");
    assert_eq!(directory_entries(&sandbox.root), before);
}

#[test]
fn racing_publications_have_exactly_one_winner_and_never_overwrite() {
    let sandbox = Sandbox::new("publication-race");
    let destination = sandbox.path("candidate.pdf");
    let first_payload = Arc::new(vec![b'A'; 256 * 1024]);
    let second_payload = Arc::new(vec![b'B'; 256 * 1024]);
    let barrier = Arc::new(Barrier::new(3));

    let first = {
        let destination = destination.clone();
        let payload = Arc::clone(&first_payload);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            publish_new_file(&destination, payload.as_slice()).is_ok()
        })
    };
    let second = {
        let destination = destination.clone();
        let payload = Arc::clone(&second_payload);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            publish_new_file(&destination, payload.as_slice()).is_ok()
        })
    };

    barrier.wait();
    let outcomes = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(outcomes.into_iter().filter(|won| *won).count(), 1);

    let published = fs::read(&destination).unwrap();
    assert!(published == *first_payload || published == *second_payload);
    assert_eq!(
        directory_entries(&sandbox.root),
        vec![OsStr::new("candidate.pdf").to_os_string()]
    );
}
