// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de l'export MusicXML de `importexport/musicxml/internal/export/` de
// MuseScore Studio, Copyright (C) MuseScore Limited and others, modifié
// (réécrit en Rust) en 2026.
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

//! Portage en Rust de l'export MusicXML de MuseScore.
//!
//! Cette bibliothèque est volontairement sans entrées/sorties : elle travaille
//! uniquement sur des octets en mémoire, sans système de fichiers, sans thread
//! et sans dépendance C. Elle doit donc compiler pour `wasm32-unknown-unknown`,
//! Android et iOS.
//!
//! Point d'entrée de cette première étape : [`MsczContainer`], qui extrait le
//! fichier `.mscx` principal d'un conteneur `.mscz` (ou accepte un `.mscx` nu),
//! et [`detect_version`], qui lit la version du format MuseScore.
//!
//! Le module [`model`] contient le modèle de partition en mémoire, sans aucune
//! lecture ni écriture XML : il est rempli par les lecteurs `.mscx` et parcouru
//! par l'export MusicXML. Le module [`export`] produit le document MusicXML
//! `partwise` (ou l'archive `.mxl`) à partir de ce modèle.
//!
//! Pour la chaîne complète en un appel, voir [`convert`] et [`ConvertOptions`].

mod container;
mod error;
pub mod export;
pub mod model;
pub mod read;

pub use container::{detect_version, MscVersion, MsczContainer};
pub use error::Error;
pub use export::ExportOptions;

/// Résultat de la bibliothèque.
pub type Result<T> = std::result::Result<T, Error>;

/// Format de sortie de [`convert`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// MusicXML `partwise` non compressé (`.musicxml`, `.xml`).
    #[default]
    MusicXml,
    /// MusicXML compressé, archive ZIP `.mxl`.
    Mxl,
}

/// Options de [`convert`].
///
/// Regroupe le choix du format de sortie et les [`ExportOptions`] transmises à
/// l'export MusicXML (logiciel annoncé, date d'encodage, hauteurs en sons
/// réels).
///
/// ```
/// use mscz2xml::{ConvertOptions, ExportOptions, OutputFormat};
///
/// let options = ConvertOptions {
///     format: OutputFormat::Mxl,
///     export: ExportOptions {
///         encoding_date: "2026-01-01".to_owned(),
///         ..ExportOptions::default()
///     },
/// };
/// assert_eq!(options.format, OutputFormat::Mxl);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvertOptions {
    /// Format de sortie.
    pub format: OutputFormat,
    /// Options transmises à l'export MusicXML.
    pub export: ExportOptions,
}

/// Convertit un `.mscz` ou un `.mscx` en MusicXML, en une seule étape.
///
/// Cette fonction est le point d'entrée de bout en bout : elle lit le
/// conteneur ([`MsczContainer`]), détecte la version du format et la répartit
/// vers le lecteur correspondant ([`read::read_score`]), puis exporte la
/// partition dans le format demandé par `options`.
///
/// Tout se passe en mémoire : aucun fichier n'est ouvert, aucune horloge n'est
/// lue. La date d'encodage vient de [`ExportOptions::encoding_date`], ce qui
/// rend les sorties reproductibles ; c'est à l'appelant (la CLI, un bindings
/// mobile) de fournir la date du jour s'il la veut.
///
/// # Exemple
///
/// ```
/// use mscz2xml::{convert, ConvertOptions, ExportOptions, OutputFormat};
///
/// let mscx = br#"<museScore version="4.60">
///   <Score>
///     <Division>480</Division>
///     <Part id="1">
///       <Staff><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
///       <Instrument id="piano">
///         <InstrumentLabel><longName>Piano</longName></InstrumentLabel>
///         <instrumentId>keyboard.piano</instrumentId>
///       </Instrument>
///     </Part>
///     <Staff>
///       <Measure>
///         <voice>
///           <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
///           <Chord>
///             <durationType>whole</durationType>
///             <Note><pitch>60</pitch><tpc>14</tpc></Note>
///           </Chord>
///         </voice>
///       </Measure>
///     </Staff>
///   </Score>
/// </museScore>"#;
///
/// let options = ConvertOptions {
///     format: OutputFormat::MusicXml,
///     export: ExportOptions {
///         encoding_date: "2026-01-01".to_owned(),
///         ..ExportOptions::default()
///     },
/// };
///
/// let musicxml = convert(mscx, &options)?;
/// let text = String::from_utf8(musicxml)?;
/// assert!(text.contains("<score-partwise"));
/// assert!(text.contains("<encoding-date>2026-01-01</encoding-date>"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Erreurs
///
/// Renvoie une [`Error`] si le conteneur est illisible, si la version
/// MuseScore n'est pas prise en charge ou si l'archive `.mxl` ne peut pas être
/// écrite.
pub fn convert(input: &[u8], options: &ConvertOptions) -> Result<Vec<u8>> {
    let container = MsczContainer::from_bytes(input)?;
    let score = read::read_score(&container)?;

    match options.format {
        OutputFormat::MusicXml => {
            Ok(export::export_musicxml_with(&score, &options.export)?.into_bytes())
        }
        OutputFormat::Mxl => export::export_mxl_with(&score, &options.export),
    }
}
