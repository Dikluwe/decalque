//! Políticas de apresentação e configuração do Decalque (L2).
//!
//! Este crate materializa somente decisões já fechadas pelos prompts do
//! núcleo, incluindo políticas, limites e apresentação das CLIs.

mod cli;
mod policy;
mod scan_evaluation;
mod scan_font_attestation;
mod scan_layout;
mod scan_observation;
mod scan_reconstruction;
mod scan_report;
mod scan_typography_search;
mod scan_validation;

pub use cli::{parse_args, render_report, CliArgs, CliParseError, USAGE};
pub use policy::{digital_to_digital_resolution, ReportMetrics};
pub use scan_evaluation::{
    parse_scan_evaluation_args, render_scan_evaluation_report, ScanEvaluationCliArgs,
    ScanEvaluationCliParseError, ScanEvaluationLimits, ScanEvaluationReportContext,
    ScanEvaluationReportRenderError, SCAN_EVALUATION_LIMITS, SCAN_EVALUATION_MAX_PDF_BYTES,
    SCAN_EVALUATION_MAX_SOURCE_BYTES, SCAN_EVALUATION_MAX_STDERR_BYTES,
    SCAN_EVALUATION_MAX_VERSION_BYTES, SCAN_EVALUATION_TIMEOUT, SCAN_EVALUATION_USAGE,
};
pub use scan_font_attestation::{
    parse_scan_font_attestation_args, render_scan_font_attestation_report,
    ScanFontAttestationCliArgs, ScanFontAttestationCliParseError,
    ScanFontAttestationDiagnosticContext, ScanFontAttestationEvidenceContext,
    ScanFontAttestationReportContext, ScanFontAttestationReportRenderError,
    ScanFontAttestationResourceContext, SCAN_FONT_ATTESTATION_LIMITS, SCAN_FONT_ATTESTATION_USAGE,
};
pub use scan_layout::{
    render_scan_layout_profile, RenderedScanLayoutProfile, RenderedScanLayoutTypst,
};
pub use scan_observation::{
    parse_scan_observation_args, ScanObservationCliArgs, ScanObservationCliParseError,
    ScanObservationInputLimits, SCAN_OBSERVATION_INPUT_LIMITS, SCAN_OBSERVATION_USAGE,
};
pub use scan_reconstruction::{
    parse_scan_reconstruction_args, ScanReconstructionCliArgs, ScanReconstructionCliParseError,
    SCAN_RECONSTRUCTION_USAGE,
};
pub use scan_report::{
    render_scan_comparison_report, ScanReportContext, ScanReportDiagnostic, ScanReportRenderError,
};
pub use scan_typography_search::{
    parse_scan_typography_search_args, render_scan_typography_search_report,
    ScanTypographySearchCliArgs, ScanTypographySearchCliParseError,
    ScanTypographySearchCoverageContext, ScanTypographySearchEligibilityContext,
    ScanTypographySearchReportContext, ScanTypographySearchReportRenderError,
    ScanTypographySearchScoreContext, ScanTypographySearchSearchSpaceContext,
    ScanTypographySearchSelectionContext, ScanTypographySearchSupportContext,
    ScanTypographySearchTrialContext, SCAN_TYPOGRAPHY_SEARCH_MAX_HYPOTHESES,
    SCAN_TYPOGRAPHY_SEARCH_USAGE,
};
pub use scan_validation::{
    parse_scan_observation_validation_args, render_scan_observation_validation_report,
    ScanObservationValidationCliArgs, ScanObservationValidationCliParseError,
    SCAN_OBSERVATION_RASTER_MAX_BYTES, SCAN_OBSERVATION_VALIDATION_USAGE,
};
