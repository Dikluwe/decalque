//! Fronteira limitada para compilar e publicar um candidato Typst.

use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;

static NEXT_TEMPORARY_FILE: AtomicU64 = AtomicU64::new(0);
const TERMINATION_GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypstExecutionLimits {
    pub max_source_bytes: usize,
    pub max_pdf_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_version_stdout_bytes: usize,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypstCompilation {
    pub compiler_version: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub pdf_bytes: Vec<u8>,
    pub pdf_sha256: String,
    pub pdf_size_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct IdentifiedTypstCompiler {
    executable: PathBuf,
    limits: TypstExecutionLimits,
    compiler_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypstExecutionError {
    message: String,
}

impl TypstExecutionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for TypstExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypstExecutionError {}

#[derive(Debug)]
pub enum PublishNewFileError {
    InvalidDestination {
        destination: PathBuf,
    },
    TemporaryCreation {
        destination: PathBuf,
        source: io::Error,
    },
    TemporaryWrite {
        source: io::Error,
    },
    TemporarySync {
        source: io::Error,
    },
    DestinationPublication {
        destination: PathBuf,
        source: io::Error,
    },
    TemporaryCleanup {
        source: io::Error,
    },
    CleanupAfterFailure {
        operation: Box<PublishNewFileError>,
        cleanup: Box<PublishNewFileError>,
    },
}

impl fmt::Display for PublishNewFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDestination { destination } => write!(
                formatter,
                "publication destination `{}` has no file name",
                destination.display()
            ),
            Self::TemporaryCreation {
                destination,
                source,
            } => write!(
                formatter,
                "failed to create an exclusive temporary file beside `{}`: {source}",
                destination.display()
            ),
            Self::TemporaryWrite { source } => {
                write!(
                    formatter,
                    "failed to write temporary publication file: {source}"
                )
            }
            Self::TemporarySync { source } => {
                write!(
                    formatter,
                    "failed to sync temporary publication file: {source}"
                )
            }
            Self::DestinationPublication {
                destination,
                source,
            } => write!(
                formatter,
                "failed to publish new file at `{}` without overwriting: {source}",
                destination.display()
            ),
            Self::TemporaryCleanup { source } => write!(
                formatter,
                "failed to remove temporary publication file: {source}"
            ),
            Self::CleanupAfterFailure { operation, cleanup } => {
                write!(formatter, "{operation}; cleanup also failed: {cleanup}")
            }
        }
    }
}

impl std::error::Error for PublishNewFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDestination { .. } => None,
            Self::TemporaryCreation { source, .. }
            | Self::TemporaryWrite { source }
            | Self::TemporarySync { source }
            | Self::DestinationPublication { source, .. }
            | Self::TemporaryCleanup { source } => Some(source),
            Self::CleanupAfterFailure { operation, .. } => Some(operation.as_ref()),
        }
    }
}

/// Identifica o compilador em uma única invocação direta e limitada.
pub fn identify_typst_compiler(
    executable: &Path,
    limits: &TypstExecutionLimits,
) -> Result<IdentifiedTypstCompiler, TypstExecutionError> {
    let version_output = run_invocation(
        executable,
        &["--version"],
        None,
        limits.max_version_stdout_bytes,
        limits.max_stderr_bytes,
        limits.timeout,
        InvocationStage::Version,
    )?;
    ensure_success(InvocationStage::Version, &version_output)?;
    let compiler_version = parse_compiler_version(version_output.stdout)?;

    Ok(IdentifiedTypstCompiler {
        executable: executable.to_path_buf(),
        limits: limits.clone(),
        compiler_version,
    })
}

impl IdentifiedTypstCompiler {
    pub fn compiler_version(&self) -> &str {
        &self.compiler_version
    }

    /// Compila uma fonte com o executável, a versão e os limites identificados.
    pub fn compile(&self, source: &[u8]) -> Result<TypstCompilation, TypstExecutionError> {
        validate_source_size(source, &self.limits)?;
        self.compile_validated(source)
    }

    fn compile_validated(&self, source: &[u8]) -> Result<TypstCompilation, TypstExecutionError> {
        let compilation_output = run_invocation(
            &self.executable,
            &[
                "compile",
                "--format",
                "pdf",
                "--creation-timestamp",
                "0",
                "-",
                "-",
            ],
            Some(source),
            self.limits.max_pdf_stdout_bytes,
            self.limits.max_stderr_bytes,
            self.limits.timeout,
            InvocationStage::Compilation,
        )?;
        ensure_success(InvocationStage::Compilation, &compilation_output)?;
        if compilation_output.stdout.is_empty() {
            return Err(TypstExecutionError::new(
                "Typst compilation produced empty PDF stdout",
            ));
        }

        let pdf_bytes = compilation_output.stdout;
        Ok(TypstCompilation {
            compiler_version: self.compiler_version.clone(),
            source_sha256: sha256_hex(source),
            source_size_bytes: source.len() as u64,
            pdf_sha256: sha256_hex(&pdf_bytes),
            pdf_size_bytes: pdf_bytes.len() as u64,
            pdf_bytes,
        })
    }
}

/// Obtém a versão do compilador e compila a fonte em duas invocações
/// independentes, diretas e limitadas.
pub fn compile_typst_candidate(
    executable: &Path,
    source: &[u8],
    limits: &TypstExecutionLimits,
) -> Result<TypstCompilation, TypstExecutionError> {
    validate_source_size(source, limits)?;
    identify_typst_compiler(executable, limits)?.compile_validated(source)
}

fn validate_source_size(
    source: &[u8],
    limits: &TypstExecutionLimits,
) -> Result<(), TypstExecutionError> {
    if source.len() > limits.max_source_bytes {
        return Err(TypstExecutionError::new(format!(
            "Typst source exceeds the limit of {} bytes (received {} bytes)",
            limits.max_source_bytes,
            source.len()
        )));
    }
    Ok(())
}

/// Publica bytes em um caminho novo sem janela de sobrescrita.
///
/// O arquivo temporário é criado, escrito e sincronizado no diretório do
/// destino. Uma única renomeação atômica `no-replace` consome o nome
/// temporário somente se o nome final ainda não existir.
pub fn publish_new_file(destination: &Path, bytes: &[u8]) -> Result<(), PublishNewFileError> {
    let mut temporary = TemporaryPublication::create(destination)?;

    if let Err(error) = temporary.write_all(bytes) {
        let error = PublishNewFileError::TemporaryWrite { source: error };
        return Err(temporary.abort(error));
    }
    if let Err(error) = temporary.sync_all() {
        let error = PublishNewFileError::TemporarySync { source: error };
        return Err(temporary.abort(error));
    }
    temporary.close();

    if let Err(error) = rename_no_replace(temporary.path(), destination) {
        let error = PublishNewFileError::DestinationPublication {
            destination: destination.to_path_buf(),
            source: error,
        };
        return Err(temporary.abort(error));
    }

    temporary.mark_published();
    Ok(())
}

#[cfg(target_os = "linux")]
fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;

    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "temporary publication path contains a NUL byte",
        )
    })?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "publication destination contains a NUL byte",
        )
    })?;

    // SAFETY: both pointers come from live `CString` values and remain valid
    // for the duration of the syscall. Both paths are in the same directory.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    // Windows rename fails rather than replacing an existing destination.
    fs::rename(source, destination)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn rename_no_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace publication is not supported on this platform",
    ))
}

#[derive(Clone, Copy, Debug)]
enum InvocationStage {
    Version,
    Compilation,
}

impl InvocationStage {
    fn label(self) -> &'static str {
        match self {
            Self::Version => "Typst version invocation",
            Self::Compilation => "Typst compilation",
        }
    }

    fn stdout_label(self) -> &'static str {
        match self {
            Self::Version => "version stdout",
            Self::Compilation => "PDF stdout",
        }
    }

    fn stderr_label(self) -> &'static str {
        match self {
            Self::Version => "version stderr",
            Self::Compilation => "compilation stderr",
        }
    }
}

struct InvocationOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Clone, Debug)]
struct WorkerFailure {
    message: String,
}

impl WorkerFailure {
    fn limit(stream: &str, maximum: usize) -> Self {
        Self {
            message: format!("{stream} exceeded the limit of {maximum} bytes"),
        }
    }

    fn io(action: &str, error: io::Error) -> Self {
        Self {
            message: format!("{action}: {error}"),
        }
    }
}

type ReaderWorker = JoinHandle<Result<Vec<u8>, WorkerFailure>>;
type WriterWorker = JoinHandle<Result<(), WorkerFailure>>;

struct InvocationWorkers {
    stdout: ReaderWorker,
    stderr: ReaderWorker,
    stdin: Option<WriterWorker>,
}

impl InvocationWorkers {
    fn all_finished(&self) -> bool {
        self.stdout.is_finished()
            && self.stderr.is_finished()
            && self.stdin.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

fn run_invocation(
    executable: &Path,
    arguments: &[&str],
    stdin_bytes: Option<&[u8]>,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    stage: InvocationStage,
) -> Result<InvocationOutput, TypstExecutionError> {
    let started = Instant::now();
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(if stdin_bytes.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = spawn_direct(&mut command, stage, timeout, started)?;

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => return Err(cleanup_after_setup_failure(&mut child, stage, "stdout")),
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => return Err(cleanup_after_setup_failure(&mut child, stage, "stderr")),
    };
    let stdin = match stdin_bytes {
        Some(_) => match child.stdin.take() {
            Some(stdin) => Some(stdin),
            None => return Err(cleanup_after_setup_failure(&mut child, stage, "stdin")),
        },
        None => None,
    };

    let (alert_sender, alert_receiver) = mpsc::channel();
    let workers = InvocationWorkers {
        stdout: spawn_reader(
            stdout,
            stdout_limit,
            stage.stdout_label(),
            alert_sender.clone(),
        ),
        stderr: spawn_reader(
            stderr,
            stderr_limit,
            stage.stderr_label(),
            alert_sender.clone(),
        ),
        stdin: stdin
            .zip(stdin_bytes)
            .map(|(stdin, bytes)| spawn_writer(stdin, bytes.to_vec())),
    };
    drop(alert_sender);

    let mut status = None;
    loop {
        if let Ok(failure) = alert_receiver.try_recv() {
            let cleanup = terminate_and_drain(&mut child, Some(workers));
            return Err(worker_execution_error(stage, failure, cleanup));
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit_status)) => status = Some(exit_status),
                Ok(None) => {}
                Err(error) => {
                    let cleanup = terminate_and_drain(&mut child, Some(workers));
                    return Err(process_execution_error(
                        stage,
                        format!("failed while waiting for process: {error}"),
                        cleanup,
                    ));
                }
            }
        }

        if status.is_some() && workers.all_finished() {
            break;
        }

        if started.elapsed() >= timeout {
            let cleanup = terminate_and_drain(&mut child, Some(workers));
            return Err(process_execution_error(
                stage,
                format!("timeout after {} milliseconds", timeout.as_millis()),
                cleanup,
            ));
        }

        let remaining = timeout.saturating_sub(started.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(2)));
    }

    let status = status.expect("the direct child exited before every worker finished");
    let stdout = join_worker(workers.stdout, stage, stage.stdout_label())?;
    let stderr = join_worker(workers.stderr, stage, stage.stderr_label())?;
    if let Some(handle) = workers.stdin {
        let writer_result = join_writer(handle, stage);
        if status.success() {
            writer_result?;
        }
    }

    Ok(InvocationOutput {
        status,
        stdout,
        stderr,
    })
}

fn spawn_direct(
    command: &mut Command,
    stage: InvocationStage,
    timeout: Duration,
    started: Instant,
) -> Result<Child, TypstExecutionError> {
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy => {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Err(TypstExecutionError::new(format!(
                        "{} timed out while waiting for its executable to become available",
                        stage.label()
                    )));
                }
                thread::sleep(remaining.min(Duration::from_millis(2)));
            }
            Err(error) => {
                return Err(TypstExecutionError::new(format!(
                    "failed to spawn {}: {error}",
                    stage.label()
                )));
            }
        }
    }
}

fn spawn_reader<R>(
    reader: R,
    maximum: usize,
    stream: &'static str,
    alert_sender: Sender<WorkerFailure>,
) -> JoinHandle<Result<Vec<u8>, WorkerFailure>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let result = read_bounded(reader, maximum, stream);
        if let Err(failure) = &result {
            let _ = alert_sender.send(failure.clone());
        }
        result
    })
}

fn spawn_writer<W>(mut writer: W, bytes: Vec<u8>) -> JoinHandle<Result<(), WorkerFailure>>
where
    W: Write + Send + 'static,
{
    thread::spawn(move || {
        writer
            .write_all(&bytes)
            .and_then(|()| writer.flush())
            .map_err(|error| WorkerFailure::io("failed to write Typst source to stdin", error))
    })
}

fn read_bounded<R>(mut reader: R, maximum: usize, stream: &str) -> Result<Vec<u8>, WorkerFailure>
where
    R: Read,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(WorkerFailure::io(
                    &format!("failed to read {stream}"),
                    error,
                ));
            }
        };
        if read == 0 {
            return Ok(output);
        }
        if read > maximum.saturating_sub(output.len()) {
            return Err(WorkerFailure::limit(stream, maximum));
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

fn join_worker(
    handle: JoinHandle<Result<Vec<u8>, WorkerFailure>>,
    stage: InvocationStage,
    worker: &str,
) -> Result<Vec<u8>, TypstExecutionError> {
    match handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(failure)) => Err(TypstExecutionError::new(format!(
            "{} failed: {}",
            stage.label(),
            failure.message
        ))),
        Err(_) => Err(TypstExecutionError::new(format!(
            "{} {worker} worker panicked",
            stage.label()
        ))),
    }
}

fn join_writer(
    handle: JoinHandle<Result<(), WorkerFailure>>,
    stage: InvocationStage,
) -> Result<(), TypstExecutionError> {
    match handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(failure)) => Err(TypstExecutionError::new(format!(
            "{} failed: {}",
            stage.label(),
            failure.message
        ))),
        Err(_) => Err(TypstExecutionError::new(format!(
            "{} stdin worker panicked",
            stage.label()
        ))),
    }
}

fn cleanup_after_setup_failure(
    child: &mut Child,
    stage: InvocationStage,
    pipe: &str,
) -> TypstExecutionError {
    let cleanup = terminate_and_drain(child, None);
    process_execution_error(
        stage,
        format!("spawned process did not expose its configured {pipe} pipe"),
        cleanup,
    )
}

fn terminate_and_drain(
    child: &mut Child,
    workers: Option<InvocationWorkers>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    if let Err(error) = terminate_process_tree(child) {
        failures.push(format!("failed to terminate process group: {error}"));
    }

    let cleanup_started = Instant::now();
    let mut child_reaped = false;
    let mut child_wait_failed = false;
    loop {
        if !child_reaped && !child_wait_failed {
            match child.try_wait() {
                Ok(Some(_)) => child_reaped = true,
                Ok(None) => {}
                Err(error) => {
                    failures.push(format!("failed to reap direct process: {error}"));
                    child_wait_failed = true;
                }
            }
        }

        let workers_finished = workers.as_ref().is_none_or(InvocationWorkers::all_finished);
        if (child_reaped || child_wait_failed) && workers_finished {
            break;
        }

        if cleanup_started.elapsed() >= TERMINATION_GRACE {
            break;
        }
        let remaining = TERMINATION_GRACE.saturating_sub(cleanup_started.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(2)));
    }

    if !child_reaped && !child_wait_failed {
        failures.push(format!(
            "direct process was not reaped within {} milliseconds",
            TERMINATION_GRACE.as_millis()
        ));
    }

    if let Some(workers) = workers {
        discard_worker(workers.stdout, "stdout", &mut failures);
        discard_worker(workers.stderr, "stderr", &mut failures);
        if let Some(stdin) = workers.stdin {
            discard_worker(stdin, "stdin", &mut failures);
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn discard_worker<T>(worker: JoinHandle<T>, name: &str, failures: &mut Vec<String>) {
    if !worker.is_finished() {
        failures.push(format!(
            "{name} worker did not stop within {} milliseconds",
            TERMINATION_GRACE.as_millis()
        ));
        return;
    }
    if worker.join().is_err() {
        failures.push(format!("{name} worker panicked during cleanup"));
    }
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child) -> io::Result<()> {
    let process_group = libc::pid_t::try_from(child.id()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "child process identifier does not fit the platform pid type",
        )
    })?;

    // SAFETY: every invocation is spawned as leader of the process group whose
    // id equals the direct child's pid. A negative id addresses that group.
    let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
    if result == 0 {
        // The direct executable could have moved itself out of the configured
        // group. `Child::kill` is pidfd-backed on supported Linux systems and
        // is a no-op after `try_wait` has cached an exit status.
        let _ = child.kill();
        return Ok(());
    }

    let group_error = io::Error::last_os_error();
    if group_error.raw_os_error() == Some(libc::ESRCH) {
        return match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => child.kill(),
            Err(error) => Err(error),
        };
    }

    let _ = child.kill();
    Err(group_error)
}

#[cfg(not(unix))]
fn terminate_process_tree(child: &mut Child) -> io::Result<()> {
    match child.try_wait()? {
        Some(_) => Ok(()),
        None => child.kill(),
    }
}

fn worker_execution_error(
    stage: InvocationStage,
    failure: WorkerFailure,
    cleanup: Result<(), String>,
) -> TypstExecutionError {
    process_execution_error(stage, failure.message, cleanup)
}

fn process_execution_error(
    stage: InvocationStage,
    detail: String,
    cleanup: Result<(), String>,
) -> TypstExecutionError {
    let mut message = format!("{} failed: {detail}", stage.label());
    if let Err(cleanup) = cleanup {
        write!(message, "; {cleanup}").expect("writing to a String cannot fail");
    }
    TypstExecutionError::new(message)
}

fn ensure_success(
    stage: InvocationStage,
    output: &InvocationOutput,
) -> Result<(), TypstExecutionError> {
    if output.status.success() {
        return Ok(());
    }

    let mut message = format!("{} exited with status {}", stage.label(), output.status);
    let stderr = sanitize_stderr(&output.stderr);
    if !stderr.is_empty() {
        write!(message, "; stderr: {stderr}").expect("writing to a String cannot fail");
    }
    Err(TypstExecutionError::new(message))
}

fn parse_compiler_version(stdout: Vec<u8>) -> Result<String, TypstExecutionError> {
    let version = String::from_utf8(stdout)
        .map_err(|_| TypstExecutionError::new("Typst version stdout is not valid UTF-8 text"))?;
    let version = version.trim();
    if version.is_empty() {
        return Err(TypstExecutionError::new("Typst version stdout is empty"));
    }
    if version.chars().any(char::is_control) {
        return Err(TypstExecutionError::new(
            "Typst version stdout contains control characters",
        ));
    }
    Ok(version.to_string())
}

fn sanitize_stderr(stderr: &[u8]) -> String {
    const MAX_DIAGNOSTIC_CHARS: usize = 4096;

    let mut output = String::new();
    let mut truncated = false;
    for (characters, character) in String::from_utf8_lossy(stderr).trim().chars().enumerate() {
        if characters >= MAX_DIAGNOSTIC_CHARS {
            truncated = true;
            break;
        }
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push('\u{fffd}'),
            character => output.push(character),
        }
    }
    if truncated {
        output.push_str("…[truncated]");
    }
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

struct TemporaryPublication {
    path: Option<PathBuf>,
    file: Option<File>,
}

impl TemporaryPublication {
    fn create(destination: &Path) -> Result<Self, PublishNewFileError> {
        let file_name =
            destination
                .file_name()
                .ok_or_else(|| PublishNewFileError::InvalidDestination {
                    destination: destination.to_path_buf(),
                })?;
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));

        for _ in 0..128 {
            let serial = NEXT_TEMPORARY_FILE.fetch_add(1, Ordering::Relaxed);
            let mut temporary_name = OsString::from(".");
            temporary_name.push(file_name);
            temporary_name.push(format!(".decalque-tmp-{}-{serial}", std::process::id()));
            let path = parent.join(temporary_name);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path: Some(path),
                        file: Some(file),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(PublishNewFileError::TemporaryCreation {
                        destination: destination.to_path_buf(),
                        source: error,
                    });
                }
            }
        }

        Err(PublishNewFileError::TemporaryCreation {
            destination: destination.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "failed to allocate an exclusive temporary publication file after 128 attempts",
            ),
        })
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("temporary publication path is present until cleanup")
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file
            .as_mut()
            .expect("temporary publication file is open before publication")
            .write_all(bytes)
    }

    fn sync_all(&self) -> io::Result<()> {
        self.file
            .as_ref()
            .expect("temporary publication file is open before publication")
            .sync_all()
    }

    fn close(&mut self) {
        self.file.take();
    }

    fn mark_published(mut self) {
        self.file.take();
        self.path.take();
    }

    fn remove(mut self) -> Result<(), PublishNewFileError> {
        self.close();
        let path = self
            .path
            .take()
            .expect("temporary publication path is present until cleanup");
        fs::remove_file(&path).map_err(|error| {
            self.path = Some(path);
            PublishNewFileError::TemporaryCleanup { source: error }
        })
    }

    fn abort(self, primary: PublishNewFileError) -> PublishNewFileError {
        match self.remove() {
            Ok(()) => primary,
            Err(cleanup) => PublishNewFileError::CleanupAfterFailure {
                operation: Box::new(primary),
                cleanup: Box::new(cleanup),
            },
        }
    }
}

impl Drop for TemporaryPublication {
    fn drop(&mut self) {
        self.file.take();
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}
