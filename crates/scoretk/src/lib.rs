// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage partiel de `importexport/musicxml/internal/musicxmlwriter.cpp` et
// `importexport/musicxml/internal/mxlwriter.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026 : aiguillage de l'écriture MusicXML et `.mxl`, et, pour la lecture,
// `importexport/musicxml/internal/import/importmusicxml.cpp`
// (`importCompressedMusicXml`).
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

//! **scoretk** : façade de conversion de partitions.
//!
//! Le modèle de partition de `scoretk_core` est le pivot commun : chaque format
//! fournit un lecteur vers ce modèle ([`read`]) et un écrivain depuis ce
//! modèle ([`write`]), ce qui évite des convertisseurs N×N. La bibliothèque
//! reste portable : aucun accès au disque, aucun thread, aucune horloge ; tout
//! passe par des octets en mémoire et elle compile pour
//! `wasm32-unknown-unknown`.
//!
//! # Formats
//!
//! [`Format`] énumère les sept formats visés. La reconnaissance se fait par
//! extension ([`Format::from_extension`], [`Format::from_path`]) ou par le
//! contenu ([`Format::detect`]). Chaque sens a sa feature, qui branche
//! exactement un crate (`docs/agents/scoretk.md` § 3) :
//!
//! | Format | Lecture | Écriture |
//! | --- | --- | --- |
//! | MuseScore `.mscz` / `.mscx` | `musescore-import` | `musescore-export` |
//! | MusicXML `.musicxml` / `.xml` | `musicxml-import` | `musicxml-export` |
//! | MusicXML `.mxl` | `musicxml-import` | `musicxml-export` |
//! | LilyPond `.ly` | `lilypond-import` | `lilypond-export` |
//! | ABC `.abc` | `abc-import` | `abc-export` |
//! | MIDI | `midi-import` | `midi-export` |
//!
//! Les dix features sont actives par défaut. Un format dont la feature du sens
//! demandé est désactivée renvoie [`Error::FormatNonPrisEnCharge`] : la façade
//! ne produit jamais de sortie vide ni de conversion silencieusement partielle.
//!
//! # Exemple
//!
//! ```
//! use scoretk::{read, write, Format, Options};
//!
//! let mscx = br#"<museScore version="4.60">
//!   <Score>
//!     <Division>480</Division>
//!     <Part id="1">
//!       <Staff><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
//!       <Instrument id="piano">
//!         <InstrumentLabel><longName>Piano</longName></InstrumentLabel>
//!         <instrumentId>keyboard.piano</instrumentId>
//!       </Instrument>
//!     </Part>
//!     <Staff>
//!       <Measure>
//!         <voice>
//!           <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
//!           <Chord>
//!             <durationType>whole</durationType>
//!             <Note><pitch>60</pitch><tpc>14</tpc></Note>
//!           </Chord>
//!         </voice>
//!       </Measure>
//!     </Staff>
//!   </Score>
//! </museScore>"#;
//!
//! let score = read(mscx, Format::Mscx)?;
//! let musicxml = write(&score, Format::MusicXml, &Options::default())?;
//! assert!(String::from_utf8(musicxml)?.contains("<score-partwise"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[cfg(feature = "musicxml-export")]
mod convert;
mod error;
mod format;
mod hooks;
mod options;

#[cfg(feature = "musicxml-export")]
pub use convert::{convert, ConvertOptions, OutputFormat};
pub use error::Error;
pub use format::{Format, Sens};
pub use options::{MidiOptions, Options};
pub use scoretk_core::model;
pub use scoretk_core::model::Score;

/// Types de la carte des temps, réexportés pour les consommateurs de
/// [`write_playback`] (wasm, CLI) sans dépendre directement de
/// `scoretk-midi-export`.
#[cfg(feature = "midi-export")]
pub use scoretk_midi_export::{MeasureSync, NoteSync, PauseSync, TempoChange, Timemap};
/// Options d'import MIDI (`MidiOperations`), réexportées pour la CLI et les
/// oracles qui reproduisent une configuration de MuseScore
/// (`simplifyDurations`, `maxVoiceCount`).
#[cfg(feature = "midi-import")]
pub use scoretk_midi_import::{ImportOptions as MidiImportOptions, VoiceCount};
#[cfg(feature = "musescore-import")]
pub use scoretk_musescore_import::{detect_version, MscVersion, MsczContainer};
#[cfg(feature = "musicxml-export")]
pub use scoretk_musicxml_export::{
    export_musicxml, export_musicxml_with, ExportOptions, DEFAULT_ENCODING_DATE, DEFAULT_SOFTWARE,
};

/// Résultat de la façade.
pub type Result<T> = std::result::Result<T, Error>;

/// Résultat de [`write_playback`] : MusicXML identifié, MIDI et carte des temps.
///
/// Le MusicXML est exporté avec `ExportOptions::stable_ids` forcé : ses
/// attributs `id`/`xml:id` de mesure et de note sont exactement les
/// identifiants `NoteSync::xml_id` et `MeasureSync::xml_id` de la carte des
/// temps.
#[cfg(all(feature = "midi-export", feature = "musicxml-export"))]
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackOutput {
    /// MusicXML partwise, avec identifiants stables (`id` et `xml:id`).
    pub musicxml: Vec<u8>,
    /// Standard MIDI File format 1.
    pub midi: Vec<u8>,
    /// Carte des temps du même rendu MIDI.
    pub timemap: scoretk_midi_export::Timemap,
}

/// Lit un fichier en mémoire et renvoie le modèle pivot.
///
/// Chaque format passe par le crate de son sens, branché par la feature
/// `<format>-import` (`docs/agents/scoretk.md` § 3) ; sans elle, [`read`]
/// renvoie [`Error::FormatNonPrisEnCharge`].
pub fn read(bytes: &[u8], format: Format) -> Result<Score> {
    match format {
        Format::Mscz | Format::Mscx => hooks::read_musescore(bytes, format),
        Format::MusicXml => hooks::read_musicxml(bytes),
        Format::Mxl => hooks::read_mxl(bytes),
        Format::Ly => hooks::read_lilypond(bytes),
        Format::Abc => hooks::read_abc(bytes),
        Format::Midi => hooks::read_midi(bytes),
    }
}

/// Lit un MIDI avec des options d'import explicites.
///
/// Les options par défaut sont celles de `MidiOperations::Opers` ; cette
/// variante sert à reproduire une configuration précise de MuseScore (tests
/// amont `dontSimplify`, `maxVoiceCount`). Sans la feature `midi-import`,
/// renvoie [`Error::FormatNonPrisEnCharge`].
#[cfg(feature = "midi-import")]
pub fn read_midi_with(bytes: &[u8], options: &MidiImportOptions) -> Result<Score> {
    scoretk_midi_import::read_midi_with(bytes, options)
        .map_err(|error| Error::MidiImport(error.to_string()))
}

/// Lit un LilyPond `.ly` en résolvant ses `\include` par un rappel.
///
/// `resolve(chemin_demandé, chemin_du_fichier_qui_inclut)` renvoie le texte du
/// fichier inclus ou `None` s'il est introuvable. Le rappel évite toute
/// entrée/sortie dans la façade : c'est l'appelant qui lit les fichiers, comme
/// la CLI (`docs/agents/lilypond.md` § 1). Les fichiers de langue de LilyPond
/// et les fichiers livrés sans musique ne passent pas par le rappel. Sans la
/// feature `lilypond-import`, la fonction renvoie
/// [`Error::FormatNonPrisEnCharge`].
pub fn read_lilypond_with_includes(
    bytes: &[u8],
    resolve: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
) -> Result<Score> {
    hooks::read_lilypond_with_includes(bytes, resolve)
}

/// Écrit le modèle pivot dans le format demandé.
///
/// MusicXML et `.mxl` passent par `scoretk-musicxml-export` ; `.mxl` réutilise
/// son archive ZIP (`META-INF/container.xml` + `score.xml`), identique à
/// `saveMxl` de MuseScore. Les autres formats passent par le crate de leur
/// sens, branché par la feature `<format>-export` ; sans elle, [`write`]
/// renvoie [`Error::FormatNonPrisEnCharge`].
pub fn write(score: &Score, format: Format, options: &Options) -> Result<Vec<u8>> {
    match format {
        Format::MusicXml => hooks::write_musicxml(score, options),
        Format::Mxl => hooks::write_mxl(score, options),
        Format::Midi => hooks::write_midi(score, options),
        Format::Mscx => hooks::write_mscx(score),
        Format::Mscz => hooks::write_mscz(score),
        Format::Ly => hooks::write_lilypond(score),
        Format::Abc => hooks::write_abc(score),
    }
}

/// Écrit une partition en MusicXML et en MIDI, avec la carte des temps du
/// rendu MIDI.
///
/// Le MusicXML est le même que [`write`] avec [`Options::default`], à une
/// exception près : `ExportOptions::stable_ids` est forcé, donc chaque
/// `<measure>` et chaque `<note>` porte un attribut `id` **et** `xml:id`
/// (même valeur). Ces identifiants correspondent exactement à
/// `MeasureSync::xml_id` et `NoteSync::xml_id` de la carte des temps, ce qui
/// permet d'associer un évènement de la carte à un élément du MusicXML.
///
/// Les reprises suivent `options.midi.expand_repeats`. La carte et le MIDI
/// sont produits par le même rendu : les ticks de la carte sont ceux du
/// fichier, pauses de section comprises. Voir `docs/agents/timemap.md`.
///
/// La fonction exige les features `midi-export` (carte des temps et MIDI) et
/// `musicxml-export` (MusicXML identifié) : elle est absente si l'une des deux
/// manque.
#[cfg(all(feature = "midi-export", feature = "musicxml-export"))]
pub fn write_playback(score: &Score, options: &Options) -> Result<PlaybackOutput> {
    let export = scoretk_musicxml_export::ExportOptions {
        stable_ids: true,
        ..options.export_options()
    };
    let musicxml = scoretk_musicxml_export::export_musicxml_with(score, &export)?.into_bytes();
    let (midi, timemap) = hooks::write_playback(score, options)?;
    Ok(PlaybackOutput {
        musicxml,
        midi,
        timemap,
    })
}
