//! Composição funcional da verificação estrutural de fonte.
//!
//! O fluxo carrega a observação, vincula o raster, gera e compila um candidato
//! Typst, inspeciona seu PDF e publica somente resultados preservados.

use super::{
    materialize_page, materialized_page_report_diagnostics, observation_report_diagnostics,
    scan_observation_limits,
};
use decalque_core::{
    attest_scan_font, compare_scan_observation, plan_scan_lines, EvidenceStatus, PdfFontResource,
    ReconstructionOutcome, ReconstructionPlan, ScanFontAttestationDiagnostic,
    ScanFontAttestationInput, ScanFontAttestationReport, ScanObservation, TypographyHypothesis,
};
use decalque_infra::{
    bind_scan_observation_raster, identify_typst_compiler, load_scan_observation_json,
    load_single_page_source_from_pdf_bytes, publish_new_file, render_strict_typst_source,
    BoundRasterIdentity, RasterIdentityError, ScanObservationJsonError, TypstCompilation,
    TypstExecutionLimits,
};
use decalque_shell::{
    render_scan_comparison_report, render_scan_font_attestation_report, ScanFontAttestationCliArgs,
    ScanFontAttestationDiagnosticContext, ScanFontAttestationEvidenceContext,
    ScanFontAttestationReportContext, ScanFontAttestationResourceContext, ScanReportContext,
    SCAN_FONT_ATTESTATION_LIMITS, SCAN_OBSERVATION_RASTER_MAX_BYTES,
};
use std::fmt;

#[derive(Debug)]
pub enum ScanFontAttestationError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
    Pipeline(String),
}

impl fmt::Display for ScanFontAttestationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => write!(formatter, "observacao: {error}"),
            Self::Raster(error) => write!(formatter, "raster: {error}"),
            Self::Pipeline(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for ScanFontAttestationError {}

struct PreparedScanFontAttestation {
    observation: ScanObservation,
    raster: BoundRasterIdentity,
    typography: TypographyHypothesis,
    policy: decalque_core::ScanComparisonPolicy,
}

struct ScanFontEvidence {
    attestation: ScanFontAttestationReport,
    comparison_json: String,
}

fn plan_lines(
    prepared: &PreparedScanFontAttestation,
) -> Result<ReconstructionPlan, ScanFontAttestationError> {
    match plan_scan_lines(&prepared.observation, &prepared.typography)
        .map_err(|error| ScanFontAttestationError::Pipeline(format!("planejamento: {error}")))?
    {
        ReconstructionOutcome::Materializable(plan) => Ok(plan),
        ReconstructionOutcome::Unknown(report) => Err(ScanFontAttestationError::Pipeline(format!(
            "planejamento inconclusivo (page_mapping ou claims obrigatorias): {report:?}"
        ))),
    }
}

fn inspect_candidate(
    prepared: &PreparedScanFontAttestation,
    plan: &ReconstructionPlan,
    compilation: &TypstCompilation,
) -> Result<ScanFontEvidence, ScanFontAttestationError> {
    let page_source = load_single_page_source_from_pdf_bytes(&compilation.pdf_bytes)
        .map_err(|error| ScanFontAttestationError::Pipeline(format!("PDF candidato: {error}")))?;
    let raw_font_catalog = page_source
        .fonts
        .iter()
        .map(|font| PdfFontResource {
            resource_name: font.resource_name.clone(),
            base_font: font.base_font.clone(),
        })
        .collect::<Vec<_>>();
    let candidate = materialize_page(page_source);
    let expected_line_texts = plan
        .lines
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>();

    let attestation = attest_scan_font(&ScanFontAttestationInput {
        expected_line_texts: &expected_line_texts,
        typography: &prepared.typography,
        candidate: &candidate.geometry,
        font_resources: &raw_font_catalog,
        font_model_diagnostics: &candidate.font_diagnostics,
        text_interpreter_diagnostics: &candidate.text_diagnostics,
    });
    let comparison =
        compare_scan_observation(&prepared.observation, &candidate.geometry, &prepared.policy);
    let observation_diagnostics = observation_report_diagnostics(&prepared.observation);
    let pdf_diagnostics = materialized_page_report_diagnostics(&candidate);
    let comparison_json = render_scan_comparison_report(
        &ScanReportContext {
            page_index: prepared.observation.source.page_index as usize,
            raster_sha256: &prepared.raster.sha256,
            observation_diagnostics: &observation_diagnostics,
            pdf_diagnostics: &pdf_diagnostics,
        },
        &prepared.policy,
        &comparison,
    )
    .map_err(|error| {
        ScanFontAttestationError::Pipeline(format!("serializacao da comparacao: {error}"))
    })?;

    Ok(ScanFontEvidence {
        attestation,
        comparison_json,
    })
}

fn render_report(
    prepared: &PreparedScanFontAttestation,
    compilation: &TypstCompilation,
    evidence: &ScanFontEvidence,
    artifact_published: bool,
) -> Result<String, ScanFontAttestationError> {
    let diagnostics = evidence
        .attestation
        .diagnostics
        .iter()
        .map(diagnostic_context)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ScanFontAttestationError::Pipeline)?;
    let used_resources = evidence
        .attestation
        .used_resources
        .iter()
        .map(|resource| {
            Ok(ScanFontAttestationResourceContext {
                resource_name: &resource.resource_name,
                base_font: resource.raw_base_font.as_deref(),
                normalized_stem: resource.normalized_stem.as_deref(),
                glyph_count: to_u64(resource.glyph_count, "attestation.glyph_count")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(ScanFontAttestationError::Pipeline)?;

    render_scan_font_attestation_report(&ScanFontAttestationReportContext {
        page_index: prepared.observation.source.page_index as usize,
        raster_sha256: &prepared.raster.sha256,
        typography: &prepared.typography,
        compiler_version: &compilation.compiler_version,
        source_sha256: &compilation.source_sha256,
        source_size_bytes: compilation.source_size_bytes,
        pdf_sha256: &compilation.pdf_sha256,
        pdf_size_bytes: compilation.pdf_size_bytes,
        attestation: ScanFontAttestationEvidenceContext {
            status: evidence.attestation.status,
            expected_scalar_count: to_u64(
                evidence.attestation.expected_scalar_count,
                "attestation.expected_scalar_count",
            )
            .map_err(ScanFontAttestationError::Pipeline)?,
            candidate_scalar_count: evidence
                .attestation
                .candidate_scalar_count
                .map(|value| to_u64(value, "attestation.candidate_scalar_count"))
                .transpose()
                .map_err(ScanFontAttestationError::Pipeline)?,
            diagnostics: &diagnostics,
            used_resources: &used_resources,
        },
        comparison_json: &evidence.comparison_json,
        artifact_published,
    })
    .map_err(|error| {
        ScanFontAttestationError::Pipeline(format!("serializacao da verificacao: {error}"))
    })
}

fn diagnostic_context(
    diagnostic: &ScanFontAttestationDiagnostic,
) -> Result<ScanFontAttestationDiagnosticContext<'_>, String> {
    let (code, resource_name, scalar_index) = match diagnostic {
        ScanFontAttestationDiagnostic::TextSequenceMismatch {
            first_differing_scalar_index,
            ..
        } => (
            "text-sequence-mismatch",
            None,
            Some(*first_differing_scalar_index),
        ),
        ScanFontAttestationDiagnostic::UnmappedGlyph { glyph_index } => {
            ("unmapped-glyph", None, Some(*glyph_index))
        }
        ScanFontAttestationDiagnostic::EmptyResourceName => ("empty-resource-name", None, None),
        ScanFontAttestationDiagnostic::MissingFontResource {
            resource_name,
            first_glyph_index,
        } => (
            "missing-font-resource",
            Some(resource_name.as_str()),
            Some(*first_glyph_index),
        ),
        ScanFontAttestationDiagnostic::DuplicateFontResource { resource_name, .. } => (
            "duplicate-font-resource",
            Some(resource_name.as_str()),
            None,
        ),
        ScanFontAttestationDiagnostic::MissingBaseFont { resource_name } => {
            ("missing-base-font", Some(resource_name.as_str()), None)
        }
        ScanFontAttestationDiagnostic::EmptyBaseFont { resource_name } => {
            ("empty-base-font", Some(resource_name.as_str()), None)
        }
        ScanFontAttestationDiagnostic::UnclassifiableBaseFont { resource_name, .. } => (
            "unclassifiable-base-font",
            Some(resource_name.as_str()),
            None,
        ),
        ScanFontAttestationDiagnostic::UnclassifiableRequestedFamily { .. } => {
            ("unclassifiable-requested-family", None, None)
        }
        ScanFontAttestationDiagnostic::MissingRequiredFace { resource_name, .. } => {
            ("missing-required-face", Some(resource_name.as_str()), None)
        }
        ScanFontAttestationDiagnostic::FaceMismatch { resource_name, .. } => {
            ("face-mismatch", Some(resource_name.as_str()), None)
        }
        ScanFontAttestationDiagnostic::FamilyMismatch { resource_name, .. } => {
            ("family-mismatch", Some(resource_name.as_str()), None)
        }
        ScanFontAttestationDiagnostic::OpaqueFontModel { .. } => ("opaque-font-model", None, None),
        ScanFontAttestationDiagnostic::OpaqueTextInterpretation { .. } => {
            ("opaque-text-interpretation", None, None)
        }
    };
    Ok(ScanFontAttestationDiagnosticContext {
        code,
        resource_name,
        scalar_index: scalar_index
            .map(|value| to_u64(value, "attestation.diagnostic.scalar_index"))
            .transpose()?,
    })
}

fn to_u64(value: usize, field: &'static str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} nao cabe em u64: {value}"))
}

fn typst_limits() -> TypstExecutionLimits {
    TypstExecutionLimits {
        max_source_bytes: SCAN_FONT_ATTESTATION_LIMITS.source_max_bytes,
        max_pdf_stdout_bytes: SCAN_FONT_ATTESTATION_LIMITS.pdf_max_bytes,
        max_stderr_bytes: SCAN_FONT_ATTESTATION_LIMITS.stderr_max_bytes,
        max_version_stdout_bytes: SCAN_FONT_ATTESTATION_LIMITS.version_max_bytes,
        timeout: SCAN_FONT_ATTESTATION_LIMITS.timeout,
    }
}

/// Avalia uma hipótese estrutural e publica o PDF somente quando preservada.
pub fn run_scan_font_attestation(
    args: &ScanFontAttestationCliArgs,
) -> Result<String, ScanFontAttestationError> {
    let observation = load_scan_observation_json(&args.observation, &scan_observation_limits())
        .map_err(ScanFontAttestationError::Observation)?;
    let raster = bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanFontAttestationError::Raster)?;
    let prepared = PreparedScanFontAttestation {
        observation,
        raster,
        typography: args.typography.clone(),
        policy: args.policy.clone(),
    };

    let plan = plan_lines(&prepared)?;
    let source = render_strict_typst_source(&plan)
        .map_err(|error| ScanFontAttestationError::Pipeline(format!("emissao Typst: {error}")))?;
    let compiler = identify_typst_compiler(&args.typst_bin, &typst_limits()).map_err(|error| {
        ScanFontAttestationError::Pipeline(format!("compilador Typst: {error}"))
    })?;
    let compilation = compiler.compile(source.as_bytes()).map_err(|error| {
        ScanFontAttestationError::Pipeline(format!("compilador Typst: {error}"))
    })?;
    let evidence = inspect_candidate(&prepared, &plan, &compilation)?;
    let publish = evidence.attestation.status == EvidenceStatus::Preserved;
    let report = render_report(&prepared, &compilation, &evidence, publish)?;

    if publish {
        publish_new_file(&args.output_pdf, &compilation.pdf_bytes)
            .map_err(|error| ScanFontAttestationError::Pipeline(format!("publicacao: {error}")))?;
    }

    Ok(report)
}
