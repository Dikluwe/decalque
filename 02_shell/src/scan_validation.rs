use decalque_core::ScanObservation;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub const SCAN_OBSERVATION_VALIDATION_USAGE: &str = concat!(
    "uso: decalque validate-scan-observation <observacao.json> --raster <raster>\n",
    "\n",
    "Valida o contrato e a identidade SHA-256/tipo/dimensões do raster.\n",
    "Não prova a exatidão do OCR nem paridade com um PDF candidato."
);

pub const SCAN_OBSERVATION_RASTER_MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanObservationValidationCliArgs {
    pub observation: PathBuf,
    pub raster: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanObservationValidationCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanObservationValidationCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_OBSERVATION_VALIDATION_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_OBSERVATION_VALIDATION_USAGE}")
            }
        }
    }
}

pub fn parse_scan_observation_validation_args<I>(
    args: I,
) -> Result<ScanObservationValidationCliArgs, ScanObservationValidationCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanObservationValidationCliParseError::HelpRequested);
    }
    let Some(observation) = args.first() else {
        return Err(invalid_usage("caminho da observação ausente"));
    };
    if observation.to_string_lossy().starts_with('-') {
        return Err(invalid_usage(format!(
            "opção desconhecida: {}",
            observation.to_string_lossy()
        )));
    }

    let mut raster = None;
    let mut index = 1;
    while index < args.len() {
        let option = &args[index];
        if option != OsStr::new("--raster") {
            return Err(invalid_usage(format!(
                "opção desconhecida: {}",
                option.to_string_lossy()
            )));
        }
        if raster.is_some() {
            return Err(invalid_usage("opção repetida: --raster"));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| invalid_usage("valor ausente para --raster"))?;
        raster = Some(PathBuf::from(value));
        index += 2;
    }

    Ok(ScanObservationValidationCliArgs {
        observation: PathBuf::from(observation),
        raster: raster.ok_or_else(|| invalid_usage("opção obrigatória ausente: --raster"))?,
    })
}

pub fn render_scan_observation_validation_report(observation: &ScanObservation) -> String {
    format!(
        concat!(
            "{{\"schema\":\"decalque.scan-observation-validation\",",
            "\"schema_version\":1,\"status\":\"valid\",",
            "\"scope\":\"contract-and-raster-identity\",",
            "\"source\":{{\"page_index\":{},\"raster_sha256\":\"{}\",",
            "\"media_type\":\"{}\",\"width_px\":{},\"height_px\":{}}},",
            "\"counts\":{{\"units\":{},\"provenance_records\":{},\"diagnostics\":{}}}}}"
        ),
        observation.source.page_index,
        observation.source.raster.sha256,
        observation.source.raster.media_type,
        observation.source.raster.width_px,
        observation.source.raster.height_px,
        observation.units.len(),
        observation.provenance.len(),
        observation.diagnostics.len(),
    )
}

fn invalid_usage(message: impl Into<String>) -> ScanObservationValidationCliParseError {
    ScanObservationValidationCliParseError::InvalidUsage(message.into())
}
