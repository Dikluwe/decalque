//! Composição do binário `decalque` (L4).
//!
//! Sem cabeçalho de linhagem: nenhum prompt gera este ficheiro — é ficheiro
//! de composição (ADR 0003, excepção para ficheiros sem prompt de origem).

use decalque_core::compare;
use decalque_shell::{digital_to_digital_resolution, parse_args, render_report, CliParseError};

fn main() {
    let args = match parse_args(std::env::args_os().skip(1)) {
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

    let result = run(&args);
    match result {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("erro: {error}");
            std::process::exit(2);
        }
    }
}

fn run(args: &decalque_shell::CliArgs) -> Result<String, String> {
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
