// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage partiel de `importexport/musicxml/internal/musicxmlwriter.cpp` et
// `importexport/musicxml/internal/mxlwriter.cpp` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026 : aiguillage de l'écriture MusicXML et `.mxl`.
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
//! Le modèle de partition de `mscz2xml` est le pivot commun : chaque format
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
//! | MusicXML `.mxl` | non | oui |
//! | LilyPond `.ly` | non | feature `lilypond` |
//! | ABC | non | non |
//! | MIDI | non | feature `midi` |
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
pub use mscz2xml::model::Score;
pub use options::Options;

/// Résultat de la façade.
pub type Result<T> = std::result::Result<T, Error>;

/// Lit un fichier en mémoire et renvoie le modèle pivot.
///
/// `mscz` et `mscx` passent par le lecteur de `mscz2xml`. MusicXML est lu si
/// la feature `musicxml-import` est active ; les autres formats renvoient
/// [`Error::FormatNonPrisEnCharge`].
pub fn read(bytes: &[u8], format: Format) -> Result<Score> {
    match format {
        Format::Mscz | Format::Mscx => {
            let container = mscz2xml::MsczContainer::from_bytes(bytes)?;
            Ok(mscz2xml::read::read_score(&container)?)
        }
        Format::MusicXml => hooks::read_musicxml(bytes),
        Format::Mxl | Format::Ly | Format::Abc | Format::Midi => {
            Err(Error::FormatNonPrisEnCharge {
                format,
                sens: Sens::Lecture,
            })
        }
    }
}

/// Écrit le modèle pivot dans le format demandé.
///
/// MusicXML passe par l'export de `mscz2xml` ; `.mxl` réutilise son archive
/// ZIP (`META-INF/container.xml` + `score.xml`), identique à `saveMxl` de
/// MuseScore. MuseScore (`.mscx`/`.mscz`) passe par `scoretk-mscx` si la
/// feature `mscx` est active, MIDI par `scoretk-midi` si la feature `midi`
/// l'est, LilyPond par `scoretk-lilypond` si la feature `lilypond` l'est ; les
/// autres formats renvoient [`Error::FormatNonPrisEnCharge`].
pub fn write(score: &Score, format: Format, options: &Options) -> Result<Vec<u8>> {
    match format {
        Format::MusicXml => Ok(mscz2xml::export::export_musicxml_with(
            score,
            &options.export_options(),
        )?
        .into_bytes()),
        Format::Mxl => Ok(mscz2xml::export::export_mxl_with(
            score,
            &options.export_options(),
        )?),
        Format::Midi => hooks::write_midi(score),
        Format::Mscx => hooks::write_mscx(score),
        Format::Mscz => hooks::write_mscz(score),
        Format::Ly => hooks::write_lilypond(score),
        Format::Abc => Err(Error::FormatNonPrisEnCharge {
            format,
            sens: Sens::Ecriture,
        }),
    }
}
