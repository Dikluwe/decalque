//! Entidades puras para o planejamento geométrico de linhas observadas.

use std::fmt;

use super::scan_observation::{
    Claim, FramedBbox, FramedPolyline, PageFrame, ScanObservationValidationError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeightHypothesis {
    Regular,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyleHypothesis {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypographyHypothesis {
    pub font_family: String,
    pub size_pt: f64,
    pub weight: FontWeightHypothesis,
    pub style: FontStyleHypothesis,
    pub tracking_pt: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionCoverage {
    pub planned_lines: usize,
    pub total_lines: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructionLine {
    pub source_unit_id: String,
    pub reading_key: Vec<u32>,
    pub text: String,
    pub target_bbox: FramedBbox,
    pub target_baseline: Claim<FramedPolyline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructionPlan {
    pub page: PageFrame,
    pub mapping_max_error_pt: f64,
    pub typography: TypographyHypothesis,
    pub lines: Vec<ReconstructionLine>,
    pub coverage: ReconstructionCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReconstructionClaimKind {
    Text,
    Bbox,
    ReadingOrder,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionDiagnostic {
    PageMappingUnknown,
    NoLines,
    RequiredClaimUnknown {
        unit_id: String,
        claim: ReconstructionClaimKind,
    },
    ExplicitLineSeparator {
        unit_id: String,
        scalar_index: usize,
        code_point: u32,
    },
    ProjectionFailed {
        unit_id: String,
        claim: ReconstructionClaimKind,
    },
    AmbiguousReadingOrder {
        unit_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionUnknownReport {
    pub coverage: ReconstructionCoverage,
    pub diagnostics: Vec<ReconstructionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReconstructionOutcome {
    Materializable(ReconstructionPlan),
    Unknown(ReconstructionUnknownReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionInputError {
    EmptyFontFamily,
    InvalidFontSizePt,
    InvalidTrackingPt,
    InvalidObservation(ScanObservationValidationError),
}

impl fmt::Display for ReconstructionInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFontFamily => formatter.write_str("font family must not be empty"),
            Self::InvalidFontSizePt => {
                formatter.write_str("font size must be finite and strictly positive")
            }
            Self::InvalidTrackingPt => formatter.write_str("tracking must be finite"),
            Self::InvalidObservation(error) => write!(formatter, "invalid observation: {error}"),
        }
    }
}

impl std::error::Error for ReconstructionInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidObservation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ScanObservationValidationError> for ReconstructionInputError {
    fn from(error: ScanObservationValidationError) -> Self {
        Self::InvalidObservation(error)
    }
}
