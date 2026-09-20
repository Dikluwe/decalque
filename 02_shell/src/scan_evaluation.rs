use crate::scan_report::push_json_string;
use decalque_core::{
    ConfidenceRequirement, FontStyleHypothesis, FontWeightHypothesis, ScanComparisonPolicy,
    ScanGranularity, TextNormalization, TypographyHypothesis,
};
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

pub const SCAN_EVALUATION_USAGE: &str = concat!(
    "uso: decalque evaluate-scan-lines <observacao.json> ",
    "--raster <raster> --output-pdf <candidato.pdf> ",
    "--font-family <familia> --font-size-pt <numero> ",
    "[--font-weight <regular|bold>] ",
    "[--font-style <normal|italic|oblique>] [--tracking-pt <numero>] ",
    "--granularity <line|word> --horizontal-tolerance-pt <numero> ",
    "--baseline-tolerance-pt <numero> ",
    "[--min-text-confidence <0..1>] [--min-geometry-confidence <0..1>] ",
    "[--typst-bin <caminho>]\n",
    "\n",
    "Padrões: --font-weight regular, --font-style normal, --tracking-pt 0 e ",
    "--typst-bin typst."
);

pub const SCAN_EVALUATION_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub const SCAN_EVALUATION_MAX_PDF_BYTES: usize = 128 * 1024 * 1024;
pub const SCAN_EVALUATION_MAX_STDERR_BYTES: usize = 1024 * 1024;
pub const SCAN_EVALUATION_MAX_VERSION_BYTES: usize = 64 * 1024;
pub const SCAN_EVALUATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Limites fechados da avaliação v1. Não há opções de CLI para elevá-los.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanEvaluationLimits {
    pub source_max_bytes: usize,
    pub pdf_max_bytes: usize,
    pub stderr_max_bytes: usize,
    pub version_max_bytes: usize,
    pub timeout: Duration,
}

pub const SCAN_EVALUATION_LIMITS: ScanEvaluationLimits = ScanEvaluationLimits {
    source_max_bytes: SCAN_EVALUATION_MAX_SOURCE_BYTES,
    pdf_max_bytes: SCAN_EVALUATION_MAX_PDF_BYTES,
    stderr_max_bytes: SCAN_EVALUATION_MAX_STDERR_BYTES,
    version_max_bytes: SCAN_EVALUATION_MAX_VERSION_BYTES,
    timeout: SCAN_EVALUATION_TIMEOUT,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ScanEvaluationCliArgs {
    pub observation: PathBuf,
    pub raster: PathBuf,
    pub output_pdf: PathBuf,
    pub typst_bin: PathBuf,
    pub typography: TypographyHypothesis,
    pub policy: ScanComparisonPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvaluationCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanEvaluationCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_EVALUATION_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_EVALUATION_USAGE}")
            }
        }
    }
}

/// Analisa integralmente o subcomando antes de qualquer I/O de domínio.
pub fn parse_scan_evaluation_args<I>(
    args: I,
) -> Result<ScanEvaluationCliArgs, ScanEvaluationCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanEvaluationCliParseError::HelpRequested);
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
    let mut granularity = None;
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
        } else if option == OsStr::new("--granularity") {
            reject_repeated(option, granularity.is_some())?;
            granularity = Some(parse_granularity(value)?);
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
    let granularity = granularity.ok_or_else(|| missing_option("--granularity"))?;
    let horizontal_tolerance_pt =
        horizontal_tolerance_pt.ok_or_else(|| missing_option("--horizontal-tolerance-pt"))?;
    let baseline_tolerance_pt =
        baseline_tolerance_pt.ok_or_else(|| missing_option("--baseline-tolerance-pt"))?;

    Ok(ScanEvaluationCliArgs {
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
            granularity,
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
        || option == OsStr::new("--granularity")
        || option == OsStr::new("--horizontal-tolerance-pt")
        || option == OsStr::new("--baseline-tolerance-pt")
        || option == OsStr::new("--min-text-confidence")
        || option == OsStr::new("--min-geometry-confidence")
        || option == OsStr::new("--typst-bin")
}

fn looks_like_option(value: &OsStr) -> bool {
    value.to_string_lossy().starts_with("--")
}

fn parse_path(option: &OsStr, value: &OsStr) -> Result<PathBuf, ScanEvaluationCliParseError> {
    if value.is_empty() {
        Err(invalid_value(option, value))
    } else {
        Ok(PathBuf::from(value))
    }
}

fn parse_font_family(value: &OsStr) -> Result<String, ScanEvaluationCliParseError> {
    let family = value
        .to_str()
        .filter(|family| !family.trim().is_empty())
        .ok_or_else(|| invalid_value(OsStr::new("--font-family"), value))?;
    Ok(family.to_string())
}

fn parse_font_size(value: &OsStr) -> Result<f64, ScanEvaluationCliParseError> {
    let option = OsStr::new("--font-size-pt");
    let size = parse_finite_number(option, value)?;
    if size <= 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(size)
}

fn parse_font_weight(value: &OsStr) -> Result<FontWeightHypothesis, ScanEvaluationCliParseError> {
    if value == OsStr::new("regular") {
        Ok(FontWeightHypothesis::Regular)
    } else if value == OsStr::new("bold") {
        Ok(FontWeightHypothesis::Bold)
    } else {
        Err(invalid_value(OsStr::new("--font-weight"), value))
    }
}

fn parse_font_style(value: &OsStr) -> Result<FontStyleHypothesis, ScanEvaluationCliParseError> {
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

fn parse_granularity(value: &OsStr) -> Result<ScanGranularity, ScanEvaluationCliParseError> {
    if value == OsStr::new("line") {
        Ok(ScanGranularity::Line)
    } else if value == OsStr::new("word") {
        Ok(ScanGranularity::Word)
    } else {
        Err(invalid_value(OsStr::new("--granularity"), value))
    }
}

fn parse_tolerance(option: &OsStr, value: &OsStr) -> Result<f64, ScanEvaluationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if parsed < 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_confidence(option: &OsStr, value: &OsStr) -> Result<f64, ScanEvaluationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if !(0.0..=1.0).contains(&parsed) {
        return Err(invalid_value(option, value));
    }
    Ok(parsed)
}

fn parse_finite_number(option: &OsStr, value: &OsStr) -> Result<f64, ScanEvaluationCliParseError> {
    value
        .to_str()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid_value(option, value))
}

fn reject_repeated(
    option: &OsStr,
    already_present: bool,
) -> Result<(), ScanEvaluationCliParseError> {
    if already_present {
        Err(invalid_usage(format!(
            "opção repetida: {}",
            option.to_string_lossy()
        )))
    } else {
        Ok(())
    }
}

fn missing_value(option: &OsStr) -> ScanEvaluationCliParseError {
    invalid_usage(format!("valor ausente para {}", option.to_string_lossy()))
}

fn invalid_value(option: &OsStr, value: &OsStr) -> ScanEvaluationCliParseError {
    invalid_usage(format!(
        "valor inválido para {}: {}",
        option.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn missing_option(option: &str) -> ScanEvaluationCliParseError {
    invalid_usage(format!("opção obrigatória ausente: {option}"))
}

fn invalid_usage(message: impl Into<String>) -> ScanEvaluationCliParseError {
    ScanEvaluationCliParseError::InvalidUsage(message.into())
}

/// Dados externos ao relatório comparativo que vinculam uma iteração completa.
pub struct ScanEvaluationReportContext<'a> {
    pub page_index: usize,
    pub raster_sha256: &'a str,
    pub typography: &'a TypographyHypothesis,
    pub compiler_version: &'a str,
    pub source_sha256: &'a str,
    pub source_size_bytes: u64,
    pub pdf_sha256: &'a str,
    pub pdf_size_bytes: u64,
    pub comparison_json: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEvaluationReportRenderError {
    field: &'static str,
}

impl fmt::Display for ScanEvaluationReportRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "valor não finito ao serializar o campo {}",
            self.field
        )
    }
}

impl std::error::Error for ScanEvaluationReportRenderError {}

/// Renderiza o envelope v1 em ordem fixa e incorpora o relatório comparativo
/// já renderizado como objeto JSON bruto, sem reinterpretar seus estados.
pub fn render_scan_evaluation_report(
    context: &ScanEvaluationReportContext<'_>,
) -> Result<String, ScanEvaluationReportRenderError> {
    let mut json = String::new();
    json.push_str("{\"schema\":\"decalque.scan-reconstruction-evaluation\",\"schema_version\":1");

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
    push_json_string(
        &mut json,
        match context.typography.weight {
            FontWeightHypothesis::Regular => "regular",
            FontWeightHypothesis::Bold => "bold",
        },
    );
    json.push_str(",\"font_style\":");
    push_json_string(
        &mut json,
        match context.typography.style {
            FontStyleHypothesis::Normal => "normal",
            FontStyleHypothesis::Italic => "italic",
            FontStyleHypothesis::Oblique => "oblique",
        },
    );
    json.push_str(",\"tracking_pt\":");
    push_f64(
        &mut json,
        context.typography.tracking_pt,
        "hypothesis.tracking_pt",
    )?;
    json.push('}');

    json.push_str(",\"compiler_version\":");
    push_json_string(&mut json, context.compiler_version);
    json.push_str(",\"source_sha256\":");
    push_json_string(&mut json, context.source_sha256);
    write!(json, ",\"source_size_bytes\":{}", context.source_size_bytes).unwrap();
    json.push_str(",\"pdf_sha256\":");
    push_json_string(&mut json, context.pdf_sha256);
    write!(json, ",\"pdf_size_bytes\":{}", context.pdf_size_bytes).unwrap();

    json.push_str(",\"comparison\":");
    json.push_str(context.comparison_json);
    json.push('}');

    Ok(json)
}

fn push_f64(
    json: &mut String,
    value: f64,
    field: &'static str,
) -> Result<(), ScanEvaluationReportRenderError> {
    if !value.is_finite() {
        return Err(ScanEvaluationReportRenderError { field });
    }
    write!(json, "{value}").unwrap();
    Ok(())
}
