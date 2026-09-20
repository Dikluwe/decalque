//! Composição do pipeline do Decalque (L4).
//!
//! Este módulo liga a tradução sintática de `03_infra` ao domínio puro de
//! `01_core`. Como arquivo de composição, não declara uma linhagem de prompt
//! própria (ADR 0003).

mod scan_font_attestation;
mod scan_layout;
mod scan_typography_search;

pub use scan_font_attestation::{run_scan_font_attestation, ScanFontAttestationError};
pub use scan_layout::{
    finalize_scan_layout_profile, ScanLayoutFinalizeError, ScanLayoutFinalizeErrorKind,
    ScanLayoutPublisher,
};
pub use scan_typography_search::{run_scan_typography_search, ScanTypographySearchError};

use decalque_core::entities::{resolve_page_geometry, PageGeometryDiagnostic};
use decalque_core::{
    build_font_model, compare_scan_observation, diagnose_page, interpret_text, plan_scan_lines,
    DocumentGeometry, FontModelDiagnostic, ReconstructionInputError, ReconstructionOutcome,
    ScanObservation, ScanObservationDiagnosticLevel, TextInterpretationInput,
    TextInterpreterDiagnostic,
};
use decalque_infra::{
    bind_scan_observation_raster, compile_typst_candidate, load_scan_observation_json,
    load_single_page_source_from_pdf_bytes, publish_new_file, render_typst_source, PageSource,
    RasterIdentityError, ScanObservationJsonError, ScanObservationLimits, TypstExecutionLimits,
};
use decalque_shell::{
    render_scan_comparison_report, render_scan_evaluation_report,
    render_scan_observation_validation_report, ScanEvaluationCliArgs, ScanEvaluationReportContext,
    ScanObservationCliArgs, ScanObservationValidationCliArgs, ScanReconstructionCliArgs,
    ScanReportContext, ScanReportDiagnostic, ScanReportRenderError, SCAN_EVALUATION_MAX_PDF_BYTES,
    SCAN_EVALUATION_MAX_SOURCE_BYTES, SCAN_EVALUATION_MAX_STDERR_BYTES,
    SCAN_EVALUATION_MAX_VERSION_BYTES, SCAN_EVALUATION_TIMEOUT, SCAN_OBSERVATION_INPUT_LIMITS,
    SCAN_OBSERVATION_RASTER_MAX_BYTES,
};
use std::fmt;
use std::path::Path;

/// Resultado completo da materialização de uma página.
///
/// `DocumentGeometry` recebe apenas diagnósticos de página, conforme o seu
/// contrato. Os diagnósticos das etapas de geometria, fonte e interpretação
/// permanecem separados para uma futura política de apresentação do shell.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedPage {
    pub geometry: DocumentGeometry,
    pub page_geometry_diagnostics: Vec<PageGeometryDiagnostic>,
    pub font_diagnostics: Vec<FontModelDiagnostic>,
    pub text_diagnostics: Vec<TextInterpreterDiagnostic>,
}

/// Materializa a fonte de uma página no contrato consumido pelo comparador.
pub fn materialize_page(source: PageSource) -> MaterializedPage {
    let (page, page_geometry_diagnostics) = resolve_page_geometry(&source.box_model);

    let mut fonts = Vec::with_capacity(source.fonts.len());
    let mut font_diagnostics = Vec::new();
    for raw_font in &source.fonts {
        let (font, diagnostics) = build_font_model(raw_font);
        fonts.push(font);
        font_diagnostics.extend(diagnostics);
    }

    let page_diagnostics = diagnose_page(
        source.hints.has_text_show_operators,
        source.hints.has_do_operator,
        &source.xobjects,
    );
    let interpretation = interpret_text(&TextInterpretationInput {
        page,
        operations: source.operations,
        fonts,
        xobjects: source.xobjects,
    });

    MaterializedPage {
        geometry: DocumentGeometry {
            page,
            glyphs: interpretation.glyphs,
            diagnostics: page_diagnostics,
        },
        page_geometry_diagnostics,
        font_diagnostics,
        text_diagnostics: interpretation.diagnostics,
    }
}

/// Carrega e materializa uma página de PDF pelo adaptador configurado.
pub fn load_materialized_page(
    path: &Path,
    page_index: usize,
) -> Result<MaterializedPage, decalque_core::PdfError> {
    decalque_infra::load_page_source(path, page_index).map(materialize_page)
}

#[derive(Debug)]
pub enum ScanObservationComparisonError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
    Candidate(decalque_core::PdfError),
    Serialization(ScanReportRenderError),
}

#[derive(Debug)]
pub enum ScanObservationValidationError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
}

#[derive(Debug)]
pub enum ScanLineReconstructionError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
    Planning(ReconstructionInputError),
    Unknown(String),
    Typst(String),
}

#[derive(Debug)]
pub enum ScanLineEvaluationError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
    Planning(ReconstructionInputError),
    Unknown(String),
    TypstSource(String),
    Compiler(String),
    Candidate(decalque_core::PdfError),
    ComparisonSerialization(ScanReportRenderError),
    EvaluationSerialization(String),
    Publication(String),
}

impl fmt::Display for ScanObservationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => write!(formatter, "observação: {error:?}"),
            Self::Raster(error) => write!(formatter, "raster: {error}"),
        }
    }
}

impl fmt::Display for ScanObservationComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => write!(formatter, "observação: {error:?}"),
            Self::Raster(error) => write!(formatter, "raster: {error}"),
            Self::Candidate(error) => write!(formatter, "candidato: {error}"),
            Self::Serialization(error) => write!(formatter, "serialização: {error}"),
        }
    }
}

impl fmt::Display for ScanLineReconstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => write!(formatter, "observação: {error}"),
            Self::Raster(error) => write!(formatter, "raster: {error}"),
            Self::Planning(error) => write!(formatter, "planejamento: {error}"),
            Self::Unknown(detail) => write!(
                formatter,
                "planejamento inconclusivo (page_mapping ou claims obrigatórias): {detail}"
            ),
            Self::Typst(detail) => write!(formatter, "emissão Typst: {detail}"),
        }
    }
}

impl fmt::Display for ScanLineEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => write!(formatter, "observação: {error}"),
            Self::Raster(error) => write!(formatter, "raster: {error}"),
            Self::Planning(error) => write!(formatter, "planejamento: {error}"),
            Self::Unknown(detail) => write!(
                formatter,
                "planejamento inconclusivo (page_mapping ou claims obrigatórias): {detail}"
            ),
            Self::TypstSource(detail) => write!(formatter, "emissão Typst: {detail}"),
            Self::Compiler(detail) => write!(formatter, "compilador Typst: {detail}"),
            Self::Candidate(error) => write!(formatter, "PDF candidato: {error}"),
            Self::ComparisonSerialization(error) => {
                write!(formatter, "serialização da comparação: {error}")
            }
            Self::EvaluationSerialization(detail) => {
                write!(formatter, "serialização da iteração: {detail}")
            }
            Self::Publication(detail) => write!(formatter, "publicação do PDF: {detail}"),
        }
    }
}

fn scan_observation_limits() -> ScanObservationLimits {
    let product_limits = SCAN_OBSERVATION_INPUT_LIMITS;
    ScanObservationLimits {
        max_input_bytes: product_limits.max_input_bytes,
        max_units: product_limits.max_units,
        max_provenance_records: product_limits.max_provenance_records,
        max_geometry_points: product_limits.max_geometry_points,
        max_total_text_bytes: product_limits.max_total_text_bytes,
    }
}

fn observation_report_diagnostics(observation: &ScanObservation) -> Vec<ScanReportDiagnostic> {
    observation
        .diagnostics
        .iter()
        .map(|diagnostic| ScanReportDiagnostic {
            code: diagnostic.code.clone(),
            level: Some(
                match diagnostic.level {
                    ScanObservationDiagnosticLevel::Information => "information",
                    ScanObservationDiagnosticLevel::Warning => "warning",
                }
                .to_string(),
            ),
            detail: diagnostic.detail.clone(),
        })
        .collect()
}

fn materialized_page_report_diagnostics(candidate: &MaterializedPage) -> Vec<ScanReportDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(
        candidate
            .geometry
            .diagnostics
            .iter()
            .map(ScanReportDiagnostic::from_debug),
    );
    diagnostics.extend(
        candidate
            .page_geometry_diagnostics
            .iter()
            .map(ScanReportDiagnostic::from_debug),
    );
    diagnostics.extend(
        candidate
            .font_diagnostics
            .iter()
            .map(ScanReportDiagnostic::from_debug),
    );
    diagnostics.extend(
        candidate
            .text_diagnostics
            .iter()
            .map(ScanReportDiagnostic::from_debug),
    );
    diagnostics
}

/// Compõe a fronteira JSON, o pipeline PDF e o comparador puro na ordem
/// determinada pela ADR 0004. O chamador só recebe JSON depois que todas as
/// etapas terminam com sucesso.
pub fn run_scan_observation_comparison(
    args: &ScanObservationCliArgs,
) -> Result<String, ScanObservationComparisonError> {
    let limits = scan_observation_limits();
    let observation = load_scan_observation_json(&args.observation, &limits)
        .map_err(ScanObservationComparisonError::Observation)?;
    let bound_raster = bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanObservationComparisonError::Raster)?;
    let page_index = observation.source.page_index as usize;
    let candidate = load_materialized_page(&args.candidate, page_index)
        .map_err(ScanObservationComparisonError::Candidate)?;
    let report = compare_scan_observation(&observation, &candidate.geometry, &args.policy);

    let observation_diagnostics = observation_report_diagnostics(&observation);
    let pdf_diagnostics = materialized_page_report_diagnostics(&candidate);

    render_scan_comparison_report(
        &ScanReportContext {
            page_index,
            raster_sha256: &bound_raster.sha256,
            observation_diagnostics: &observation_diagnostics,
            pdf_diagnostics: &pdf_diagnostics,
        },
        &args.policy,
        &report,
    )
    .map_err(ScanObservationComparisonError::Serialization)
}

/// Valida o contrato e vincula a identidade declarada aos bytes do raster,
/// sem carregar candidato nem executar OCR.
pub fn run_scan_observation_validation(
    args: &ScanObservationValidationCliArgs,
) -> Result<String, ScanObservationValidationError> {
    let limits = scan_observation_limits();
    let observation = load_scan_observation_json(&args.observation, &limits)
        .map_err(ScanObservationValidationError::Observation)?;
    bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanObservationValidationError::Raster)?;
    Ok(render_scan_observation_validation_report(&observation))
}

/// Compõe a reconstrução estrita por linhas sem consultar ou produzir um PDF
/// candidato. A fonte Typst só é devolvida depois de todas as etapas terem
/// terminado com sucesso.
pub fn run_scan_line_reconstruction(
    args: &ScanReconstructionCliArgs,
) -> Result<String, ScanLineReconstructionError> {
    let limits = scan_observation_limits();
    let observation = load_scan_observation_json(&args.observation, &limits)
        .map_err(ScanLineReconstructionError::Observation)?;
    bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanLineReconstructionError::Raster)?;
    let outcome = plan_scan_lines(&observation, &args.typography)
        .map_err(ScanLineReconstructionError::Planning)?;
    let plan = match outcome {
        ReconstructionOutcome::Materializable(plan) => plan,
        ReconstructionOutcome::Unknown(report) => {
            return Err(ScanLineReconstructionError::Unknown(format!("{report:?}")))
        }
    };

    render_typst_source(&plan).map_err(ScanLineReconstructionError::Typst)
}

/// Executa uma iteração completa sem permitir que o PDF candidato participe
/// da observação, do vínculo físico ou do planejamento que o gerou.
pub fn run_scan_line_evaluation(
    args: &ScanEvaluationCliArgs,
) -> Result<String, ScanLineEvaluationError> {
    let observation = load_scan_observation_json(&args.observation, &scan_observation_limits())
        .map_err(ScanLineEvaluationError::Observation)?;
    let bound_raster = bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanLineEvaluationError::Raster)?;

    let outcome = plan_scan_lines(&observation, &args.typography)
        .map_err(ScanLineEvaluationError::Planning)?;
    let plan = match outcome {
        ReconstructionOutcome::Materializable(plan) => plan,
        ReconstructionOutcome::Unknown(report) => {
            return Err(ScanLineEvaluationError::Unknown(format!("{report:?}")))
        }
    };
    let source = render_typst_source(&plan).map_err(ScanLineEvaluationError::TypstSource)?;

    let execution_limits = TypstExecutionLimits {
        max_source_bytes: SCAN_EVALUATION_MAX_SOURCE_BYTES,
        max_pdf_stdout_bytes: SCAN_EVALUATION_MAX_PDF_BYTES,
        max_stderr_bytes: SCAN_EVALUATION_MAX_STDERR_BYTES,
        max_version_stdout_bytes: SCAN_EVALUATION_MAX_VERSION_BYTES,
        timeout: SCAN_EVALUATION_TIMEOUT,
    };
    let compiled = compile_typst_candidate(&args.typst_bin, source.as_bytes(), &execution_limits)
        .map_err(|error| ScanLineEvaluationError::Compiler(error.to_string()))?;
    let candidate_source = load_single_page_source_from_pdf_bytes(&compiled.pdf_bytes)
        .map_err(ScanLineEvaluationError::Candidate)?;
    let candidate = materialize_page(candidate_source);
    let comparison = compare_scan_observation(&observation, &candidate.geometry, &args.policy);

    let page_index = observation.source.page_index as usize;
    let observation_diagnostics = observation_report_diagnostics(&observation);
    let pdf_diagnostics = materialized_page_report_diagnostics(&candidate);
    let comparison_json = render_scan_comparison_report(
        &ScanReportContext {
            page_index,
            raster_sha256: &bound_raster.sha256,
            observation_diagnostics: &observation_diagnostics,
            pdf_diagnostics: &pdf_diagnostics,
        },
        &args.policy,
        &comparison,
    )
    .map_err(ScanLineEvaluationError::ComparisonSerialization)?;

    let report = render_scan_evaluation_report(&ScanEvaluationReportContext {
        page_index,
        raster_sha256: &bound_raster.sha256,
        typography: &args.typography,
        compiler_version: &compiled.compiler_version,
        source_sha256: &compiled.source_sha256,
        source_size_bytes: compiled.source_size_bytes,
        pdf_sha256: &compiled.pdf_sha256,
        pdf_size_bytes: compiled.pdf_size_bytes,
        comparison_json: &comparison_json,
    })
    .map_err(|error| ScanLineEvaluationError::EvaluationSerialization(error.to_string()))?;

    publish_new_file(&args.output_pdf, &compiled.pdf_bytes)
        .map_err(|error| ScanLineEvaluationError::Publication(error.to_string()))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use decalque_core::entities::{PageBoxModel, Rect};
    use decalque_core::{
        ContentOperation, PageDiagnostic, RawFontData, RawFontEncoding, RawFontSubtype,
        XObjectInfo, XObjectSubtype,
    };
    use decalque_infra::PageSourceHints;

    fn box_model() -> PageBoxModel {
        PageBoxModel {
            media_box: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 200.0,
                y1: 300.0,
            },
            crop_box: None,
            rotate: None,
            user_unit: None,
        }
    }

    #[test]
    fn materializa_pagina_de_imagem_sem_inventar_glifos() {
        let source = PageSource {
            box_model: box_model(),
            operations: vec![ContentOperation::InvokeXObject {
                name: "Im1".to_string(),
            }],
            fonts: Vec::new(),
            xobjects: vec![XObjectInfo {
                name: "Im1".to_string(),
                subtype: XObjectSubtype::Image,
            }],
            hints: PageSourceHints {
                has_text_show_operators: false,
                has_do_operator: true,
            },
        };

        let result = materialize_page(source);
        assert!(result.geometry.glyphs.is_empty());
        assert_eq!(
            result.geometry.diagnostics,
            vec![PageDiagnostic::ImageOnlyPage]
        );
        assert!(result.page_geometry_diagnostics.is_empty());
        assert!(result.font_diagnostics.is_empty());
    }

    #[test]
    fn materializa_texto_e_preserva_diagnosticos_por_etapa() {
        let source = PageSource {
            box_model: box_model(),
            operations: vec![
                ContentOperation::BeginText,
                ContentOperation::SetFont {
                    name: "F1".to_string(),
                    size_pt: 10.0,
                },
                ContentOperation::SetTextMatrix {
                    m: [1.0, 0.0, 0.0, 1.0, 20.0, 30.0],
                },
                ContentOperation::ShowText { bytes: vec![65] },
                ContentOperation::EndText,
            ],
            fonts: vec![RawFontData {
                resource_name: "F1".to_string(),
                subtype: RawFontSubtype::Type1,
                base_font: None,
                encoding: RawFontEncoding::Absent,
                default_width: Some(500.0),
                widths: vec![(65, 500.0)],
                tounicode: None,
            }],
            xobjects: Vec::new(),
            hints: PageSourceHints {
                has_text_show_operators: true,
                has_do_operator: false,
            },
        };

        let result = materialize_page(source);
        assert_eq!(result.geometry.glyphs.len(), 1);
        assert!(result.geometry.diagnostics.is_empty());
        assert!(result
            .text_diagnostics
            .contains(&TextInterpreterDiagnostic::UnmappedGlyphs));
    }
}
