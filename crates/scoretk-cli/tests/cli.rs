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

//! Tests de la CLI `scoretk` lancée comme un vrai processus.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scoretk"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/v4")
        .join(name)
}

fn temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("scoretk-cli-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("répertoire temporaire");
    dir
}

/// Ligne de `--list-formats` d'un format donné.
fn ligne_format(tableau: &str, format: &str) -> String {
    tableau
        .lines()
        .find(|ligne| {
            ligne
                .split_whitespace()
                .next()
                .is_some_and(|nom| nom == format)
        })
        .unwrap_or_else(|| panic!("tableau sans {format} : {tableau}"))
        .to_owned()
}

/// Sens (lecture, écriture) annoncés par la ligne d'un format.
fn sens_format(tableau: &str, format: &str) -> (String, String) {
    let ligne = ligne_format(tableau, format);
    let mut jetons = ligne.split_whitespace().rev();
    let ecriture = jetons.next().unwrap_or_default().to_owned();
    let lecture = jetons.next().unwrap_or_default().to_owned();
    (lecture, ecriture)
}

#[test]
fn list_formats() {
    let output = binary().arg("--list-formats").output().expect("exécution");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    for format in ["mscz", "mscx", "musicxml", "mxl", "ly", "abc", "midi"] {
        assert!(text.contains(format), "tableau sans {format} : {text}");
    }
    assert!(text.contains("LECTURE"));
    assert!(text.contains("ÉCRITURE"));

    // État réel des features activées par la CLI : les sept formats se lisent
    // et s'écrivent (`.mxl` en lecture depuis t43).
    let oui = ("oui".to_owned(), "oui".to_owned());
    assert_eq!(sens_format(&text, "mscz"), oui, "{text}");
    assert_eq!(sens_format(&text, "musicxml"), oui, "{text}");
    assert_eq!(sens_format(&text, "midi"), oui, "{text}");
    assert_eq!(sens_format(&text, "mxl"), oui, "{text}");
    assert_eq!(sens_format(&text, "ly"), oui, "{text}");
    assert_eq!(sens_format(&text, "abc"), oui, "{text}");
}

#[test]
fn version_porte_la_licence() {
    let output = binary().arg("--version").output().expect("exécution");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("GPL-3.0-only"), "{text}");
    assert!(text.contains("sans aucune garantie"), "{text}");
}

#[test]
fn mscx_vers_musicxml_sur_la_sortie_standard() {
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .args(["--to", "musicxml", "--encoding-date", "2026-11-17"])
        .output()
        .expect("exécution");
    assert!(output.status.success(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("<score-partwise"));
    assert!(text.contains("<encoding-date>2026-11-17</encoding-date>"));
}

#[test]
fn mscz_vers_musicxml_identique_a_scoretk_core() {
    let mscx = std::fs::read(corpus("dynamics.mscx")).expect("corpus");
    let mscz = mscz_from_mscx(&mscx);

    let dir = temp_dir();
    let input = dir.join("partition.mscz");
    std::fs::write(&input, &mscz).expect("écriture du .mscz");

    let output = binary()
        .arg(&input)
        .args(["--to", "musicxml", "--encoding-date", "2026-11-17"])
        .output()
        .expect("exécution");
    assert!(output.status.success(), "{:?}", output.stderr);

    let options = scoretk::ConvertOptions {
        format: scoretk::OutputFormat::MusicXml,
        export: scoretk_musicxml_export::ExportOptions {
            encoding_date: "2026-11-17".to_owned(),
            ..scoretk_musicxml_export::ExportOptions::default()
        },
    };
    let reference = scoretk::convert(&mscx, &options).expect("scoretk_core");

    assert_eq!(output.stdout, reference);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn entree_et_sortie_standard_avec_from_et_to() {
    let mscx = std::fs::read(corpus("dynamics.mscx")).expect("corpus");

    let mut child = binary()
        .args(["-", "--from", "mscx", "--to", "musicxml"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lancement");
    std::io::Write::write_all(child.stdin.as_mut().expect("stdin"), &mscx).expect("écriture stdin");
    let output = child.wait_with_output().expect("attente");

    assert!(output.status.success(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("<score-partwise"));
}

#[test]
fn sortie_mxl_par_extension() {
    let dir = temp_dir();
    let output_path = dir.join("partition.mxl");
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(&output_path)
        .args(["--encoding-date", "2026-11-17"])
        .output()
        .expect("exécution");
    assert!(output.status.success(), "{:?}", output.stderr);

    let bytes = std::fs::read(&output_path).expect(".mxl");
    assert_eq!(bytes.get(..4), Some(b"PK\x03\x04".as_slice()));

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("archive");
    assert!(archive.by_name("META-INF/container.xml").is_ok());
    assert!(archive.by_name("score.xml").is_ok());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn entree_standard_sans_from_echoue() {
    let output = binary()
        .args(["-", "--to", "musicxml"])
        .stdin(Stdio::null())
        .output()
        .expect("exécution");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(stderr.contains("--from"), "{stderr}");
}

#[test]
fn sortie_standard_sans_to_echoue() {
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .output()
        .expect("exécution");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(stderr.contains("--to"), "{stderr}");
}

#[test]
fn extension_de_sortie_inconnue_echoue() {
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(Path::new("/tmp/scoretk-sortie.pdf"))
        .output()
        .expect("exécution");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(stderr.contains("extension de sortie"), "{stderr}");
}

#[test]
fn entree_mxl_invalide_echoue_explicitement() {
    // Contenu `.mscx` déclaré `.mxl` : l'archive est invalide et la conversion
    // doit échouer explicitement au lieu de produire une sortie vide.
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .args(["--from", "mxl"])
        .arg("-o")
        .arg(Path::new("/tmp/scoretk-sortie.abc"))
        .output()
        .expect("exécution");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(stderr.contains("conteneur .mxl"), "{stderr}");
}

fn mscz_from_mscx(mscx: &[u8]) -> Vec<u8> {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let options = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("META-INF/container.xml", options)
        .expect("container.xml");
    writer
        .write_all(
            br#"<container><rootfiles><rootfile full-path="score.mscx"/></rootfiles></container>"#,
        )
        .expect("contenu container.xml");
    writer
        .start_file("score.mscx", options)
        .expect("score.mscx");
    writer.write_all(mscx).expect("contenu .mscx");
    writer.finish().expect("archive").into_inner()
}

#[test]
fn mscx_vers_lilypond() {
    let sortie = Path::new("/tmp/scoretk-cli-sortie.ly");
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(sortie)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let texte = std::fs::read_to_string(sortie).expect("sortie LilyPond");
    assert!(texte.starts_with("\\version \"2.24.0\"\n"), "{texte}");
    assert!(texte.contains("= \\fixed c' {"), "{texte}");
    assert!(!texte.contains("\\relative"), "{texte}");
    let _ = std::fs::remove_file(sortie);
}

#[test]
fn mscx_ecrit_et_relu_donne_le_meme_musicxml() {
    let dir = temp_dir();
    let written = dir.join("copie.mscx");
    let direct = dir.join("direct.musicxml");
    let relu = dir.join("relu.musicxml");

    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(output.status.success(), "{:?}", output.stderr);
    let text = std::fs::read_to_string(&written).expect("sortie .mscx");
    assert!(text.contains("<museScore version=\"4.70\">"), "{text}");

    let export = |input: &Path, output: &Path| {
        let status = binary()
            .arg(input)
            .args(["--to", "musicxml", "--encoding-date", "2026-11-17"])
            .arg("-o")
            .arg(output)
            .status()
            .expect("exécution");
        assert!(status.success());
        std::fs::read(output).expect("sortie MusicXML")
    };
    let expected = export(&corpus("dynamics.mscx"), &direct);
    let actual = export(&written, &relu);
    assert_eq!(
        expected, actual,
        "l'aller-retour .mscx doit donner le même MusicXML"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mscz_ecrit_et_relu_par_scoretk_core() {
    let dir = temp_dir();
    let written = dir.join("copie.mscz");
    let output = binary()
        .arg(corpus("hairpin_simple.mscx"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(output.status.success(), "{:?}", output.stderr);

    let archive = std::fs::read(&written).expect("sortie .mscz");
    let container =
        scoretk_musescore_import::MsczContainer::from_bytes(&archive).expect("conteneur .mscz");
    assert_eq!(container.version().expect("version").to_string(), "4.70");
    assert_eq!(container.main_mscx_name(), "score.mscx");
    let score = scoretk_musescore_import::read::read_score(&container).expect("relecture");
    let options = scoretk_musicxml_export::ExportOptions {
        encoding_date: "2026-11-17".to_owned(),
        ..scoretk_musicxml_export::ExportOptions::default()
    };
    let xml = scoretk_musicxml_export::export_musicxml_with(&score, &options).expect("export");
    assert!(xml.contains("<score-partwise"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn musicxml_vers_mscx() {
    let dir = temp_dir();
    let written = dir.join("depuis-musicxml.mscx");
    let output = binary()
        .arg(corpus("dynamics.ref.musicxml"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(&written).expect("sortie .mscx");
    assert!(text.contains("<museScore version=\"4.70\">"), "{text}");
    assert!(text.contains("<Note>"), "{text}");

    // Le `.mscx` écrit se relit et s'exporte en MusicXML.
    let exporte = dir.join("relu.musicxml");
    let status = binary()
        .arg(&written)
        .args(["--to", "musicxml", "--encoding-date", "2026-11-17"])
        .arg("-o")
        .arg(&exporte)
        .status()
        .expect("exécution");
    assert!(status.success());
    let xml = std::fs::read_to_string(&exporte).expect("sortie MusicXML");
    assert!(xml.contains("<score-partwise"), "{xml}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn musicxml_vers_lilypond() {
    let dir = temp_dir();
    let written = dir.join("depuis-musicxml.ly");
    let output = binary()
        .arg(corpus("dynamics.ref.musicxml"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let texte = std::fs::read_to_string(&written).expect("sortie LilyPond");
    assert!(texte.starts_with("\\version \"2.24.0\"\n"), "{texte}");
    assert!(texte.contains("= \\fixed c' {"), "{texte}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mscx_vers_midi() {
    let dir = temp_dir();
    let written = dir.join("depuis-mscx.mid");
    let output = binary()
        .arg(corpus("dynamics.mscx"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = std::fs::read(&written).expect("sortie MIDI");
    assert_eq!(bytes.get(..4), Some(b"MThd".as_slice()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn midi_vers_musicxml() {
    let dir = temp_dir();
    let written = dir.join("depuis-midi.musicxml");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/midi/v4/dynamics.ref.mid");
    let output = binary()
        .arg(&source)
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&written).expect("sortie MusicXML");
    assert!(text.contains("<score-partwise"), "{text}");
    assert!(text.contains("<step>D</step>"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn musicxml_vers_midi() {
    let dir = temp_dir();
    let written = dir.join("depuis-musicxml.mid");
    let output = binary()
        .arg(corpus("dynamics.ref.musicxml"))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("exécution");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = std::fs::read(&written).expect("sortie MIDI");
    assert_eq!(bytes.get(..4), Some(b"MThd".as_slice()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_expand_repeats_raccourcit_le_midi() {
    // `repeat65` porte de vraies reprises (deux voltas et un saut) : le MIDI
    // déroulé est nécessairement plus long que le MIDI « à plat ».
    let with_repeats = binary()
        .arg(corpus("repeat65.mscx"))
        .args(["--to", "midi"])
        .output()
        .expect("exécution");
    assert!(
        with_repeats.status.success(),
        "{}",
        String::from_utf8_lossy(&with_repeats.stderr)
    );

    let without_repeats = binary()
        .arg(corpus("repeat65.mscx"))
        .args(["--to", "midi", "--no-expand-repeats"])
        .output()
        .expect("exécution");
    assert!(
        without_repeats.status.success(),
        "{}",
        String::from_utf8_lossy(&without_repeats.stderr)
    );

    assert_eq!(
        with_repeats.stdout.get(..4),
        Some(b"MThd".as_slice()),
        "sortie MIDI attendue"
    );
    assert!(
        with_repeats.stdout.len() > without_repeats.stdout.len(),
        "--no-expand-repeats devrait raccourcir le MIDI ({} vs {} octets)",
        without_repeats.stdout.len(),
        with_repeats.stdout.len()
    );
}
