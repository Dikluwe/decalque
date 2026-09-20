//! Núcleo de domínio do Decalque (L1): geometria de páginas e glifos,
//! observacoes de scan, normalização de coordenadas, resolução de medição e
//! motores de comparação.
//! Zero I/O, zero dependências externas.

pub mod content;
pub mod engine;
pub mod entities;
pub mod geometry;
#[path = "engine/scan_layout.rs"]
mod scan_layout_profile;

pub use content::{
    build_font_model, interpret_text, parse_tounicode_cmap, CmapDiagnostic, CmapEntry, CmapMapping,
    CmapParseResult, ContentOperation, FontModel, FontModelDiagnostic, FontWidths,
    GlyphCodeDecoder, RawFontData, RawFontEncoding, RawFontSubtype, TextInterpretationInput,
    TextInterpretationOutput, TextInterpreterDiagnostic, TjItem, XObjectInfo, XObjectSubtype,
};
pub use engine::{
    attest_scan_font, compare, compare_scan_observation, evaluate_scan_typography_fit,
    plan_scan_lines, CandidateUnitRef, ClusterShift, ComparisonReport, ConfidenceRequirement,
    Coverage, EvidenceStatus, GeometryComponent, GlyphPair, PdfFontResource, ReflowWitness,
    ScanComparisonDiagnostic, ScanComparisonPolicy, ScanComparisonReport, ScanCoverage,
    ScanFontAttestationDiagnostic, ScanFontAttestationInput, ScanFontAttestationReport,
    ScanFontAttestationResource, ScanFontFace, ScanGranularity, ScanMatch,
    ScanTypographyEligibility, ScanTypographyEvidenceScope, ScanTypographyFitError,
    ScanTypographyFitEvaluation, ScanTypographyFitKey, ScanTypographyInconclusiveReason,
    ScanTypographyIneligibilityReason, ScanTypographyResidual, ScanTypographySelection,
    ScanTypographySupport, ScanTypographyTrialEvaluation, TextNormalization,
};
pub use entities::{
    diagnose_page, validate_scan_observation, Claim, Confidence, DocumentGeometry, EvidenceBasis,
    FontStyleHypothesis, FontWeightHypothesis, FramedBbox, FramedPolygon, FramedPolyline,
    GeometryClaims, GlyphInstance, MeasurementResolution, ObservationKind, ObservationUnit,
    PageDiagnostic, PageFrame, PageGeometry, PageMapping, PageMappingKind, PdfError,
    ProducerIdentity, ProvenanceId, ProvenanceRecord, ProvenanceStage, RasterArtifact, RasterFrame,
    ReconstructionClaimKind, ReconstructionCoverage, ReconstructionDiagnostic,
    ReconstructionInputError, ReconstructionLine, ReconstructionOutcome, ReconstructionPlan,
    ReconstructionUnknownReport, ScanObservation, ScanObservationDiagnostic,
    ScanObservationDiagnosticLevel, ScanObservationValidationError, ScanSource, TextMappingStatus,
    TypographyHypothesis, UnicodeRange, UnknownReason,
};
pub use geometry::normalize_to_top_left;
pub use scan_layout_profile::{
    derive_scan_layout, ScanLayoutBoxMeasurementView, ScanLayoutBoxView, ScanLayoutCountView,
    ScanLayoutCoverageView, ScanLayoutExtentView, ScanLayoutInsetsMeasurementView,
    ScanLayoutInsetsView, ScanLayoutNumberListMeasurementView, ScanLayoutPageView,
    ScanLayoutProfile, ScanLayoutProfileError, ScanLayoutRegionView, ScanLayoutRegionsView,
    ScanLayoutScalarMeasurementView, ScanLayoutSource, ScanLayoutStringListView,
};
