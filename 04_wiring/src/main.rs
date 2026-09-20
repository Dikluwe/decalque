//! Composição do binário `decalque` (L4).
//!
//! Sem cabeçalho de linhagem: nenhum prompt gera este ficheiro — é ficheiro
//! de composição (ADR 0003, excepção para ficheiros sem prompt de origem).

use decalque_core::compare;
use decalque_shell::{
    digital_to_digital_resolution, parse_args, parse_scan_evaluation_args,
    parse_scan_font_attestation_args, parse_scan_observation_args,
    parse_scan_observation_validation_args, parse_scan_reconstruction_args,
    parse_scan_typography_search_args, render_report, CliParseError, ScanEvaluationCliParseError,
    ScanFontAttestationCliParseError, ScanObservationCliParseError,
    ScanObservationValidationCliParseError, ScanReconstructionCliParseError,
    ScanTypographySearchCliParseError,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.first().map(OsString::as_os_str) == Some(OsStr::new("derive-scan-layout")) {
        run_derive_scan_layout_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("attest-scan-font")) {
        run_scan_font_attestation_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("fit-scan-lines")) {
        run_scan_typography_search_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("evaluate-scan-lines")) {
        run_scan_line_evaluation_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("reconstruct-scan-lines")) {
        run_scan_line_reconstruction_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("scan-observation")) {
        run_scan_command(args.into_iter().skip(1));
    } else if args.first().map(OsString::as_os_str) == Some(OsStr::new("validate-scan-observation"))
    {
        run_scan_validation_command(args.into_iter().skip(1));
    } else {
        run_digital_command(args);
    }
}

struct DeriveScanLayoutArgs {
    observation: PathBuf,
    raster: PathBuf,
    output_typst: Option<PathBuf>,
}

fn parse_derive_scan_layout_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<DeriveScanLayoutArgs, ()> {
    let mut observation = None;
    let mut raster = None;
    let mut output_typst = None;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        if argument == OsStr::new("--raster") {
            if raster.is_some() {
                return Err(());
            }
            let value = args.next().ok_or(())?;
            if is_scan_layout_option(&value) {
                return Err(());
            }
            raster = Some(PathBuf::from(value));
        } else if argument == OsStr::new("--output-typst") {
            if output_typst.is_some() {
                return Err(());
            }
            let value = args.next().ok_or(())?;
            if is_scan_layout_option(&value) {
                return Err(());
            }
            output_typst = Some(PathBuf::from(value));
        } else {
            if argument.as_encoded_bytes().first() == Some(&b'-') {
                return Err(());
            }
            if observation.replace(PathBuf::from(argument)).is_some() {
                return Err(());
            }
        }
    }

    Ok(DeriveScanLayoutArgs {
        observation: observation.ok_or(())?,
        raster: raster.ok_or(())?,
        output_typst,
    })
}

fn is_scan_layout_option(argument: &OsStr) -> bool {
    argument.as_encoded_bytes().first() == Some(&b'-')
}

struct InfraScanLayoutPublisher;

impl decalque::ScanLayoutPublisher for InfraScanLayoutPublisher {
    type Error = decalque_infra::PublishNewFileError;

    fn publish_new_file(&mut self, destination: &Path, bytes: &[u8]) -> Result<(), Self::Error> {
        decalque_infra::publish_new_file(destination, bytes)
    }
}

fn run_derive_scan_layout_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_derive_scan_layout_args(args) {
        Ok(args) => args,
        Err(()) => {
            eprintln!("derive-scan-layout: usage-error");
            std::process::exit(2);
        }
    };

    let product_limits = decalque_shell::SCAN_OBSERVATION_INPUT_LIMITS;
    let limits = decalque_infra::ScanObservationLimits {
        max_input_bytes: product_limits.max_input_bytes,
        max_units: product_limits.max_units,
        max_provenance_records: product_limits.max_provenance_records,
        max_geometry_points: product_limits.max_geometry_points,
        max_total_text_bytes: product_limits.max_total_text_bytes,
    };
    let observation = match decalque_infra::load_scan_observation_json(&args.observation, &limits) {
        Ok(observation) => observation,
        Err(error) => {
            eprintln!("derive-scan-layout: observation-error");
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = decalque_infra::bind_scan_observation_raster(
        &observation,
        &args.raster,
        decalque_shell::SCAN_OBSERVATION_RASTER_MAX_BYTES,
    ) {
        eprintln!("derive-scan-layout: raster-bind-error");
        eprintln!("{error}");
        std::process::exit(2);
    }

    let profile = match decalque_core::derive_scan_layout(&observation) {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("derive-scan-layout: observation-error");
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let rendered = decalque_shell::render_scan_layout_profile(&profile);
    let mut publisher = InfraScanLayoutPublisher;
    match decalque::finalize_scan_layout_profile(
        &rendered,
        args.output_typst.as_deref(),
        &mut publisher,
    ) {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("derive-scan-layout: publication-error");
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_font_attestation_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_font_attestation_args(args) {
        Ok(args) => args,
        Err(ScanFontAttestationCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_FONT_ATTESTATION_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_font_attestation(&args) {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_typography_search_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_typography_search_args(args) {
        Ok(args) => args,
        Err(ScanTypographySearchCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_TYPOGRAPHY_SEARCH_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_typography_search(&args) {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_line_evaluation_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_evaluation_args(args) {
        Ok(args) => args,
        Err(ScanEvaluationCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_EVALUATION_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_line_evaluation(&args) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_line_reconstruction_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_reconstruction_args(args) {
        Ok(args) => args,
        Err(ScanReconstructionCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_RECONSTRUCTION_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_line_reconstruction(&args) {
        Ok(source) => print!("{source}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_validation_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_observation_validation_args(args) {
        Ok(args) => args,
        Err(ScanObservationValidationCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_OBSERVATION_VALIDATION_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_observation_validation(&args) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_digital_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_args(args) {
        Ok(args) => args,
        Err(CliParseError::HelpRequested) => {
            println!("{}", decalque_shell::USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    let result = run_digital(&args);
    match result {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_scan_command(args: impl IntoIterator<Item = OsString>) {
    let args = match parse_scan_observation_args(args) {
        Ok(args) => args,
        Err(ScanObservationCliParseError::HelpRequested) => {
            println!("{}", decalque_shell::SCAN_OBSERVATION_USAGE);
            return;
        }
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    };

    match decalque::run_scan_observation_comparison(&args) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run_digital(args: &decalque_shell::CliArgs) -> Result<String, String> {
    let reference = decalque::load_materialized_page(&args.reference, args.page_index)
        .map_err(|error| format!("referência: {error}"))?;
    let candidate = decalque::load_materialized_page(&args.candidate, args.page_index)
        .map_err(|error| format!("candidato: {error}"))?;
    let report = compare(
        &reference.geometry,
        &candidate.geometry,
        &digital_to_digital_resolution(),
    );
    Ok(render_report(&report))
}
