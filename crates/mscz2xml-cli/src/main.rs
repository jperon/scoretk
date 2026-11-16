// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see the LICENSE file at the root of this repository or
// <https://www.gnu.org/licenses/gpl-3.0.html>.

//! CLI `mscz2xml` : convertit un `.mscz` ou un `.mscx` en MusicXML.
//!
//! Le format de sortie est déduit de l'extension du fichier de sortie
//! (`.musicxml`/`.xml` → MusicXML, `.mxl` → archive compressée). Sans `-o`, le
//! MusicXML est écrit sur la sortie standard.
//!
//! La date d'encodage est celle du jour en UTC, calculée à partir de
//! `std::time` (aucune dépendance supplémentaire) ; `--encoding-date` la fige
//! pour des sorties reproductibles.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};

/// Version affichée par `--version` : en plus du numéro, la licence et
/// l'absence de garantie (GPL §0, « Appropriate Legal Notices »).
const VERSION_LINE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nLicence : GNU GPL version 3 seulement (GPL-3.0-only), sans aucune garantie ;",
    "\nvoir le fichier LICENSE ou <https://www.gnu.org/licenses/gpl-3.0.html>."
);

/// Portage Rust de l'export MusicXML de MuseScore.
#[derive(Debug, Parser)]
#[command(name = "mscz2xml", version = VERSION_LINE, about)]
struct Cli {
    /// Fichier d'entrée (.mscz ou .mscx)
    input: std::path::PathBuf,

    /// Fichier de sortie (.musicxml, .xml ou .mxl) ; sortie standard si absent
    #[arg(short, long, value_name = "SORTIE")]
    output: Option<std::path::PathBuf>,

    /// Date d'encodage au format AAAA-MM-JJ (par défaut : aujourd'hui en UTC)
    #[arg(long, value_name = "AAAA-MM-JJ", value_parser = parse_encoding_date)]
    encoding_date: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let format = match &cli.output {
        Some(path) => match output_format(path) {
            Ok(format) => format,
            Err(message) => {
                eprintln!("mscz2xml : {message}");
                return ExitCode::FAILURE;
            }
        },
        None => OutputFormat::MusicXml,
    };

    let input = match std::fs::read(&cli.input) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "mscz2xml : lecture de {} impossible : {error}",
                cli.input.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let options = ConvertOptions {
        format,
        export: ExportOptions {
            encoding_date: cli.encoding_date.unwrap_or_else(today_utc),
            ..ExportOptions::default()
        },
    };

    let bytes = match convert(&input, &options) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("mscz2xml : {} : {error}", cli.input.display());
            return ExitCode::FAILURE;
        }
    };

    match &cli.output {
        Some(path) => {
            if let Err(error) = std::fs::write(path, &bytes) {
                eprintln!(
                    "mscz2xml : écriture de {} impossible : {error}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        }
        None => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            if let Err(error) = lock.write_all(&bytes).and_then(|()| lock.flush()) {
                eprintln!("mscz2xml : écriture sur la sortie standard impossible : {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

/// Déduit le format de sortie de l'extension du fichier.
fn output_format(path: &Path) -> Result<OutputFormat, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("musicxml" | "xml") => Ok(OutputFormat::MusicXml),
        Some("mxl") => Ok(OutputFormat::Mxl),
        _ => Err(format!(
            "extension de sortie non reconnue pour {} : attendu .musicxml, .xml ou .mxl",
            path.display()
        )),
    }
}

/// Valide `--encoding-date` : `AAAA-MM-JJ`, mois et jour plausibles.
fn parse_encoding_date(text: &str) -> Result<String, String> {
    let invalid = || {
        format!(
            "date d'encodage invalide : « {text} » (attendu AAAA-MM-JJ, par exemple 2026-11-16)"
        )
    };

    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(invalid());
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        if bytes[range.clone()].iter().all(u8::is_ascii_digit) {
            text[range].parse().ok()
        } else {
            None
        }
    };
    let (_year, month, day) = match (digits(0..4), digits(5..7), digits(8..10)) {
        (Some(year), Some(month), Some(day)) => (year, month, day),
        _ => return Err(invalid()),
    };

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(invalid());
    }

    Ok(text.to_owned())
}

/// Date du jour en UTC, au format `AAAA-MM-JJ`.
fn today_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convertit un nombre de jours depuis l'époque Unix en date civile UTC.
///
/// Algorithme de Howard Hinnant (`civil_from_days`) : proleptique grégorien,
/// valable pour toutes les dates positives de l'ère commune, sans table ni
/// dépendance.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_affiche_la_licence() {
        use clap::CommandFactory;

        let version = Cli::command().render_version().to_string();
        assert!(version.contains("GPL-3.0-only"), "{version}");
        assert!(version.contains("garantie"), "{version}");
    }

    #[test]
    fn dates_civiles_connues() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_711), (2026, 9, 15));
        assert_eq!(civil_from_days(20_773), (2026, 11, 16));
    }

    #[test]
    fn formats_de_sortie() {
        assert_eq!(
            output_format(Path::new("a.musicxml")),
            Ok(OutputFormat::MusicXml)
        );
        assert_eq!(
            output_format(Path::new("a.XML")),
            Ok(OutputFormat::MusicXml)
        );
        assert_eq!(output_format(Path::new("a.mxl")), Ok(OutputFormat::Mxl));
        assert!(output_format(Path::new("a.pdf")).is_err());
        assert!(output_format(Path::new("a")).is_err());
    }

    #[test]
    fn dates_d_encodage_validees() {
        assert_eq!(
            parse_encoding_date("2026-11-16"),
            Ok("2026-11-16".to_owned())
        );
        for invalid in [
            "2026-13-01",
            "2026-00-10",
            "2026-11-32",
            "26-11-16",
            "2026/11/16",
        ] {
            assert!(parse_encoding_date(invalid).is_err(), "{invalid} accepté");
        }
    }
}
