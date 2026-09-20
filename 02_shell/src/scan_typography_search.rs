use crate::scan_report::push_json_string;
use decalque_core::{
    ConfidenceRequirement, EvidenceStatus, FontStyleHypothesis, FontWeightHypothesis,
    ScanComparisonPolicy, ScanGranularity, TextNormalization, TypographyHypothesis,
};
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Write};
use std::path::PathBuf;

pub const SCAN_TYPOGRAPHY_SEARCH_USAGE: &str = concat!(
    "uso: decalque fit-scan-lines <observacao.json> ",
    "--raster <raster> --output-pdf <vencedor.pdf> ",
    "--font-family <familia> --font-size-pt <numero> ",
    "[--font-size-pt <numero> ...] [--font-weight <regular|bold>] ",
    "[--font-style <normal|italic|oblique>] [--tracking-pt <numero> ...] ",
    "--horizontal-tolerance-pt <numero> --baseline-tolerance-pt <numero> ",
    "[--min-text-confidence <0..1>] [--min-geometry-confidence <0..1>] ",
    "[--typst-bin <caminho>]\n",
    "\n",
    "Padrões: --font-weight regular, --font-style normal, --tracking-pt 0 e ",
    "--typst-bin typst. A grade canônica contém no máximo 32 hipóteses."
);

pub const SCAN_TYPOGRAPHY_SEARCH_MAX_HYPOTHESES: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct ScanTypographySearchCliArgs {
    pub observation: PathBuf,
    pub raster: PathBuf,
    pub output_pdf: PathBuf,
    pub typst_bin: PathBuf,
    pub typography_grid: Vec<TypographyHypothesis>,
    pub policy: ScanComparisonPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanTypographySearchCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanTypographySearchCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_TYPOGRAPHY_SEARCH_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_TYPOGRAPHY_SEARCH_USAGE}")
            }
        }
    }
}

impl std::error::Error for ScanTypographySearchCliParseError {}

/// Analisa e fecha integralmente a grade antes de qualquer I/O de domínio.
pub fn parse_scan_typography_search_args<I>(
    args: I,
) -> Result<ScanTypographySearchCliArgs, ScanTypographySearchCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanTypographySearchCliParseError::HelpRequested);
    }

    let Some(observation) = args.first() else {
        return Err(invalid_usage("caminho da observação ausente"));
    };
    if observation.is_empty() {
        return Err(invalid_usage("caminho da observação vazio"));
    }
    if looks_like_option(observation) {
        return Err(invalid_usage(format!(
            "opção desconhecida: {}",
            observation.to_string_lossy()
        )));
    }

    let mut raster = None;
    let mut output_pdf = None;
    let mut typst_bin = None;
    let mut font_family = None;
    let mut font_sizes_pt = Vec::new();
    let mut font_weight = None;
    let mut font_style = None;
    let mut trackings_pt = Vec::new();
    let mut horizontal_tolerance_pt = None;
    let mut baseline_tolerance_pt = None;
    let mut min_text_confidence = None;
    let mut min_geometry_confidence = None;

    let mut index = 1;
    while index < args.len() {
        let option = &args[index];
        if !is_known_option(option) {
            return Err(invalid_usage(format!(
                "opção desconhecida: {}",
                option.to_string_lossy()
            )));
        }

        let value = args.get(index + 1).ok_or_else(|| missing_value(option))?;
        if looks_like_option(value) {
            return Err(missing_value(option));
        }

        if option == OsStr::new("--raster") {
            reject_repeated(option, raster.is_some())?;
            raster = Some(parse_path(option, value)?);
        } else if option == OsStr::new("--output-pdf") {
            reject_repeated(option, output_pdf.is_some())?;
            output_pdf = Some(parse_path(option, value)?);
        } else if option == OsStr::new("--typst-bin") {
            reject_repeated(option, typst_bin.is_some())?;
            typst_bin = Some(parse_path(option, value)?);
        } else if option == OsStr::new("--font-family") {
            reject_repeated(option, font_family.is_some())?;
            font_family = Some(parse_font_family(value)?);
        } else if option == OsStr::new("--font-size-pt") {
            font_sizes_pt.push(parse_bounded_number(option, value, 4.0, 96.0)?);
        } else if option == OsStr::new("--font-weight") {
            reject_repeated(option, font_weight.is_some())?;
            font_weight = Some(parse_font_weight(value)?);
        } else if option == OsStr::new("--font-style") {
            reject_repeated(option, font_style.is_some())?;
            font_style = Some(parse_font_style(value)?);
        } else if option == OsStr::new("--tracking-pt") {
            trackings_pt.push(parse_bounded_number(option, value, -2.0, 2.0)?);
        } else if option == OsStr::new("--horizontal-tolerance-pt") {
            reject_repeated(option, horizontal_tolerance_pt.is_some())?;
            horizontal_tolerance_pt = Some(parse_tolerance(option, value)?);
        } else if option == OsStr::new("--baseline-tolerance-pt") {
            reject_repeated(option, baseline_tolerance_pt.is_some())?;
            baseline_tolerance_pt = Some(parse_tolerance(option, value)?);
        } else if option == OsStr::new("--min-text-confidence") {
            reject_repeated(option, min_text_confidence.is_some())?;
            min_text_confidence = Some(parse_confidence(option, value)?);
        } else if option == OsStr::new("--min-geometry-confidence") {
            reject_repeated(option, min_geometry_confidence.is_some())?;
            min_geometry_confidence = Some(parse_confidence(option, value)?);
        }

        index += 2;
    }

    let raster = raster.ok_or_else(|| missing_option("--raster"))?;
    let output_pdf = output_pdf.ok_or_else(|| missing_option("--output-pdf"))?;
    let font_family = font_family.ok_or_else(|| missing_option("--font-family"))?;
    if font_sizes_pt.is_empty() {
        return Err(missing_option("--font-size-pt"));
    }
    let horizontal_tolerance_pt =
        horizontal_tolerance_pt.ok_or_else(|| missing_option("--horizontal-tolerance-pt"))?;
    let baseline_tolerance_pt =
        baseline_tolerance_pt.ok_or_else(|| missing_option("--baseline-tolerance-pt"))?;

    if trackings_pt.is_empty() {
        trackings_pt.push(0.0);
    }
    canonicalize_values(&mut font_sizes_pt, "--font-size-pt")?;
    canonicalize_values(&mut trackings_pt, "--tracking-pt")?;

    let hypothesis_count = font_sizes_pt
        .len()
        .checked_mul(trackings_pt.len())
        .ok_or_else(|| invalid_usage("overflow ao calcular a grade tipográfica"))?;
    if hypothesis_count == 0 || hypothesis_count > SCAN_TYPOGRAPHY_SEARCH_MAX_HYPOTHESES {
        return Err(invalid_usage(format!(
            "a grade tipográfica deve conter de 1 a {} hipóteses",
            SCAN_TYPOGRAPHY_SEARCH_MAX_HYPOTHESES
        )));
    }

    let weight = font_weight.unwrap_or(FontWeightHypothesis::Regular);
    let style = font_style.unwrap_or(FontStyleHypothesis::Normal);
    let mut typography_grid = Vec::with_capacity(hypothesis_count);
    for size_pt in font_sizes_pt {
        for &tracking_pt in &trackings_pt {
            typography_grid.push(TypographyHypothesis {
                font_family: font_family.clone(),
                size_pt,
                weight,
                style,
                tracking_pt,
            });
        }
    }

    Ok(ScanTypographySearchCliArgs {
        observation: PathBuf::from(observation),
        raster,
        output_pdf,
        typst_bin: typst_bin.unwrap_or_else(|| PathBuf::from("typst")),
        typography_grid,
        policy: ScanComparisonPolicy {
            granularity: ScanGranularity::Line,
            horizontal_tolerance_pt,
            baseline_tolerance_pt,
            text_confidence: min_text_confidence.map_or(
                ConfidenceRequirement::Any,
                ConfidenceRequirement::KnownAtLeast,
            ),
            geometry_confidence: min_geometry_confidence.map_or(
                ConfidenceRequirement::Any,
                ConfidenceRequirement::KnownAtLeast,
            ),
            text_normalization: TextNormalization::Exact,
        },
    })
}

fn is_known_option(option: &OsStr) -> bool {
    option == OsStr::new("--raster")
        || option == OsStr::new("--output-pdf")
        || option == OsStr::new("--font-family")
        || option == OsStr::new("--font-size-pt")
        || option == OsStr::new("--font-weight")
        || option == OsStr::new("--font-style")
        || option == OsStr::new("--tracking-pt")
        || option == OsStr::new("--horizontal-tolerance-pt")
        || option == OsStr::new("--baseline-tolerance-pt")
        || option == OsStr::new("--min-text-confidence")
        || option == OsStr::new("--min-geometry-confidence")
        || option == OsStr::new("--typst-bin")
}

fn looks_like_option(value: &OsStr) -> bool {
    value.to_string_lossy().starts_with("--")
}

fn parse_path(option: &OsStr, value: &OsStr) -> Result<PathBuf, ScanTypographySearchCliParseError> {
    if value.is_empty() {
        Err(invalid_value(option, value))
    } else {
        Ok(PathBuf::from(value))
    }
}

fn parse_font_family(value: &OsStr) -> Result<String, ScanTypographySearchCliParseError> {
    let family = value
        .to_str()
        .filter(|family| !family.trim().is_empty())
        .ok_or_else(|| invalid_value(OsStr::new("--font-family"), value))?;
    Ok(family.to_string())
}

fn parse_font_weight(
    value: &OsStr,
) -> Result<FontWeightHypothesis, ScanTypographySearchCliParseError> {
    if value == OsStr::new("regular") {
        Ok(FontWeightHypothesis::Regular)
    } else if value == OsStr::new("bold") {
        Ok(FontWeightHypothesis::Bold)
    } else {
        Err(invalid_value(OsStr::new("--font-weight"), value))
    }
}

fn parse_font_style(
    value: &OsStr,
) -> Result<FontStyleHypothesis, ScanTypographySearchCliParseError> {
    if value == OsStr::new("normal") {
        Ok(FontStyleHypothesis::Normal)
    } else if value == OsStr::new("italic") {
        Ok(FontStyleHypothesis::Italic)
    } else if value == OsStr::new("oblique") {
        Ok(FontStyleHypothesis::Oblique)
    } else {
        Err(invalid_value(OsStr::new("--font-style"), value))
    }
}

fn parse_bounded_number(
    option: &OsStr,
    value: &OsStr,
    minimum: f64,
    maximum: f64,
) -> Result<f64, ScanTypographySearchCliParseError> {
    let parsed = normalize_zero(parse_finite_number(option, value)?);
    if !(minimum..=maximum).contains(&parsed) {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_tolerance(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanTypographySearchCliParseError> {
    let parsed = normalize_zero(parse_finite_number(option, value)?);
    if parsed < 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_confidence(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanTypographySearchCliParseError> {
    let parsed = normalize_zero(parse_finite_number(option, value)?);
    if !(0.0..=1.0).contains(&parsed) {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_finite_number(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanTypographySearchCliParseError> {
    value
        .to_str()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid_value(option, value))
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn canonicalize_values(
    values: &mut [f64],
    option: &'static str,
) -> Result<(), ScanTypographySearchCliParseError> {
    values.sort_by(f64::total_cmp);
    if values
        .windows(2)
        .any(|pair| pair[0].total_cmp(&pair[1]) == Ordering::Equal)
    {
        return Err(invalid_usage(format!(
            "valor repetido após canonicalização em {option}"
        )));
    }
    Ok(())
}

fn reject_repeated(
    option: &OsStr,
    already_present: bool,
) -> Result<(), ScanTypographySearchCliParseError> {
    if already_present {
        Err(invalid_usage(format!(
            "opção repetida: {}",
            option.to_string_lossy()
        )))
    } else {
        Ok(())
    }
}

fn missing_value(option: &OsStr) -> ScanTypographySearchCliParseError {
    invalid_usage(format!("valor ausente para {}", option.to_string_lossy()))
}

fn invalid_value(option: &OsStr, value: &OsStr) -> ScanTypographySearchCliParseError {
    invalid_usage(format!(
        "valor inválido para {}: {}",
        option.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn missing_option(option: &str) -> ScanTypographySearchCliParseError {
    invalid_usage(format!("opção obrigatória ausente: {option}"))
}

fn invalid_usage(message: impl Into<String>) -> ScanTypographySearchCliParseError {
    ScanTypographySearchCliParseError::InvalidUsage(message.into())
}

pub struct ScanTypographySearchReportContext<'a> {
    pub page_index: u64,
    pub raster_sha256: &'a str,
    pub search_space: ScanTypographySearchSearchSpaceContext<'a>,
    pub compiler_version: &'a str,
    pub trials: &'a [ScanTypographySearchTrialContext<'a>],
    pub selection: ScanTypographySearchSelectionContext<'a>,
}

pub struct ScanTypographySearchSearchSpaceContext<'a> {
    pub font_family: &'a str,
    pub font_weight: FontWeightHypothesis,
    pub font_style: FontStyleHypothesis,
    pub font_sizes_pt: &'a [f64],
    pub trackings_pt: &'a [f64],
    pub hypothesis_count: u64,
}

pub struct ScanTypographySearchTrialContext<'a> {
    pub index: u64,
    pub hypothesis: &'a TypographyHypothesis,
    pub source_sha256: &'a str,
    pub source_size_bytes: u64,
    pub pdf_sha256: &'a str,
    pub pdf_size_bytes: u64,
    pub content_status: EvidenceStatus,
    pub geometry_status: EvidenceStatus,
    pub overall_status: EvidenceStatus,
    pub coverage: ScanTypographySearchCoverageContext,
    pub eligibility: ScanTypographySearchEligibilityContext<'a>,
}

pub struct ScanTypographySearchCoverageContext {
    pub matched_scan: u64,
    pub total_scan: u64,
    pub matched_candidate: u64,
    pub total_candidate: u64,
}

pub enum ScanTypographySearchEligibilityContext<'a> {
    Eligible {
        support: ScanTypographySearchSupportContext<'a>,
        score: ScanTypographySearchScoreContext<'a>,
    },
    Ineligible {
        reason: &'a str,
    },
}

pub struct ScanTypographySearchSupportContext<'a> {
    pub horizontal_scan_unit_ids: &'a [&'a str],
    pub known_baseline_scan_unit_ids: &'a [&'a str],
}

pub struct ScanTypographySearchScoreContext<'a> {
    pub horizontal_violations: u64,
    pub horizontal_residuals_desc: &'a [u64],
    pub baseline_violations: u64,
    pub baseline_residuals_desc: &'a [u64],
}

pub enum ScanTypographySearchSelectionContext<'a> {
    Selected {
        selected_index: u64,
        evidence_scope: &'a str,
        winner_json: &'a str,
    },
    Tied {
        indices: &'a [u64],
        evidence_scope: &'a str,
    },
    Inconclusive {
        reason: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTypographySearchReportRenderError {
    field: &'static str,
}

impl fmt::Display for ScanTypographySearchReportRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "valor não finito ao serializar o campo {}",
            self.field
        )
    }
}

impl std::error::Error for ScanTypographySearchReportRenderError {}

/// Renderiza o relatório de busca v1 em ordem fixa, sem I/O nem dependências externas.
pub fn render_scan_typography_search_report(
    context: &ScanTypographySearchReportContext<'_>,
) -> Result<String, ScanTypographySearchReportRenderError> {
    let mut json = String::new();
    json.push_str("{\"schema\":\"decalque.scan-typography-search\",\"schema_version\":1");

    write!(json, ",\"source\":{{\"page_index\":{}", context.page_index).unwrap();
    json.push_str(",\"raster_sha256\":");
    push_json_string(&mut json, context.raster_sha256);
    json.push('}');

    push_search_space(&mut json, &context.search_space)?;
    json.push_str(",\"compiler_version\":");
    push_json_string(&mut json, context.compiler_version);

    json.push_str(",\"trials\":[");
    for (index, trial) in context.trials.iter().enumerate() {
        push_separator(&mut json, index);
        push_trial(&mut json, trial)?;
    }
    json.push(']');

    json.push_str(",\"selection\":");
    push_selection(&mut json, &context.selection);
    json.push_str(",\"winner\":");
    match &context.selection {
        ScanTypographySearchSelectionContext::Selected { winner_json, .. } => {
            json.push_str(winner_json)
        }
        ScanTypographySearchSelectionContext::Tied { .. }
        | ScanTypographySearchSelectionContext::Inconclusive { .. } => json.push_str("null"),
    }
    json.push('}');
    Ok(json)
}

fn push_search_space(
    json: &mut String,
    context: &ScanTypographySearchSearchSpaceContext<'_>,
) -> Result<(), ScanTypographySearchReportRenderError> {
    json.push_str(",\"search_space\":{\"font_family\":");
    push_json_string(json, context.font_family);
    json.push_str(",\"font_weight\":");
    push_json_string(json, font_weight(context.font_weight));
    json.push_str(",\"font_style\":");
    push_json_string(json, font_style(context.font_style));
    json.push_str(",\"font_sizes_pt\":[");
    push_f64_array(json, context.font_sizes_pt, "search_space.font_sizes_pt")?;
    json.push_str("],\"trackings_pt\":[");
    push_f64_array(json, context.trackings_pt, "search_space.trackings_pt")?;
    write!(
        json,
        "],\"hypothesis_count\":{}}}",
        context.hypothesis_count
    )
    .unwrap();
    Ok(())
}

fn push_trial(
    json: &mut String,
    trial: &ScanTypographySearchTrialContext<'_>,
) -> Result<(), ScanTypographySearchReportRenderError> {
    write!(json, "{{\"index\":{}", trial.index).unwrap();
    json.push_str(",\"hypothesis\":{\"font_family\":");
    push_json_string(json, &trial.hypothesis.font_family);
    json.push_str(",\"font_size_pt\":");
    push_f64(
        json,
        trial.hypothesis.size_pt,
        "trials.hypothesis.font_size_pt",
    )?;
    json.push_str(",\"font_weight\":");
    push_json_string(json, font_weight(trial.hypothesis.weight));
    json.push_str(",\"font_style\":");
    push_json_string(json, font_style(trial.hypothesis.style));
    json.push_str(",\"tracking_pt\":");
    push_f64(
        json,
        trial.hypothesis.tracking_pt,
        "trials.hypothesis.tracking_pt",
    )?;
    json.push('}');

    json.push_str(",\"source_sha256\":");
    push_json_string(json, trial.source_sha256);
    write!(json, ",\"source_size_bytes\":{}", trial.source_size_bytes).unwrap();
    json.push_str(",\"pdf_sha256\":");
    push_json_string(json, trial.pdf_sha256);
    write!(json, ",\"pdf_size_bytes\":{}", trial.pdf_size_bytes).unwrap();
    json.push_str(",\"content_status\":");
    push_json_string(json, evidence_status(&trial.content_status));
    json.push_str(",\"geometry_status\":");
    push_json_string(json, evidence_status(&trial.geometry_status));
    json.push_str(",\"overall_status\":");
    push_json_string(json, evidence_status(&trial.overall_status));
    write!(
        json,
        ",\"coverage\":{{\"matched_scan\":{},\"total_scan\":{},\"matched_candidate\":{},\"total_candidate\":{}}}",
        trial.coverage.matched_scan,
        trial.coverage.total_scan,
        trial.coverage.matched_candidate,
        trial.coverage.total_candidate,
    )
    .unwrap();

    match &trial.eligibility {
        ScanTypographySearchEligibilityContext::Eligible { support, score } => {
            json.push_str(",\"eligibility\":{\"status\":\"eligible\"}");
            json.push_str(",\"support\":");
            push_support(json, support);
            json.push_str(",\"score\":");
            push_score(json, score);
        }
        ScanTypographySearchEligibilityContext::Ineligible { reason } => {
            json.push_str(",\"eligibility\":{\"status\":\"ineligible\",\"reason\":");
            push_json_string(json, reason);
            json.push_str("},\"support\":null,\"score\":null");
        }
    }
    json.push('}');
    Ok(())
}

fn push_support(json: &mut String, support: &ScanTypographySearchSupportContext<'_>) {
    json.push_str("{\"horizontal_scan_unit_ids\":[");
    for (index, &value) in support.horizontal_scan_unit_ids.iter().enumerate() {
        push_separator(json, index);
        push_json_string(json, value);
    }
    json.push_str("],\"known_baseline_scan_unit_ids\":[");
    for (index, &value) in support.known_baseline_scan_unit_ids.iter().enumerate() {
        push_separator(json, index);
        push_json_string(json, value);
    }
    json.push_str("]}");
}

fn push_score(json: &mut String, score: &ScanTypographySearchScoreContext<'_>) {
    write!(
        json,
        "{{\"horizontal_violations\":{}",
        score.horizontal_violations
    )
    .unwrap();
    json.push_str(",\"horizontal_residuals_desc\":[");
    push_u64_array(json, score.horizontal_residuals_desc);
    write!(
        json,
        "],\"baseline_violations\":{}",
        score.baseline_violations
    )
    .unwrap();
    json.push_str(",\"baseline_residuals_desc\":[");
    push_u64_array(json, score.baseline_residuals_desc);
    json.push_str("]}");
}

fn push_selection(json: &mut String, selection: &ScanTypographySearchSelectionContext<'_>) {
    match selection {
        ScanTypographySearchSelectionContext::Selected {
            selected_index,
            evidence_scope,
            ..
        } => {
            write!(
                json,
                "{{\"status\":\"selected\",\"selected_index\":{selected_index},\"evidence_scope\":"
            )
            .unwrap();
            push_json_string(json, evidence_scope);
            json.push_str(",\"artifact_published\":true}");
        }
        ScanTypographySearchSelectionContext::Tied {
            indices,
            evidence_scope,
        } => {
            json.push_str("{\"status\":\"tied\",\"indices\":[");
            push_u64_array(json, indices);
            json.push_str("],\"evidence_scope\":");
            push_json_string(json, evidence_scope);
            json.push_str(",\"artifact_published\":false}");
        }
        ScanTypographySearchSelectionContext::Inconclusive { reason } => {
            json.push_str("{\"status\":\"inconclusive\",\"reason\":");
            push_json_string(json, reason);
            json.push_str(",\"evidence_scope\":null,\"artifact_published\":false}");
        }
    }
}

fn push_f64_array(
    json: &mut String,
    values: &[f64],
    field: &'static str,
) -> Result<(), ScanTypographySearchReportRenderError> {
    for (index, &value) in values.iter().enumerate() {
        push_separator(json, index);
        push_f64(json, value, field)?;
    }
    Ok(())
}

fn push_f64(
    json: &mut String,
    value: f64,
    field: &'static str,
) -> Result<(), ScanTypographySearchReportRenderError> {
    if !value.is_finite() {
        return Err(ScanTypographySearchReportRenderError { field });
    }
    write!(json, "{}", normalize_zero(value)).unwrap();
    Ok(())
}

fn push_u64_array(json: &mut String, values: &[u64]) {
    for (index, value) in values.iter().enumerate() {
        push_separator(json, index);
        write!(json, "{value}").unwrap();
    }
}

fn push_separator(json: &mut String, index: usize) {
    if index != 0 {
        json.push(',');
    }
}

fn evidence_status(status: &EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Preserved => "preserved",
        EvidenceStatus::Violated => "violated",
        EvidenceStatus::Unknown => "unknown",
    }
}

fn font_weight(weight: FontWeightHypothesis) -> &'static str {
    match weight {
        FontWeightHypothesis::Regular => "regular",
        FontWeightHypothesis::Bold => "bold",
    }
}

fn font_style(style: FontStyleHypothesis) -> &'static str {
    match style {
        FontStyleHypothesis::Normal => "normal",
        FontStyleHypothesis::Italic => "italic",
        FontStyleHypothesis::Oblique => "oblique",
    }
}
