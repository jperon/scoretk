// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
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

//! CLI `scoretk` : convertit une partition d'un format à un autre.
//!
//! Les formats sont déduits des extensions : entrée `.mscz`/`.mscx`/… et
//! sortie `.musicxml`/`.mxl`/…. `--from` et `--to` forcent le choix, ce qui
//! est nécessaire quand l'entrée ou la sortie est `-` (stdin/stdout).
//!
//! La date d'encodage MusicXML est celle du jour en UTC, comme dans la CLI
//! `mscz2xml` ; `--encoding-date` la fige pour des sorties reproductibles.
//! Seule cette CLI touche au système de fichiers.

use std::io::{Read, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

use scoretk::{Format, MidiOptions, Options};

/// Version affichée par `--version` : en plus du numéro, la licence et
/// l'absence de garantie (GPL §0, « Appropriate Legal Notices »).
const VERSION_LINE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nLicence : GNU GPL version 3 seulement (GPL-3.0-only), sans aucune garantie ;",
    "\nvoir le fichier LICENSE ou <https://www.gnu.org/licenses/gpl-3.0.html>."
);

/// Boîte à outils de conversion de partitions (MuseScore, MusicXML, LilyPond,
/// ABC, MIDI).
#[derive(Debug, Parser)]
#[command(name = "scoretk", version = VERSION_LINE, about)]
struct Cli {
    /// Fichier d'entrée, ou « - » pour l'entrée standard
    #[arg(required_unless_present = "list_formats")]
    input: Option<String>,

    /// Fichier de sortie, ou « - » pour la sortie standard
    #[arg(short, long, value_name = "SORTIE")]
    output: Option<String>,

    /// Force le format d'entrée (mscz, mscx, musicxml, mxl, ly, abc, midi)
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)]
    from: Option<Format>,

    /// Force le format de sortie (mscz, mscx, musicxml, mxl, ly, abc, midi)
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)]
    to: Option<Format>,

    /// Affiche les formats reconnus et les sens pris en charge
    #[arg(long)]
    list_formats: bool,

    /// Date d'encodage MusicXML au format AAAA-MM-JJ (par défaut : aujourd'hui
    /// en UTC)
    #[arg(long, value_name = "AAAA-MM-JJ", value_parser = parse_encoding_date)]
    encoding_date: Option<String>,

    /// Ne pas dérouler les reprises dans la sortie MIDI
    #[arg(long)]
    no_expand_repeats: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.list_formats {
        print_formats();
        return ExitCode::SUCCESS;
    }

    // `clap` garantit la présence de `input` hors `--list-formats`.
    let input = cli.input.expect("entrée exigée par clap");
    let bytes = match read_input(&input) {
        Ok(bytes) => bytes,
        Err(message) => return fail(&message),
    };

    let from = match input_format(cli.from, &input, &bytes) {
        Ok(format) => format,
        Err(message) => return fail(&message),
    };
    let to = match output_format(cli.to, cli.output.as_deref()) {
        Ok(format) => format,
        Err(message) => return fail(&message),
    };

    let score = match scoretk::read(&bytes, from) {
        Ok(score) => score,
        Err(error) => return fail(&format!("{input} : {error}")),
    };

    let options = Options {
        encoding_date: cli.encoding_date.unwrap_or_else(today_utc),
        midi: MidiOptions {
            expand_repeats: !cli.no_expand_repeats,
        },
        ..Options::default()
    };

    let output = match scoretk::write(&score, to, &options) {
        Ok(output) => output,
        Err(error) => return fail(&format!("{input} → {to} : {error}")),
    };

    match cli.output.as_deref() {
        None | Some("-") => write_stdout(&output),
        Some(path) => write_file(path, &output),
    }
}

/// Lit l'entrée : fichier, ou entrée standard pour `-`.
fn read_input(input: &str) -> Result<Vec<u8>, String> {
    if input == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .lock()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("lecture de l'entrée standard impossible : {error}"))?;
        return Ok(bytes);
    }
    std::fs::read(input).map_err(|error| format!("lecture de {input} impossible : {error}"))
}

/// Détermine le format d'entrée : `--from`, sinon l'extension, sinon le
/// contenu.
fn input_format(forced: Option<Format>, input: &str, bytes: &[u8]) -> Result<Format, String> {
    if let Some(format) = forced {
        return Ok(format);
    }
    if input != "-" {
        if let Some(format) = Format::from_path(input) {
            return Ok(format);
        }
    }
    Format::detect(bytes).ok_or_else(|| {
        if input == "-" {
            "format d'entrée indéterminé : précisez --from".to_owned()
        } else {
            format!("format d'entrée indéterminé pour {input} : précisez --from")
        }
    })
}

/// Détermine le format de sortie : `--to`, sinon l'extension du fichier de
/// sortie.
fn output_format(forced: Option<Format>, output: Option<&str>) -> Result<Format, String> {
    if let Some(format) = forced {
        return Ok(format);
    }
    match output {
        None | Some("-") => {
            Err("format de sortie indéterminé : précisez --to (sortie standard)".to_owned())
        }
        Some(path) => Format::from_path(path)
            .ok_or_else(|| format!("extension de sortie non reconnue pour {path} : précisez --to")),
    }
}

fn write_stdout(output: &[u8]) -> ExitCode {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if let Err(error) = lock.write_all(output).and_then(|()| lock.flush()) {
        return fail(&format!(
            "écriture sur la sortie standard impossible : {error}"
        ));
    }
    ExitCode::SUCCESS
}

fn write_file(path: &str, output: &[u8]) -> ExitCode {
    if let Err(error) = std::fs::write(path, output) {
        return fail(&format!("écriture de {path} impossible : {error}"));
    }
    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode {
    eprintln!("scoretk : {message}");
    ExitCode::FAILURE
}

/// Tableau de `--list-formats`.
fn print_formats() {
    println!(
        "{:<9} {:<20} {:<8} ÉCRITURE",
        "FORMAT", "EXTENSIONS", "LECTURE"
    );
    for format in Format::ALL {
        let extensions = format
            .extensions()
            .iter()
            .map(|extension| format!(".{extension}"))
            .collect::<Vec<_>>()
            .join(", ");
        let yes_no = |supported: bool| if supported { "oui" } else { "non" };
        println!(
            "{:<9} {:<20} {:<8} {}",
            format.as_str(),
            extensions,
            yes_no(format.supports_read()),
            yes_no(format.supports_write()),
        );
    }
}

/// Analyse un nom de format pour `clap`.
fn parse_format(text: &str) -> Result<Format, String> {
    text.parse::<Format>().map_err(|error| error.to_string())
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
