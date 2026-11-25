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
//! La date d'encodage MusicXML est celle du jour en UTC, comme dans l'export
//! de `scoretk-core` ; `--encoding-date` la fige pour des sorties reproductibles.
//! Seule cette CLI touche au système de fichiers.
//!
//! Codes de sortie : 0 succès, 1 échec de conversion, 2 usage invalide,
//! 3 entrée délibérément non prise en charge (version ou format), voir
//! [`EXIT_UNSUPPORTED`].

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

use scoretk::{Format, MidiOptions, Options, Score};

/// Version affichée par `--version` : en plus du numéro, la licence et
/// l'absence de garantie (GPL §0, « Appropriate Legal Notices »).
const VERSION_LINE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nLicence : GNU GPL version 3 seulement (GPL-3.0-only), sans aucune garantie ;",
    "\nvoir le fichier LICENSE ou <https://www.gnu.org/licenses/gpl-3.0.html>."
);

/// Code de sortie d'une entrée délibérément non prise en charge.
///
/// Distinct d'un échec de conversion (1) et d'un usage invalide (2) : les
/// scripts de mesure excluent ces fichiers de leur numérateur et de leur
/// dénominateur au lieu de les compter comme des régressions. Voir
/// `scoretk::Error::is_unsupported`.
const EXIT_UNSUPPORTED: u8 = 3;

/// Boîte à outils de conversion de partitions (MuseScore, MusicXML, LilyPond,
/// ABC, MIDI).
#[derive(Debug, Parser)]
#[command(
    name = "scoretk",
    version = VERSION_LINE,
    about,
    after_help = "Codes de sortie : 0 succès ; 1 échec de conversion ; \
                  2 usage invalide ; 3 entrée non prise en charge \
                  (version ou format délibérément non supporté, par exemple \
                  MuseScore < 1.15)."
)]
struct Cli {
    /// Fichier d'entrée, ou « - » pour l'entrée standard
    #[arg(required_unless_present = "list_formats")]
    input: Option<String>,

    /// Fichier de sortie, ou « - » pour la sortie standard
    #[arg(short, long, value_name = "SORTIE")]
    output: Option<String>,

    /// Force le format d'entrée (mscz, mscx, musicxml, mxl, mei, ly, abc, midi, gabc)
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)]
    from: Option<Format>,

    /// Force le format de sortie (mscz, mscx, musicxml, mxl, mei, ly, abc, midi, gabc)
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

    /// Import MIDI : ne pas simplifier les durées (`simplifyDurations`)
    #[arg(long)]
    midi_no_simplify: bool,

    /// Import MIDI : plus petite valeur de quantification, en fraction de
    /// ronde (`quantValue` : 4, 8, 16 par défaut, 32…)
    #[arg(long, value_name = "N", value_parser = parse_quant_value)]
    midi_quant: Option<u16>,

    /// Import MIDI : ne pas changer de clef (`changeClef`)
    #[arg(long)]
    midi_no_change_clef: bool,

    /// Import MIDI : nombre maximal de voix, 1 à 4 (`maxVoiceCount`)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=4))]
    midi_voices: Option<u8>,

    /// Import MIDI : détecter et noter les n-olets (`<time-modification>`)
    #[arg(long)]
    midi_tuplet_notation: bool,

    /// Import MIDI : verrouiller `doStaffSplit` sur false (tests amont)
    #[arg(long, conflicts_with = "midi_staff_split")]
    midi_no_staff_split: bool,

    /// Import MIDI : verrouiller `doStaffSplit` sur true (tests amont)
    #[arg(long)]
    midi_staff_split: bool,
}

/// Valeur de quantification MIDI acceptée : une puissance de deux ≥ 4.
fn parse_quant_value(valeur: &str) -> Result<u16, String> {
    let quant: u16 = valeur
        .parse()
        .map_err(|_| format!("valeur de quantification invalide : {valeur}"))?;
    if quant >= 4 && quant.is_power_of_two() {
        Ok(quant)
    } else {
        Err(format!(
            "valeur de quantification invalide : {valeur} (4, 8, 16, 32, 64, 128, 256, 512 ou 1024)"
        ))
    }
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

    // Options d'import MIDI explicites (`MidiOperations`) : la CLI les
    // transmet telles quelles, ce qui permet de reproduire une configuration
    // de référence MuseScore.
    let mut midi_options = scoretk::MidiImportOptions::default();
    let mut custom_midi = false;
    if cli.midi_no_simplify {
        midi_options.simplify_durations = false;
        custom_midi = true;
    }
    if let Some(quant) = cli.midi_quant {
        midi_options.quant_value = match quant {
            4 => scoretk::QuantValue::Q4,
            8 => scoretk::QuantValue::Q8,
            16 => scoretk::QuantValue::Q16,
            32 => scoretk::QuantValue::Q32,
            64 => scoretk::QuantValue::Q64,
            128 => scoretk::QuantValue::Q128,
            256 => scoretk::QuantValue::Q256,
            512 => scoretk::QuantValue::Q512,
            _ => scoretk::QuantValue::Q1024,
        };
        custom_midi = true;
    }
    if cli.midi_no_change_clef {
        midi_options.change_clef = false;
        custom_midi = true;
    }
    if cli.midi_tuplet_notation {
        midi_options.tuplet_notation = true;
        custom_midi = true;
    }
    if cli.midi_no_staff_split {
        midi_options.do_staff_split = false;
        midi_options.allow_staff_split_redefinition = false;
        custom_midi = true;
    }
    if cli.midi_staff_split {
        midi_options.do_staff_split = true;
        midi_options.allow_staff_split_redefinition = false;
        custom_midi = true;
    }
    if let Some(voices) = cli.midi_voices {
        midi_options.max_voice_count = match voices {
            1 => scoretk::VoiceCount::V1,
            2 => scoretk::VoiceCount::V2,
            3 => scoretk::VoiceCount::V3,
            _ => scoretk::VoiceCount::V4,
        };
        custom_midi = true;
    }

    let score = if from == scoretk::Format::Midi && custom_midi {
        match scoretk::read_midi_with(&bytes, &midi_options) {
            Ok(score) => score,
            Err(error) if error.is_unsupported() => {
                return fail_unsupported(&format!("{input} : {error}"));
            }
            Err(error) => return fail(&format!("{input} : {error}")),
        }
    } else {
        match read_score(&bytes, from, &input) {
            Ok(score) => score,
            Err(error) if error.is_unsupported() => {
                return fail_unsupported(&format!("{input} : {error}"));
            }
            Err(error) => return fail(&format!("{input} : {error}")),
        }
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

/// Lit la partition : un fichier `.ly` passe par le résolveur d'inclusions du
/// disque, les autres formats par la façade.
fn read_score(bytes: &[u8], from: Format, input: &str) -> scoretk::Result<Score> {
    if from == Format::Ly && input != "-" {
        let mut resolve = lilypond_resolver(Path::new(input));
        return scoretk::read_lilypond_with_includes(bytes, &mut resolve);
    }
    scoretk::read(bytes, from)
}

/// Résolveur de `\include` pour un fichier `.ly` du disque.
///
/// Le chemin demandé est cherché d'abord dans le dossier du fichier qui
/// inclut, puis dans celui du fichier principal ; les `.` et `..` sont
/// normalisés. Aucune autre restriction n'est imposée.
fn lilypond_resolver(main: &Path) -> impl FnMut(&str, Option<&str>) -> Option<String> {
    let main_dir = main.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    move |requested, includer| {
        let includer_dir = includer
            .and_then(|path| Path::new(path).parent())
            .unwrap_or_else(|| Path::new(""));
        let candidates = [
            main_dir.join(includer_dir).join(requested),
            main_dir.join(requested),
        ];
        candidates
            .iter()
            .find_map(|path| std::fs::read_to_string(normalize_path(path)).ok())
    }
}

/// Normalise un chemin en résolvant `.` et `..` lexicalement.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
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

/// Échec dû à une entrée délibérément non prise en charge (code 3).
fn fail_unsupported(message: &str) -> ExitCode {
    eprintln!("scoretk : entrée non prise en charge : {message}");
    ExitCode::from(EXIT_UNSUPPORTED)
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
