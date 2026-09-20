use decalque_shell::RenderedScanLayoutProfile;
use std::error::Error;
use std::fmt;
use std::path::Path;

pub trait ScanLayoutPublisher {
    type Error: Error + Send + Sync + 'static;

    fn publish_new_file(&mut self, destination: &Path, bytes: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLayoutFinalizeErrorKind {
    Publication,
}

#[derive(Debug)]
pub struct ScanLayoutFinalizeError {
    message: String,
}

impl ScanLayoutFinalizeError {
    pub fn kind(&self) -> ScanLayoutFinalizeErrorKind {
        ScanLayoutFinalizeErrorKind::Publication
    }
}

pub fn finalize_scan_layout_profile<'a, P: ScanLayoutPublisher>(
    rendered: &'a RenderedScanLayoutProfile<'_>,
    destination: Option<&Path>,
    publisher: &mut P,
) -> Result<&'a str, ScanLayoutFinalizeError> {
    let Some(destination) = destination else {
        return Ok(rendered.unpublished_report());
    };
    let Some(artifact) = rendered.artifact() else {
        return Ok(rendered.unpublished_report());
    };

    publisher
        .publish_new_file(destination, artifact.source_bytes())
        .map_err(|error| ScanLayoutFinalizeError {
            message: error.to_string(),
        })?;
    Ok(rendered
        .published_report()
        .unwrap_or_else(|| rendered.unpublished_report()))
}

impl fmt::Display for ScanLayoutFinalizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ScanLayoutFinalizeError {}
