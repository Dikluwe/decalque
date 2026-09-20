use decalque_core::{
    ConfidenceRequirement, ScanComparisonPolicy, ScanGranularity, TextNormalization,
};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub const SCAN_OBSERVATION_USAGE: &str = concat!(
    "uso: decalque scan-observation <observacao.json> <candidato.pdf> ",
    "--raster <raster> ",
    "--granularity <line|word> --horizontal-tolerance-pt <numero> ",
    "--baseline-tolerance-pt <numero> ",
    "[--min-text-confidence <0..1>] [--min-geometry-confidence <0..1>]\n",
    "\n",
    "O índice zero-based da página vem de source.page_index no artefato.\n",
    "unknown permanece inconclusivo; código 0 significa execução concluída, não paridade."
);

#[derive(Debug, Clone, PartialEq)]
pub struct ScanObservationCliArgs {
    pub observation: PathBuf,
    pub candidate: PathBuf,
    pub raster: PathBuf,
    pub policy: ScanComparisonPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanObservationCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanObservationCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_OBSERVATION_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_OBSERVATION_USAGE}")
            }
        }
    }
}

/// Limites de produto que L4 traduz para o tipo de fronteira de L3.
///
/// Eles são deliberadamente explícitos em L2: a v1 não oferece flags para
/// elevá-los e L3 não escolhe limites escondidos para entrada não confiável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanObservationInputLimits {
    pub max_input_bytes: usize,
    pub max_units: usize,
    pub max_provenance_records: usize,
    pub max_geometry_points: usize,
    pub max_total_text_bytes: usize,
}

pub const SCAN_OBSERVATION_INPUT_LIMITS: ScanObservationInputLimits = ScanObservationInputLimits {
    max_input_bytes: 16 * 1024 * 1024,
    max_units: 100_000,
    max_provenance_records: 10_000,
    max_geometry_points: 1_000_000,
    max_total_text_bytes: 8 * 1024 * 1024,
};

/// Faz parsing completo do subcomando antes de qualquer I/O de domínio.
pub fn parse_scan_observation_args<I>(
    args: I,
) -> Result<ScanObservationCliArgs, ScanObservationCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanObservationCliParseError::HelpRequested);
    }

    if args.len() < 2 {
        return Err(invalid_usage(
            "esperados os caminhos da observação e do PDF candidato",
        ));
    }

    let observation = PathBuf::from(&args[0]);
    let candidate = PathBuf::from(&args[1]);
    let mut raster = None;
    let mut granularity = None;
    let mut horizontal_tolerance_pt = None;
    let mut baseline_tolerance_pt = None;
    let mut min_text_confidence = None;
    let mut min_geometry_confidence = None;

    let mut index = 2;
    while index < args.len() {
        let option = &args[index];
        if !is_known_option(option) {
            return Err(invalid_usage(format!(
                "opção desconhecida: {}",
                option.to_string_lossy()
            )));
        }
        let value = args.get(index + 1).ok_or_else(|| {
            invalid_usage(format!("valor ausente para {}", option.to_string_lossy()))
        })?;

        if option == OsStr::new("--raster") {
            reject_repeated(option, raster.is_some())?;
            raster = Some(PathBuf::from(value));
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
    let granularity = granularity.ok_or_else(|| missing_option("--granularity"))?;
    let horizontal_tolerance_pt =
        horizontal_tolerance_pt.ok_or_else(|| missing_option("--horizontal-tolerance-pt"))?;
    let baseline_tolerance_pt =
        baseline_tolerance_pt.ok_or_else(|| missing_option("--baseline-tolerance-pt"))?;

    Ok(ScanObservationCliArgs {
        observation,
        candidate,
        raster,
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
        || option == OsStr::new("--granularity")
        || option == OsStr::new("--horizontal-tolerance-pt")
        || option == OsStr::new("--baseline-tolerance-pt")
        || option == OsStr::new("--min-text-confidence")
        || option == OsStr::new("--min-geometry-confidence")
}

fn parse_granularity(value: &OsStr) -> Result<ScanGranularity, ScanObservationCliParseError> {
    if value == OsStr::new("line") {
        Ok(ScanGranularity::Line)
    } else if value == OsStr::new("word") {
        Ok(ScanGranularity::Word)
    } else {
        Err(invalid_usage(format!(
            "valor inválido para --granularity: {}",
            value.to_string_lossy()
        )))
    }
}

fn parse_tolerance(option: &OsStr, value: &OsStr) -> Result<f64, ScanObservationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if parsed < 0.0 {
        return Err(invalid_number(option, value));
    }
    Ok(parsed)
}

fn parse_confidence(option: &OsStr, value: &OsStr) -> Result<f64, ScanObservationCliParseError> {
    let parsed = parse_finite_number(option, value)?;
    if !(0.0..=1.0).contains(&parsed) {
        return Err(invalid_number(option, value));
    }
    Ok(parsed)
}

fn parse_finite_number(option: &OsStr, value: &OsStr) -> Result<f64, ScanObservationCliParseError> {
    let parsed = value
        .to_str()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid_number(option, value))?;
    Ok(parsed)
}

fn reject_repeated(
    option: &OsStr,
    already_present: bool,
) -> Result<(), ScanObservationCliParseError> {
    if already_present {
        Err(invalid_usage(format!(
            "opção repetida: {}",
            option.to_string_lossy()
        )))
    } else {
        Ok(())
    }
}

fn invalid_number(option: &OsStr, value: &OsStr) -> ScanObservationCliParseError {
    invalid_usage(format!(
        "valor inválido para {}: {}",
        option.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn missing_option(option: &str) -> ScanObservationCliParseError {
    invalid_usage(format!("opção obrigatória ausente: {option}"))
}

fn invalid_usage(message: impl Into<String>) -> ScanObservationCliParseError {
    ScanObservationCliParseError::InvalidUsage(message.into())
}
