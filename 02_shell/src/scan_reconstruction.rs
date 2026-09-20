use decalque_core::{FontStyleHypothesis, FontWeightHypothesis, TypographyHypothesis};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub const SCAN_RECONSTRUCTION_USAGE: &str = concat!(
    "uso: decalque reconstruct-scan-lines <observacao.json> ",
    "--raster <raster> --font-family <familia> --font-size-pt <numero> ",
    "[--font-weight <regular|bold>] ",
    "[--font-style <normal|italic|oblique>] [--tracking-pt <numero>]\n",
    "\n",
    "Padrões: --font-weight regular, --font-style normal e --tracking-pt 0."
);

#[derive(Debug, Clone, PartialEq)]
pub struct ScanReconstructionCliArgs {
    pub observation: PathBuf,
    pub raster: PathBuf,
    pub typography: TypographyHypothesis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanReconstructionCliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for ScanReconstructionCliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(SCAN_RECONSTRUCTION_USAGE),
            Self::InvalidUsage(message) => {
                write!(formatter, "{message}\n{SCAN_RECONSTRUCTION_USAGE}")
            }
        }
    }
}

/// Faz o parsing completo da hipótese antes de qualquer I/O de domínio.
pub fn parse_scan_reconstruction_args<I>(
    args: I,
) -> Result<ScanReconstructionCliArgs, ScanReconstructionCliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(ScanReconstructionCliParseError::HelpRequested);
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
    let mut font_family = None;
    let mut font_size_pt = None;
    let mut font_weight = None;
    let mut font_style = None;
    let mut tracking_pt = None;

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
            if value.is_empty() {
                return Err(invalid_value(option, value));
            }
            raster = Some(PathBuf::from(value));
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
        }

        index += 2;
    }

    let raster = raster.ok_or_else(|| missing_option("--raster"))?;
    let font_family = font_family.ok_or_else(|| missing_option("--font-family"))?;
    let size_pt = font_size_pt.ok_or_else(|| missing_option("--font-size-pt"))?;

    Ok(ScanReconstructionCliArgs {
        observation: PathBuf::from(observation),
        raster,
        typography: TypographyHypothesis {
            font_family,
            size_pt,
            weight: font_weight.unwrap_or(FontWeightHypothesis::Regular),
            style: font_style.unwrap_or(FontStyleHypothesis::Normal),
            tracking_pt: tracking_pt.unwrap_or(0.0),
        },
    })
}

fn is_known_option(option: &OsStr) -> bool {
    option == OsStr::new("--raster")
        || option == OsStr::new("--font-family")
        || option == OsStr::new("--font-size-pt")
        || option == OsStr::new("--font-weight")
        || option == OsStr::new("--font-style")
        || option == OsStr::new("--tracking-pt")
}

fn looks_like_option(value: &OsStr) -> bool {
    value.to_string_lossy().starts_with("--")
}

fn parse_font_family(value: &OsStr) -> Result<String, ScanReconstructionCliParseError> {
    let family = value
        .to_str()
        .filter(|family| !family.trim().is_empty())
        .ok_or_else(|| invalid_value(OsStr::new("--font-family"), value))?;
    Ok(family.to_string())
}

fn parse_font_size(value: &OsStr) -> Result<f64, ScanReconstructionCliParseError> {
    let option = OsStr::new("--font-size-pt");
    let size = parse_finite_number(option, value)?;
    if size <= 0.0 {
        return Err(invalid_value(option, value));
    }
    Ok(size)
}

fn parse_font_weight(
    value: &OsStr,
) -> Result<FontWeightHypothesis, ScanReconstructionCliParseError> {
    if value == OsStr::new("regular") {
        Ok(FontWeightHypothesis::Regular)
    } else if value == OsStr::new("bold") {
        Ok(FontWeightHypothesis::Bold)
    } else {
        Err(invalid_value(OsStr::new("--font-weight"), value))
    }
}

fn parse_font_style(value: &OsStr) -> Result<FontStyleHypothesis, ScanReconstructionCliParseError> {
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

fn parse_finite_number(
    option: &OsStr,
    value: &OsStr,
) -> Result<f64, ScanReconstructionCliParseError> {
    value
        .to_str()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid_value(option, value))
}

fn reject_repeated(
    option: &OsStr,
    already_present: bool,
) -> Result<(), ScanReconstructionCliParseError> {
    if already_present {
        Err(invalid_usage(format!(
            "opção repetida: {}",
            option.to_string_lossy()
        )))
    } else {
        Ok(())
    }
}

fn missing_value(option: &OsStr) -> ScanReconstructionCliParseError {
    invalid_usage(format!("valor ausente para {}", option.to_string_lossy()))
}

fn invalid_value(option: &OsStr, value: &OsStr) -> ScanReconstructionCliParseError {
    invalid_usage(format!(
        "valor inválido para {}: {}",
        option.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn missing_option(option: &str) -> ScanReconstructionCliParseError {
    invalid_usage(format!("opção obrigatória ausente: {option}"))
}

fn invalid_usage(message: impl Into<String>) -> ScanReconstructionCliParseError {
    ScanReconstructionCliParseError::InvalidUsage(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn required_args() -> Vec<OsString> {
        [
            "observation.json",
            "--raster",
            "page.pgm",
            "--font-family",
            "Libertinus Serif",
            "--font-size-pt",
            "10",
        ]
        .map(OsString::from)
        .into()
    }

    #[test]
    fn declares_defaults_and_accepts_explicit_hypothesis() {
        let parsed = parse_scan_reconstruction_args(required_args()).unwrap();
        assert_eq!(parsed.observation, PathBuf::from("observation.json"));
        assert_eq!(parsed.raster, PathBuf::from("page.pgm"));
        assert_eq!(parsed.typography.font_family, "Libertinus Serif");
        assert_eq!(parsed.typography.size_pt, 10.0);
        assert_eq!(parsed.typography.weight, FontWeightHypothesis::Regular);
        assert_eq!(parsed.typography.style, FontStyleHypothesis::Normal);
        assert_eq!(parsed.typography.tracking_pt, 0.0);

        let mut explicit = required_args();
        explicit.extend(
            [
                "--font-weight",
                "bold",
                "--font-style",
                "oblique",
                "--tracking-pt",
                "-0.25",
            ]
            .map(OsString::from),
        );
        let parsed = parse_scan_reconstruction_args(explicit).unwrap();
        assert_eq!(parsed.typography.weight, FontWeightHypothesis::Bold);
        assert_eq!(parsed.typography.style, FontStyleHypothesis::Oblique);
        assert_eq!(parsed.typography.tracking_pt, -0.25);
    }

    #[test]
    fn rejects_missing_repeated_and_unknown_options() {
        let missing_family = [
            "missing.json",
            "--raster",
            "missing.pgm",
            "--font-size-pt",
            "10",
        ]
        .map(OsString::from);
        let error = parse_scan_reconstruction_args(missing_family).unwrap_err();
        assert!(error.to_string().contains("--font-family"));

        let mut repeated = required_args();
        repeated.extend(["--raster", "other.pgm"].map(OsString::from));
        let error = parse_scan_reconstruction_args(repeated).unwrap_err();
        assert!(error.to_string().contains("repetida"));

        let mut unknown = required_args();
        unknown.extend(["--candidate", "candidate.pdf"].map(OsString::from));
        let error = parse_scan_reconstruction_args(unknown).unwrap_err();
        assert!(error.to_string().contains("--candidate"));
    }

    #[test]
    fn rejects_invalid_hypothesis_values() {
        for (option, value) in [
            ("--font-size-pt", "0"),
            ("--font-size-pt", "NaN"),
            ("--tracking-pt", "inf"),
            ("--font-weight", "heavy"),
            ("--font-style", "slanted-ish"),
        ] {
            let mut args = required_args();
            if let Some(position) = args.iter().position(|argument| argument == option) {
                args[position + 1] = OsString::from(value);
            } else {
                args.extend([option, value].map(OsString::from));
            }

            let error = parse_scan_reconstruction_args(args).unwrap_err();
            assert!(error.to_string().contains(option), "{option}={value}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let observation = OsString::from_vec(b"observation-\xff.json".to_vec());
        let raster = OsString::from_vec(b"page-\xfe.pgm".to_vec());
        let args = vec![
            observation.clone(),
            OsString::from("--raster"),
            raster.clone(),
            OsString::from("--font-family"),
            OsString::from("Font"),
            OsString::from("--font-size-pt"),
            OsString::from("10"),
        ];

        let parsed = parse_scan_reconstruction_args(args).unwrap();
        assert_eq!(parsed.observation.as_os_str(), observation.as_os_str());
        assert_eq!(parsed.raster.as_os_str(), raster.as_os_str());
    }
}
