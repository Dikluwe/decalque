//! Entidades do domínio (L1): geometria de página, resolução de medição,
//! instâncias de glifo e observacoes OCR externas.

pub mod document_geometry;
pub mod glyph_instance;
pub mod measurement_resolution;
pub mod page_diagnostic;
pub mod page_geometry;
pub mod pdf_error;
pub mod reconstruction;
pub mod scan_observation;

pub use document_geometry::DocumentGeometry;
pub use glyph_instance::{GlyphInstance, TextMappingStatus};
pub use measurement_resolution::MeasurementResolution;
pub use page_diagnostic::{diagnose_page, PageDiagnostic};
pub use page_geometry::{
    display_size, resolve_page_geometry, PageBoxModel, PageGeometry, PageGeometryDiagnostic,
    PageRotation, Rect,
};
pub use pdf_error::PdfError;
pub use reconstruction::{
    FontStyleHypothesis, FontWeightHypothesis, ReconstructionClaimKind, ReconstructionCoverage,
    ReconstructionDiagnostic, ReconstructionInputError, ReconstructionLine, ReconstructionOutcome,
    ReconstructionPlan, ReconstructionUnknownReport, TypographyHypothesis,
};
pub use scan_observation::{
    validate_scan_observation, Claim, Confidence, EvidenceBasis, FramedBbox, FramedPolygon,
    FramedPolyline, GeometryClaims, ObservationKind, ObservationUnit, PageFrame, PageMapping,
    PageMappingKind, ProducerIdentity, ProvenanceId, ProvenanceRecord, ProvenanceStage,
    RasterArtifact, RasterFrame, ScanObservation, ScanObservationDiagnostic,
    ScanObservationDiagnosticLevel, ScanObservationValidationError, ScanSource, UnicodeRange,
    UnknownReason,
};
