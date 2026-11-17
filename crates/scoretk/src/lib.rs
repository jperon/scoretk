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
//! contenu ([`Format::detect`]).
//!
//! | Format | Lecture | Écriture |
//! | --- | --- | --- |
//! | MuseScore `.mscz` / `.mscx` | oui | feature `mscx` |
//! | MusicXML `.musicxml` / `.xml` | feature `musicxml-import` | oui |
//! | MusicXML `.mxl` | feature `musicxml-import` | oui |
//! | LilyPond `.ly` | feature `lilypond-import` | feature `lilypond` |
//! | ABC `.abc` | feature `abc` | feature `abc` |
//! | MIDI | feature `midi` | feature `midi` |
//!
//! Un format sans convertisseur dans le sens demandé renvoie
//! [`Error::FormatNonPrisEnCharge`] : la façade ne produit jamais de sortie
//! vide ni de conversion silencieusement partielle.
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

mod error;
mod format;
mod hooks;
mod options;

pub use error::Error;
pub use format::{Format, Sens};
pub use options::{MidiOptions, Options};
pub use scoretk_core::model::Score;
/// Types de la carte des temps, réexportés pour les consommateurs de
/// [`write_playback`] (wasm, CLI) sans dépendre directement de `scoretk-midi`.
#[cfg(feature = "midi")]
pub use scoretk_midi::{MeasureSync, NoteSync, PauseSync, TempoChange, Timemap};

/// Résultat de la façade.
pub type Result<T> = std::result::Result<T, Error>;

/// Résultat de [`write_playback`] : MusicXML identifié, MIDI et carte des temps.
///
/// Le MusicXML est exporté avec `ExportOptions::stable_ids` forcé : ses
/// attributs `id`/`xml:id` de mesure et de note sont exactement les
/// identifiants [`scoretk_midi::NoteSync::xml_id`] et
/// [`scoretk_midi::MeasureSync::xml_id`] de la carte des temps.
#[cfg(feature = "midi")]
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackOutput {
    /// MusicXML partwise, avec identifiants stables (`id` et `xml:id`).
    pub musicxml: Vec<u8>,
    /// Standard MIDI File format 1.
    pub midi: Vec<u8>,
    /// Carte des temps du même rendu MIDI.
    pub timemap: scoretk_midi::Timemap,
}

/// Lit un fichier en mémoire et renvoie le modèle pivot.
///
/// `mscz` et `mscx` passent par le lecteur de `scoretk_core`. MusicXML (`.musicxml`
/// nu comme archive `.mxl`) est lu si la feature `musicxml-import` est active,
/// LilyPond avec `lilypond-import`, ABC avec `abc` ; les autres formats
/// renvoient [`Error::FormatNonPrisEnCharge`].
pub fn read(bytes: &[u8], format: Format) -> Result<Score> {
    match format {
        Format::Mscz | Format::Mscx => {
            let container = scoretk_core::MsczContainer::from_bytes(bytes)?;
            Ok(scoretk_core::read::read_score(&container)?)
        }
        Format::MusicXml => hooks::read_musicxml(bytes),
        Format::Mxl => hooks::read_mxl(bytes),
        Format::Ly => hooks::read_lilypond(bytes),
        Format::Abc => hooks::read_abc(bytes),
        Format::Midi => hooks::read_midi(bytes),
    }
}

/// Écrit le modèle pivot dans le format demandé.
///
/// MusicXML passe par l'export de `scoretk_core` ; `.mxl` réutilise son archive
/// ZIP (`META-INF/container.xml` + `score.xml`), identique à `saveMxl` de
/// MuseScore. MuseScore (`.mscx`/`.mscz`) passe par `scoretk-mscx` si la
/// feature `mscx` est active, MIDI par `scoretk-midi` si la feature `midi`
/// l'est, LilyPond par `scoretk-lilypond` si la feature `lilypond` l'est, ABC
/// par `scoretk-abc` si la feature `abc` l'est ; les autres formats renvoient
/// [`Error::FormatNonPrisEnCharge`].
pub fn write(score: &Score, format: Format, options: &Options) -> Result<Vec<u8>> {
    match format {
        Format::MusicXml => Ok(scoretk_core::export::export_musicxml_with(
            score,
            &options.export_options(),
        )?
        .into_bytes()),
        Format::Mxl => Ok(scoretk_core::export::export_mxl_with(
            score,
            &options.export_options(),
        )?),
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
/// [`scoretk_midi::MeasureSync::xml_id`] et
/// [`scoretk_midi::NoteSync::xml_id`], ce qui permet d'associer un évènement
/// de la carte des temps à un élément du MusicXML.
///
/// Les reprises suivent `options.midi.expand_repeats`. La carte et le MIDI
/// sont produits par le même rendu : les ticks de la carte sont ceux du
/// fichier, pauses de section comprises. Voir `docs/agents/timemap.md`.
#[cfg(feature = "midi")]
pub fn write_playback(score: &Score, options: &Options) -> Result<PlaybackOutput> {
    let export = scoretk_core::ExportOptions {
        stable_ids: true,
        ..options.export_options()
    };
    let musicxml = scoretk_core::export::export_musicxml_with(score, &export)?.into_bytes();
    let (midi, timemap) = hooks::write_playback(score, options)?;
    Ok(PlaybackOutput {
        musicxml,
        midi,
        timemap,
    })
}
