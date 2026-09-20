//! Composição exaustiva da grade tipográfica. Nenhum resultado de
//! candidato altera a observação, a política ou as hipóteses seguintes.

use super::{
    materialize_page, materialized_page_report_diagnostics, observation_report_diagnostics,
    scan_observation_limits, MaterializedPage,
};
use decalque_core::{
    compare_scan_observation, evaluate_scan_typography_fit, plan_scan_lines,
    ReconstructionInputError, ReconstructionOutcome, ScanComparisonReport, ScanObservation,
    ScanTypographyEligibility, ScanTypographyEvidenceScope, ScanTypographyFitEvaluation,
    ScanTypographyFitKey, ScanTypographyInconclusiveReason, ScanTypographyIneligibilityReason,
    ScanTypographySelection, TypographyHypothesis,
};
use decalque_infra::{
    bind_scan_observation_raster, identify_typst_compiler, load_scan_observation_json,
    load_single_page_source_from_pdf_bytes, publish_new_file, render_typst_source,
    IdentifiedTypstCompiler, RasterIdentityError, ScanObservationJsonError, TypstCompilation,
    TypstExecutionLimits,
};
use decalque_shell::{
    render_scan_comparison_report, render_scan_evaluation_report,
    render_scan_typography_search_report, ScanEvaluationReportContext, ScanReportContext,
    ScanReportRenderError, ScanTypographySearchCliArgs, ScanTypographySearchCoverageContext,
    ScanTypographySearchEligibilityContext, ScanTypographySearchReportContext,
    ScanTypographySearchReportRenderError, ScanTypographySearchScoreContext,
    ScanTypographySearchSearchSpaceContext, ScanTypographySearchSelectionContext,
    ScanTypographySearchSupportContext, ScanTypographySearchTrialContext,
    SCAN_EVALUATION_MAX_PDF_BYTES, SCAN_EVALUATION_MAX_SOURCE_BYTES,
    SCAN_EVALUATION_MAX_STDERR_BYTES, SCAN_EVALUATION_MAX_VERSION_BYTES, SCAN_EVALUATION_TIMEOUT,
    SCAN_OBSERVATION_RASTER_MAX_BYTES,
};
use std::fmt;

#[derive(Debug)]
pub enum ScanTypographySearchError {
    Observation(ScanObservationJsonError),
    Raster(RasterIdentityError),
    Planning(ReconstructionInputError),
    Unknown(String),
    TypstSource(String),
    Compiler(String),
    Candidate(decalque_core::PdfError),
    Evidence(String),
    ComparisonSerialization(ScanReportRenderError),
    WinnerSerialization(String),
    SearchSerialization(ScanTypographySearchReportRenderError),
    NonDeterministicWinner(String),
    Publication(String),
    Internal(String),
}

impl fmt::Display for ScanTypographySearchError {
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
            Self::Evidence(detail) => write!(formatter, "evidência de ajuste: {detail}"),
            Self::ComparisonSerialization(error) => {
                write!(formatter, "serialização da comparação: {error}")
            }
            Self::WinnerSerialization(detail) => {
                write!(formatter, "serialização da confirmação: {detail}")
            }
            Self::SearchSerialization(error) => {
                write!(formatter, "serialização da busca: {error}")
            }
            Self::NonDeterministicWinner(detail) => {
                write!(formatter, "non-deterministic-winner: {detail}")
            }
            Self::Publication(detail) => write!(formatter, "publicação do PDF: {detail}"),
            Self::Internal(detail) => write!(formatter, "invariante interna da busca: {detail}"),
        }
    }
}

impl std::error::Error for ScanTypographySearchError {}

#[derive(Debug)]
struct TrialMetadata {
    source_sha256: String,
    source_size_bytes: u64,
    pdf_sha256: String,
    pdf_size_bytes: u64,
}

struct SearchReportInput<'a> {
    args: &'a ScanTypographySearchCliArgs,
    observation: &'a ScanObservation,
    raster_sha256: &'a str,
    compiler_version: &'a str,
    metadata: &'a [TrialMetadata],
    reports: &'a [ScanComparisonReport],
    fit: &'a ScanTypographyFitEvaluation,
    winner_json: Option<&'a str>,
}

#[derive(Debug)]
struct RetainedCandidate {
    index: usize,
    key: ScanTypographyFitKey,
    source: Vec<u8>,
    compilation: TypstCompilation,
}

#[derive(Debug)]
struct ExecutedTrial {
    source: Vec<u8>,
    compilation: TypstCompilation,
    candidate: MaterializedPage,
    comparison: ScanComparisonReport,
}

/// Executa a grade canônica inteira, confirma uma seleção única e só então
/// publica o PDF confirmado. O JSON só é devolvido depois da publicação.
pub fn run_scan_typography_search(
    args: &ScanTypographySearchCliArgs,
) -> Result<String, ScanTypographySearchError> {
    let observation = load_scan_observation_json(&args.observation, &scan_observation_limits())
        .map_err(ScanTypographySearchError::Observation)?;
    let bound_raster = bind_scan_observation_raster(
        &observation,
        &args.raster,
        SCAN_OBSERVATION_RASTER_MAX_BYTES,
    )
    .map_err(ScanTypographySearchError::Raster)?;

    let first_hypothesis = args
        .typography_grid
        .first()
        .ok_or_else(|| ScanTypographySearchError::Internal("grade vazia após o parser".into()))?;
    // Fecha claims independentes do candidato antes da primeira invocação do
    // compilador. Cada tentativa será planejada novamente, na ordem canônica.
    materializable_plan(&observation, first_hypothesis)?;

    let compiler = identify_typst_compiler(&args.typst_bin, &execution_limits())
        .map_err(|error| ScanTypographySearchError::Compiler(error.to_string()))?;
    let mut reports = Vec::with_capacity(args.typography_grid.len());
    let mut metadata = Vec::with_capacity(args.typography_grid.len());
    let mut retained: Option<RetainedCandidate> = None;

    for (index, hypothesis) in args.typography_grid.iter().enumerate() {
        let executed = execute_trial(&observation, hypothesis, &args.policy, &compiler)?;
        let single = evaluate_scan_typography_fit(std::slice::from_ref(&executed.comparison))
            .map_err(|error| ScanTypographySearchError::Evidence(error.to_string()))?;
        let trial = single.trials.first().ok_or_else(|| {
            ScanTypographySearchError::Internal("avaliação unitária sem tentativa".into())
        })?;

        let trial_metadata = TrialMetadata {
            source_sha256: executed.compilation.source_sha256.clone(),
            source_size_bytes: executed.compilation.source_size_bytes,
            pdf_sha256: executed.compilation.pdf_sha256.clone(),
            pdf_size_bytes: executed.compilation.pdf_size_bytes,
        };

        let should_retain = trial
            .key
            .as_ref()
            .is_some_and(|key| retained.as_ref().is_none_or(|current| key < &current.key));

        let ExecutedTrial {
            source,
            compilation,
            candidate: _,
            comparison,
        } = executed;
        if should_retain {
            retained = Some(RetainedCandidate {
                index,
                key: trial.key.clone().expect("key checked above"),
                source,
                compilation,
            });
        }
        metadata.push(trial_metadata);
        reports.push(comparison);
    }

    let fit = evaluate_scan_typography_fit(&reports)
        .map_err(|error| ScanTypographySearchError::Evidence(error.to_string()))?;

    match &fit.selection {
        ScanTypographySelection::Selected {
            index,
            evidence_scope: _,
        } => {
            let retained = retained.ok_or_else(|| {
                ScanTypographySearchError::Internal(
                    "seleção única sem artefato provisório retido".into(),
                )
            })?;
            if retained.index != *index {
                return Err(ScanTypographySearchError::Internal(format!(
                    "melhor provisório {} diverge da seleção {}",
                    retained.index, index
                )));
            }

            let confirmation = execute_trial(
                &observation,
                &args.typography_grid[*index],
                &args.policy,
                &compiler,
            )?;
            confirm_reproduction(&retained, &reports[*index], &fit, &confirmation, *index)?;

            let observation_diagnostics = observation_report_diagnostics(&observation);
            let pdf_diagnostics = materialized_page_report_diagnostics(&confirmation.candidate);
            let page_index = observation.source.page_index as usize;
            let comparison_json = render_scan_comparison_report(
                &ScanReportContext {
                    page_index,
                    raster_sha256: &bound_raster.sha256,
                    observation_diagnostics: &observation_diagnostics,
                    pdf_diagnostics: &pdf_diagnostics,
                },
                &args.policy,
                &confirmation.comparison,
            )
            .map_err(ScanTypographySearchError::ComparisonSerialization)?;
            let winner_json = render_scan_evaluation_report(&ScanEvaluationReportContext {
                page_index,
                raster_sha256: &bound_raster.sha256,
                typography: &args.typography_grid[*index],
                compiler_version: &confirmation.compilation.compiler_version,
                source_sha256: &confirmation.compilation.source_sha256,
                source_size_bytes: confirmation.compilation.source_size_bytes,
                pdf_sha256: &confirmation.compilation.pdf_sha256,
                pdf_size_bytes: confirmation.compilation.pdf_size_bytes,
                comparison_json: &comparison_json,
            })
            .map_err(|error| ScanTypographySearchError::WinnerSerialization(error.to_string()))?;

            let report = render_search_report(SearchReportInput {
                args,
                observation: &observation,
                raster_sha256: &bound_raster.sha256,
                compiler_version: compiler.compiler_version(),
                metadata: &metadata,
                reports: &reports,
                fit: &fit,
                winner_json: Some(&winner_json),
            })?;
            publish_new_file(&args.output_pdf, &confirmation.compilation.pdf_bytes)
                .map_err(|error| ScanTypographySearchError::Publication(error.to_string()))?;
            Ok(report)
        }
        ScanTypographySelection::Tied { .. } | ScanTypographySelection::Inconclusive { .. } => {
            render_search_report(SearchReportInput {
                args,
                observation: &observation,
                raster_sha256: &bound_raster.sha256,
                compiler_version: compiler.compiler_version(),
                metadata: &metadata,
                reports: &reports,
                fit: &fit,
                winner_json: None,
            })
        }
    }
}

fn execution_limits() -> TypstExecutionLimits {
    TypstExecutionLimits {
        max_source_bytes: SCAN_EVALUATION_MAX_SOURCE_BYTES,
        max_pdf_stdout_bytes: SCAN_EVALUATION_MAX_PDF_BYTES,
        max_stderr_bytes: SCAN_EVALUATION_MAX_STDERR_BYTES,
        max_version_stdout_bytes: SCAN_EVALUATION_MAX_VERSION_BYTES,
        timeout: SCAN_EVALUATION_TIMEOUT,
    }
}

fn materializable_plan(
    observation: &ScanObservation,
    hypothesis: &TypographyHypothesis,
) -> Result<decalque_core::ReconstructionPlan, ScanTypographySearchError> {
    match plan_scan_lines(observation, hypothesis).map_err(ScanTypographySearchError::Planning)? {
        ReconstructionOutcome::Materializable(plan) => Ok(plan),
        ReconstructionOutcome::Unknown(report) => {
            Err(ScanTypographySearchError::Unknown(format!("{report:?}")))
        }
    }
}

fn execute_trial(
    observation: &ScanObservation,
    hypothesis: &TypographyHypothesis,
    policy: &decalque_core::ScanComparisonPolicy,
    compiler: &IdentifiedTypstCompiler,
) -> Result<ExecutedTrial, ScanTypographySearchError> {
    let plan = materializable_plan(observation, hypothesis)?;
    let source = render_typst_source(&plan).map_err(ScanTypographySearchError::TypstSource)?;
    let source = source.into_bytes();
    let compilation = compiler
        .compile(&source)
        .map_err(|error| ScanTypographySearchError::Compiler(error.to_string()))?;
    let page_source = load_single_page_source_from_pdf_bytes(&compilation.pdf_bytes)
        .map_err(ScanTypographySearchError::Candidate)?;
    let candidate = materialize_page(page_source);
    let comparison = compare_scan_observation(observation, &candidate.geometry, policy);
    Ok(ExecutedTrial {
        source,
        compilation,
        candidate,
        comparison,
    })
}

fn confirm_reproduction(
    retained: &RetainedCandidate,
    original_report: &ScanComparisonReport,
    original_fit: &ScanTypographyFitEvaluation,
    confirmation: &ExecutedTrial,
    selected_index: usize,
) -> Result<(), ScanTypographySearchError> {
    if confirmation.source != retained.source {
        return Err(ScanTypographySearchError::NonDeterministicWinner(
            "os bytes da fonte Typst divergiram".into(),
        ));
    }
    if confirmation.compilation.compiler_version != retained.compilation.compiler_version
        || confirmation.compilation.source_sha256 != retained.compilation.source_sha256
        || confirmation.compilation.source_size_bytes != retained.compilation.source_size_bytes
        || confirmation.compilation.pdf_sha256 != retained.compilation.pdf_sha256
        || confirmation.compilation.pdf_size_bytes != retained.compilation.pdf_size_bytes
        || confirmation.compilation.pdf_bytes != retained.compilation.pdf_bytes
    {
        return Err(ScanTypographySearchError::NonDeterministicWinner(
            "a compilação confirmatória divergiu da tentativa original".into(),
        ));
    }
    if &confirmation.comparison != original_report {
        return Err(ScanTypographySearchError::NonDeterministicWinner(
            "o ScanComparisonReport confirmatório divergiu".into(),
        ));
    }

    let confirmation_fit =
        evaluate_scan_typography_fit(std::slice::from_ref(&confirmation.comparison))
            .map_err(|error| ScanTypographySearchError::Evidence(error.to_string()))?;
    let confirmation_trial = confirmation_fit.trials.first().ok_or_else(|| {
        ScanTypographySearchError::Internal("confirmação sem tentativa de ajuste".into())
    })?;
    let original_trial = original_fit.trials.get(selected_index).ok_or_else(|| {
        ScanTypographySearchError::Internal("índice selecionado fora das tentativas".into())
    })?;
    if confirmation_trial.eligibility != original_trial.eligibility
        || confirmation_trial.support != original_trial.support
        || confirmation_trial.key != original_trial.key
        || original_trial.key.as_ref() != Some(&retained.key)
    {
        return Err(ScanTypographySearchError::NonDeterministicWinner(
            "elegibilidade, suporte ou chave confirmatórios divergiram".into(),
        ));
    }
    Ok(())
}

fn render_search_report(input: SearchReportInput<'_>) -> Result<String, ScanTypographySearchError> {
    let SearchReportInput {
        args,
        observation,
        raster_sha256,
        compiler_version,
        metadata,
        reports,
        fit,
        winner_json,
    } = input;
    if metadata.len() != reports.len() || reports.len() != fit.trials.len() {
        return Err(ScanTypographySearchError::Internal(
            "metadados, comparações e avaliações têm comprimentos distintos".into(),
        ));
    }
    let (font_sizes, trackings) = canonical_axes(&args.typography_grid);
    let first = args
        .typography_grid
        .first()
        .ok_or_else(|| ScanTypographySearchError::Internal("grade vazia ao serializar".into()))?;

    let horizontal_ids = fit
        .trials
        .iter()
        .map(|trial| {
            trial.support.as_ref().map_or_else(Vec::new, |support| {
                support
                    .horizontal_scan_unit_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let baseline_ids = fit
        .trials
        .iter()
        .map(|trial| {
            trial.support.as_ref().map_or_else(Vec::new, |support| {
                support
                    .known_baseline_scan_unit_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    let mut trial_contexts = Vec::with_capacity(fit.trials.len());
    for (index, domain_trial) in fit.trials.iter().enumerate() {
        let report = &reports[index];
        let trial_metadata = &metadata[index];
        let eligibility = match &domain_trial.eligibility {
            ScanTypographyEligibility::Eligible => {
                let key = domain_trial.key.as_ref().ok_or_else(|| {
                    ScanTypographySearchError::Internal("elegível sem chave".into())
                })?;
                ScanTypographySearchEligibilityContext::Eligible {
                    support: ScanTypographySearchSupportContext {
                        horizontal_scan_unit_ids: &horizontal_ids[index],
                        known_baseline_scan_unit_ids: &baseline_ids[index],
                    },
                    score: ScanTypographySearchScoreContext {
                        horizontal_violations: key.horizontal_violations,
                        horizontal_residuals_desc: &key.horizontal_residuals_desc,
                        baseline_violations: key.baseline_violations,
                        baseline_residuals_desc: &key.baseline_residuals_desc,
                    },
                }
            }
            ScanTypographyEligibility::Ineligible(reason) => {
                ScanTypographySearchEligibilityContext::Ineligible {
                    reason: ineligibility_reason(*reason),
                }
            }
        };
        trial_contexts.push(ScanTypographySearchTrialContext {
            index: to_u64(index, "trial.index")?,
            hypothesis: &args.typography_grid[index],
            source_sha256: &trial_metadata.source_sha256,
            source_size_bytes: trial_metadata.source_size_bytes,
            pdf_sha256: &trial_metadata.pdf_sha256,
            pdf_size_bytes: trial_metadata.pdf_size_bytes,
            content_status: report.content_status,
            geometry_status: report.geometry_status,
            overall_status: report.overall_status,
            coverage: ScanTypographySearchCoverageContext {
                matched_scan: to_u64(report.coverage.matched_scan, "coverage.matched_scan")?,
                total_scan: to_u64(report.coverage.total_scan, "coverage.total_scan")?,
                matched_candidate: to_u64(
                    report.coverage.matched_candidate,
                    "coverage.matched_candidate",
                )?,
                total_candidate: to_u64(
                    report.coverage.total_candidate,
                    "coverage.total_candidate",
                )?,
            },
            eligibility,
        });
    }

    let tied_indices = match &fit.selection {
        ScanTypographySelection::Tied { indices, .. } => indices
            .iter()
            .map(|index| to_u64(*index, "selection.indices"))
            .collect::<Result<Vec<_>, _>>()?,
        _ => Vec::new(),
    };
    let selection = match &fit.selection {
        ScanTypographySelection::Selected {
            index,
            evidence_scope,
        } => ScanTypographySearchSelectionContext::Selected {
            selected_index: to_u64(*index, "selection.selected_index")?,
            evidence_scope: evidence_scope_name(*evidence_scope),
            winner_json: winner_json.ok_or_else(|| {
                ScanTypographySearchError::Internal("seleção sem winner JSON".into())
            })?,
        },
        ScanTypographySelection::Tied {
            indices: _,
            evidence_scope,
        } => ScanTypographySearchSelectionContext::Tied {
            indices: &tied_indices,
            evidence_scope: evidence_scope_name(*evidence_scope),
        },
        ScanTypographySelection::Inconclusive { reason } => {
            if winner_json.is_some() {
                return Err(ScanTypographySearchError::Internal(
                    "resultado inconclusivo recebeu winner JSON".into(),
                ));
            }
            ScanTypographySearchSelectionContext::Inconclusive {
                reason: inconclusive_reason(*reason),
            }
        }
    };

    render_scan_typography_search_report(&ScanTypographySearchReportContext {
        page_index: u64::from(observation.source.page_index),
        raster_sha256,
        search_space: ScanTypographySearchSearchSpaceContext {
            font_family: &first.font_family,
            font_weight: first.weight,
            font_style: first.style,
            font_sizes_pt: &font_sizes,
            trackings_pt: &trackings,
            hypothesis_count: to_u64(args.typography_grid.len(), "hypothesis_count")?,
        },
        compiler_version,
        trials: &trial_contexts,
        selection,
    })
    .map_err(ScanTypographySearchError::SearchSerialization)
}

fn canonical_axes(grid: &[TypographyHypothesis]) -> (Vec<f64>, Vec<f64>) {
    let mut font_sizes = Vec::new();
    let mut trackings = Vec::new();
    for hypothesis in grid {
        if font_sizes
            .last()
            .is_none_or(|size: &f64| size.total_cmp(&hypothesis.size_pt).is_ne())
        {
            font_sizes.push(hypothesis.size_pt);
        }
        if !trackings
            .iter()
            .any(|tracking: &f64| tracking.total_cmp(&hypothesis.tracking_pt).is_eq())
        {
            trackings.push(hypothesis.tracking_pt);
        }
    }
    trackings.sort_by(f64::total_cmp);
    (font_sizes, trackings)
}

fn ineligibility_reason(reason: ScanTypographyIneligibilityReason) -> &'static str {
    match reason {
        ScanTypographyIneligibilityReason::EmptyScanScope => "empty-scan-scope",
        ScanTypographyIneligibilityReason::ContentNotPreserved => "content-not-preserved",
        ScanTypographyIneligibilityReason::PartialCoverage => "partial-coverage",
        ScanTypographyIneligibilityReason::UnmatchedUnits => "unmatched-units",
        ScanTypographyIneligibilityReason::MissingHorizontalEvidence => {
            "missing-horizontal-evidence"
        }
    }
}

fn evidence_scope_name(scope: ScanTypographyEvidenceScope) -> &'static str {
    match scope {
        ScanTypographyEvidenceScope::HorizontalOnly => "horizontal-only",
        ScanTypographyEvidenceScope::HorizontalAndObservedBaseline => {
            "horizontal-and-observed-baseline"
        }
    }
}

fn inconclusive_reason(reason: ScanTypographyInconclusiveReason) -> &'static str {
    match reason {
        ScanTypographyInconclusiveReason::NoEligibleTrial => "no-eligible-trial",
        ScanTypographyInconclusiveReason::IncomparableSupport => "incomparable-support",
    }
}

fn to_u64(value: usize, field: &'static str) -> Result<u64, ScanTypographySearchError> {
    u64::try_from(value).map_err(|_| {
        ScanTypographySearchError::Internal(format!("{field} não cabe em u64: {value}"))
    })
}
