use crate::scan_evaluation::{ScanEvaluationLimits, SCAN_EVALUATION_LIMITS};
use crate::scan_report::push_json_string;
use decalque_core::{
    ConfidenceRequirement, EvidenceStatus, FontStyleHypothesis, FontWeightHypothesis,
    ScanComparisonPolicy, ScanGranularity, TextNormalization, TypographyHypothesis,
};
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Write};
use std::path::PathBuf;

pub const SCAN_FONT_ATTESTATION_USAGE: &str = concat!(
    "uso: decalque attest-scan-font <observacao.json> ",
    "--raster <raster> --output-pdf <candidato.pdf> ",
    "--font-family <familia> --font-size-pt <numero> ",
    "[--font-weight <regular|bold>] ",
    "[--font-style <normal|italic|oblique>] [--tracking-pt <numero>] ",
    "--horizontal-tolerance-pt <numero> --baseline-tolerance-pt <numero> ",
    "[--min-text-confidence <0..1>] [--min-geometry-confidence <0..1>] ",
    "[--typst-bin <caminho>]\n",
    "\n",
    "Padrões: granularidade line, --font-weight regular, --font-style normal, ",
    "--tracking-pt 0 e --typst-bin typst."
);

/// A verificação usa os mesmos limites operacionais da avaliação de candidatos.
pub const SCAN_FONT_ATTESTATION_LIMITS: ScanEvaluationLimits = SCAN_EVALUATION_LIMITS;

#[derive(Debug, Clone, PartialEq)]
pub struct ScanFontAttestationCliArgs {
    pub observation: PathBuf,
    pub raster: PathBuf,
    pub output_pdf: PathBuf,
    pub typst_bin: PathBuf,
    pub typography: TypographyHypothesis,
    pub policy: ScanComparisonPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanFontAttestationCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanFontAttestationCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_FONT_ATTESTATION_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_FONT_ATTESTATION_USAGE}")
            }
        }
    }
}

impl std::error::Error for ScanFontAttestationCliParseError {}

/// Analisa integralmente o subcomando antes de qualquer I/O de domínio.
pub fn parse_scan_font_attestation_args<I>(
    args: I,
) -> Result<ScanFontAttestationCliArgs, ScanFontAttestationCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanFontAttestationCliParseError::HelpRequested);
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
    let mut font_size_pt = None;
    let mut font_weight = None;
    let mut font_style = None;
    let mut tracking_pt = None;
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
            reject_repeated(option, font_size_pt.is_some())?;
            font_size_pt = Some(parse_font_size(value)?);
        } else if option == OsStr::new("--font-weight") {
            reject_repeated(option, font_weight.is_some())?;
            font_weight = Some(parse_font_weight(value)?);
        } else if option == OsStr::new("--font-style") {
            reject_repeated(option, font_style.is_some())?;
            font_style = Some(parse_font_style(value)?);
        } else if option == OsStr::new("--tracking-pt") {
            reject_repeated(option, tracking_pt.is_some())?;
            tracking_pt = Some(parse_finite_number(option, value)?);
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
    let size_pt = font_size_pt.ok_or_else(|| missing_option("--font-size-pt"))?;
    let horizontal_tolerance_pt =
        horizontal_tolerance_pt.ok_or_else(|| missing_option("--horizontal-tolerance-pt"))?;
    let baseline_tolerance_pt =
        baseline_tolerance_pt.ok_or_else(|| missing_option("--baseline-tolerance-pt"))?;

    Ok(ScanFontAttestationCliArgs {
        observation: PathBuf::from(observation),
        raster,
        output_pdf,
        typst_bin: typst_bin.unwrap_or_else(|| PathBuf::from("typst")),
        typography: TypographyHypothesis {
            font_family,
            size_pt,
            weight: font_weight.unwrap_or(FontWeightHypothesis::Regular),
            style: font_style.unwrap_or(FontStyleHypothesis::Normal),
            tracking_pt: tracking_pt.unwrap_or(0.0),
        },
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

fn parse_path(option: &OsStr, value: &OsStr) -> Result<PathBuf, ScanFontAttestationCliParseError> {
    if value.is_empty() {
        Err(invalid_value(option, value))
    } else {
        Ok(PathBuf::from(value))
    }
}

fn parse_font_family(value: &OsStr) -> Result<String, ScanFontAttestationCliParseError> {
    let family = value
        .to_str()
        .filter(|family| !family.trim().is_empty())
        .ok_or_else(|| invalid_value(OsStr::new("--font-family"), value))?;
    Ok(family.to_string())
}

fn parse_font_size(value: &OsStr) -> Result<f64, ScanFontAttestationCliParseError> {
    let option = OsStr::new("--font-size-pt");
    let size = parse_finite_number(option, value)?;
    if size <= 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(size)
}

fn parse_font_weight(
    value: &OsStr,
) -> Result<FontWeightHypothesis, ScanFontAttestationCliParseError> {
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
) -> Result<FontStyleHypothesis, ScanFontAttestationCliParseError> {
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

fn parse_tolerance(option: &OsStr, value: &OsStr) -> Result<f64, ScanFontAttestationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if parsed < 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_confidence(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanFontAttestationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if !(0.0..=1.0).contains(&parsed) {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_finite_number(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanFontAttestationCliParseError> {
    value
        .to_str()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid_value(option, value))
}

fn reject_repeated(
    option: &OsStr,
    already_present: bool,
) -> Result<(), ScanFontAttestationCliParseError> {
    if already_present {
        Err(invalid_usage(format!(
            "opção repetida: {}",
            option.to_string_lossy()
        )))
    } else {
        Ok(())
    }
}

fn missing_value(option: &OsStr) -> ScanFontAttestationCliParseError {
    invalid_usage(format!("valor ausente para {}", option.to_string_lossy()))
}

fn invalid_value(option: &OsStr, value: &OsStr) -> ScanFontAttestationCliParseError {
    invalid_usage(format!(
        "valor inválido para {}: {}",
        option.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn missing_option(option: &str) -> ScanFontAttestationCliParseError {
    invalid_usage(format!("opção obrigatória ausente: {option}"))
}

fn invalid_usage(message: impl Into<String>) -> ScanFontAttestationCliParseError {
    ScanFontAttestationCliParseError::InvalidUsage(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanFontAttestationDiagnosticContext<'a> {
    pub code: &'a str,
    pub resource_name: Option<&'a str>,
    pub scalar_index: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanFontAttestationResourceContext<'a> {
    pub resource_name: &'a str,
    pub base_font: Option<&'a str>,
    pub normalized_stem: Option<&'a str>,
    pub glyph_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanFontAttestationEvidenceContext<'a> {
    pub status: EvidenceStatus,
    pub expected_scalar_count: u64,
    pub candidate_scalar_count: Option<u64>,
    pub diagnostics: &'a [ScanFontAttestationDiagnosticContext<'a>],
    pub used_resources: &'a [ScanFontAttestationResourceContext<'a>],
}

pub struct ScanFontAttestationReportContext<'a> {
    pub page_index: usize,
    pub raster_sha256: &'a str,
    pub typography: &'a TypographyHypothesis,
    pub compiler_version: &'a str,
    pub source_sha256: &'a str,
    pub source_size_bytes: u64,
    pub pdf_sha256: &'a str,
    pub pdf_size_bytes: u64,
    pub attestation: ScanFontAttestationEvidenceContext<'a>,
    pub comparison_json: &'a str,
    pub artifact_published: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanFontAttestationReportRenderError {
    NonFinite { field: &'static str },
    PublishedWithoutPreservedAttestation,
}

impl fmt::Display for ScanFontAttestationReportRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { field } => {
                write!(formatter, "valor não finito ao serializar o campo {field}")
            }
            Self::PublishedWithoutPreservedAttestation => {
                formatter.write_str("artifact_published exige uma atestação preserved confirmada")
            }
        }
    }
}

impl std::error::Error for ScanFontAttestationReportRenderError {}

/// Renderiza o relatório v1 em ordem fixa, sem caminhos e sem LF terminal.
pub fn render_scan_font_attestation_report(
    context: &ScanFontAttestationReportContext<'_>,
) -> Result<String, ScanFontAttestationReportRenderError> {
    if context.artifact_published && context.attestation.status != EvidenceStatus::Preserved {
        return Err(ScanFontAttestationReportRenderError::PublishedWithoutPreservedAttestation);
    }

    let mut diagnostics: Vec<_> = context.attestation.diagnostics.iter().collect();
    diagnostics.sort_by(|left, right| {
        (left.code, left.resource_name, left.scalar_index).cmp(&(
            right.code,
            right.resource_name,
            right.scalar_index,
        ))
    });

    let mut used_resources: Vec<_> = context.attestation.used_resources.iter().collect();
    used_resources.sort_by(|left, right| left.resource_name.cmp(right.resource_name));

    let mut json = String::new();
    json.push_str("{\"schema\":\"decalque.scan-font-attestation\",\"schema_version\":1");

    write!(json, ",\"source\":{{\"page_index\":{}", context.page_index).unwrap();
    json.push_str(",\"raster_sha256\":");
    push_json_string(&mut json, context.raster_sha256);
    json.push('}');

    json.push_str(",\"hypothesis\":{\"font_family\":");
    push_json_string(&mut json, &context.typography.font_family);
    json.push_str(",\"font_size_pt\":");
    push_f64(
        &mut json,
        context.typography.size_pt,
        "hypothesis.font_size_pt",
    )?;
    json.push_str(",\"font_weight\":");
    push_json_string(&mut json, font_weight(context.typography.weight));
    json.push_str(",\"font_style\":");
    push_json_string(&mut json, font_style(context.typography.style));
    json.push_str(",\"tracking_pt\":");
    push_f64(
        &mut json,
        context.typography.tracking_pt,
        "hypothesis.tracking_pt",
    )?;
    json.push('}');

    json.push_str(",\"fallback_policy\":\"forbidden\",\"compiler_version\":");
    push_json_string(&mut json, context.compiler_version);
    json.push_str(",\"source_sha256\":");
    push_json_string(&mut json, context.source_sha256);
    write!(json, ",\"source_size_bytes\":{}", context.source_size_bytes).unwrap();
    json.push_str(",\"pdf_sha256\":");
    push_json_string(&mut json, context.pdf_sha256);
    write!(json, ",\"pdf_size_bytes\":{}", context.pdf_size_bytes).unwrap();

    json.push_str(",\"attestation\":{\"status\":");
    push_json_string(&mut json, evidence_status(context.attestation.status));
    write!(
        json,
        ",\"expected_scalar_count\":{}",
        context.attestation.expected_scalar_count
    )
    .unwrap();
    json.push_str(",\"candidate_scalar_count\":");
    push_optional_u64(&mut json, context.attestation.candidate_scalar_count);

    json.push_str(",\"diagnostics\":[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        push_separator(&mut json, index);
        json.push_str("{\"code\":");
        push_json_string(&mut json, diagnostic.code);
        json.push_str(",\"resource_name\":");
        push_optional_string(&mut json, diagnostic.resource_name);
        json.push_str(",\"scalar_index\":");
        push_optional_u64(&mut json, diagnostic.scalar_index);
        json.push('}');
    }
    json.push(']');

    json.push_str(",\"used_resources\":[");
    for (index, resource) in used_resources.iter().enumerate() {
        push_separator(&mut json, index);
        json.push_str("{\"resource_name\":");
        push_json_string(&mut json, resource.resource_name);
        json.push_str(",\"base_font\":");
        push_optional_string(&mut json, resource.base_font);
        json.push_str(",\"normalized_stem\":");
        push_optional_string(&mut json, resource.normalized_stem);
        write!(json, ",\"glyph_count\":{}}}", resource.glyph_count).unwrap();
    }
    json.push_str("]}");

    json.push_str(",\"comparison\":");
    json.push_str(context.comparison_json);
    write!(
        json,
        ",\"artifact_published\":{}}}",
        context.artifact_published
    )
    .unwrap();

    Ok(json)
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

fn evidence_status(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Preserved => "preserved",
        EvidenceStatus::Violated => "violated",
        EvidenceStatus::Unknown => "unknown",
    }
}

fn push_f64(
    json: &mut String,
    value: f64,
    field: &'static str,
) -> Result<(), ScanFontAttestationReportRenderError> {
    if !value.is_finite() {
        return Err(ScanFontAttestationReportRenderError::NonFinite { field });
    }
    write!(json, "{value}").unwrap();
    Ok(())
}

fn push_optional_string(json: &mut String, value: Option<&str>) {
    match value {
        Some(value) => push_json_string(json, value),
        None => json.push_str("null"),
    }
}

fn push_optional_u64(json: &mut String, value: Option<u64>) {
    match value {
        Some(value) => write!(json, "{value}").unwrap(),
        None => json.push_str("null"),
    }
}

fn push_separator(json: &mut String, index: usize) {
    if index != 0 {
        json.push(',');
    }
}
