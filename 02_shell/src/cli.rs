//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cli-digital-compare.md
//! @layer L2
//! @updated 2026-09-14

use decalque_core::ComparisonReport;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub const USAGE: &str = "uso: decalque <referencia.pdf> <candidato.pdf> [--page <indice>]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub reference: PathBuf,
    pub candidate: PathBuf,
    pub page_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliParseError {
    HelpRequested,
    InvalidUsage(String),
}

impl fmt::Display for CliParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpRequested => formatter.write_str(USAGE),
            Self::InvalidUsage(message) => write!(formatter, "{message}\n{USAGE}"),
        }
    }
}

pub fn parse_args<I>(args: I) -> Result<CliArgs, CliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == OsStr::new("--help") || arg == OsStr::new("-h"))
    {
        return Err(CliParseError::HelpRequested);
    }
    if args.len() != 2 && args.len() != 4 {
        return Err(CliParseError::InvalidUsage(
            "esperados dois caminhos e, opcionalmente, --page <indice>".to_string(),
        ));
    }
    let page_index = if args.len() == 4 {
        if args[2] != OsStr::new("--page") {
            return Err(CliParseError::InvalidUsage(format!(
                "opção desconhecida: {}",
                args[2].to_string_lossy()
            )));
        }
        args[3]
            .to_str()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| CliParseError::InvalidUsage("índice de página inválido".to_string()))?
    } else {
        0
    };
    Ok(CliArgs {
        reference: PathBuf::from(&args[0]),
        candidate: PathBuf::from(&args[1]),
        page_index,
    })
}

pub fn render_report(report: &ComparisonReport<'_>) -> String {
    format!(
        "pares: {}\nnão emparelhados: A={} B={}\ncobertura: A={}/{} B={}/{}\nmediana |dx|: {} pt\nmediana |dy|: {} pt\nmáximo |dx|: {} pt\nmáximo |dy|: {} pt",
        report.pairs.len(),
        report.unmatched_a.len(),
        report.unmatched_b.len(),
        report.coverage.matched_a,
        report.coverage.total_a,
        report.coverage.matched_b,
        report.coverage.total_b,
        metric(report.median_abs_dx),
        metric(report.median_abs_dy),
        metric(report.max_abs_dx),
        metric(report.max_abs_dy),
    )
}

fn metric(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |value| format!("{value:.3}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use decalque_core::entities::PageRotation;
    use decalque_core::{compare, DocumentGeometry, MeasurementResolution, PageGeometry};

    #[test]
    fn parseia_default_e_pagina_explicita() {
        let default = parse_args(["a.pdf", "b.pdf"].map(OsString::from)).unwrap();
        assert_eq!(default.page_index, 0);
        let explicit = parse_args(["a.pdf", "b.pdf", "--page", "3"].map(OsString::from)).unwrap();
        assert_eq!(explicit.page_index, 3);
    }

    #[test]
    fn rejeita_opcao_e_indice_invalidos() {
        assert!(matches!(
            parse_args(["a", "b", "--pages", "1"].map(OsString::from)),
            Err(CliParseError::InvalidUsage(_))
        ));
        assert!(matches!(
            parse_args(["a", "b", "--page", "x"].map(OsString::from)),
            Err(CliParseError::InvalidUsage(_))
        ));
    }

    #[test]
    fn relatorio_sem_pares_exibe_na_e_cobertura() {
        let document = DocumentGeometry {
            page: PageGeometry {
                width: 100.0,
                height: 100.0,
                origin: (0.0, 0.0),
                rotation: PageRotation::Deg0,
                user_unit: 1.0,
            },
            glyphs: Vec::new(),
            diagnostics: Vec::new(),
        };
        let report = compare(
            &document,
            &document,
            &MeasurementResolution::Absolute { tolerance_pt: 0.5 },
        );
        let rendered = render_report(&report);
        assert!(rendered.contains("cobertura: A=0/0 B=0/0"));
        assert!(rendered.contains("mediana |dx|: n/a pt"));
        assert!(!rendered.contains("mediana |dx|: 0.000 pt"));
    }
}
