//! Motores puros para comparação digital e entre observação de scan e candidato.
//! Motor de emparelhamento e comparação (L1).

pub mod compare;
pub mod scan_compare;
pub mod scan_font_attestation;
pub mod scan_reconstruct;
pub mod scan_typography_fit;

pub use compare::{compare, ClusterShift, ComparisonReport, Coverage, GlyphPair};
pub use scan_compare::{
    compare_scan_observation, CandidateUnitRef, ConfidenceRequirement, EvidenceStatus,
    GeometryComponent, ReflowWitness, ScanComparisonDiagnostic, ScanComparisonPolicy,
    ScanComparisonReport, ScanCoverage, ScanGranularity, ScanMatch, TextNormalization,
};
pub use scan_font_attestation::{
    attest_scan_font, PdfFontResource, ScanFontAttestationDiagnostic, ScanFontAttestationInput,
    ScanFontAttestationReport, ScanFontAttestationResource, ScanFontFace,
};
pub use scan_reconstruct::plan_scan_lines;
pub use scan_typography_fit::{
    evaluate_scan_typography_fit, ScanTypographyEligibility, ScanTypographyEvidenceScope,
    ScanTypographyFitError, ScanTypographyFitEvaluation, ScanTypographyFitKey,
    ScanTypographyInconclusiveReason, ScanTypographyIneligibilityReason, ScanTypographyResidual,
    ScanTypographySelection, ScanTypographySupport, ScanTypographyTrialEvaluation,
};
