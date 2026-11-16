use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Portage Rust de l'export MusicXML de MuseScore.
#[derive(Debug, Parser)]
#[command(name = "mscz2xml", version, about)]
struct Cli {
    /// Fichier d'entrée (.mscz ou .mscx)
    input: PathBuf,

    /// Fichier MusicXML de sortie (export non encore implémenté)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let bytes = match std::fs::read(&cli.input) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("mscz2xml : lecture de {} impossible : {e}", cli.input.display());
            return ExitCode::FAILURE;
        }
    };

    let container = match mscz2xml::MsczContainer::from_bytes(&bytes) {
        Ok(container) => container,
        Err(e) => {
            eprintln!("mscz2xml : {} : {e}", cli.input.display());
            return ExitCode::FAILURE;
        }
    };

    let version = match container.version() {
        Ok(version) => version,
        Err(e) => {
            eprintln!("mscz2xml : {} : {e}", cli.input.display());
            return ExitCode::FAILURE;
        }
    };

    println!("fichier principal : {}", container.main_mscx_name());
    println!("version mscx     : {version} ({})", version.as_u32());

    if let Some(output) = &cli.output {
        println!(
            "sortie demandée  : {} (export MusicXML non implémenté à cette étape)",
            output.display()
        );
    }

    ExitCode::SUCCESS
}
