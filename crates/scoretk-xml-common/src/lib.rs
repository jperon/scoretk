// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Ce fichier est original : il rassemble les utilitaires XML en mémoire déjà
// utilisés par les crates MusicXML, afin que les autres formats XML (MEI) les
// partagent (règle des crates `-common` de `docs/agents/developpement.md` §1).
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

//! Arbre XML minimal et écriture XML déterministe, sans entrée/sortie.
//!
//! Le lecteur remplace le `XmlStreamReader` en flux de MuseScore par un arbre
//! résidant en mémoire ; l'écrivain reproduit l'indentation de
//! `engraving/rw/xmlwriter.cpp`. Aucun de ces deux modules ne contient de
//! logique de format.

pub mod tree;
pub mod writer;

pub use tree::{parse, Element};
pub use writer::XmlWriter;

/// Erreur commune aux utilitaires XML.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Le document XML est mal formé.
    #[error("XML invalide : {0}")]
    Xml(#[from] quick_xml::Error),

    /// Attribut mal formé.
    #[error("attribut XML invalide : {0}")]
    Attribute(#[from] quick_xml::events::attributes::AttrError),

    /// Le document n'a pas de racine.
    #[error("document XML sans racine")]
    NoRoot,
}

/// Résultat des utilitaires XML.
pub type Result<T> = std::result::Result<T, Error>;

/// Types d'items que la XSD MusicXML 4.0 peut porter.
///
/// Chaque kind de l'inventaire est déclaré : `text-line` est porté par
/// `xsd:dashes` / `xsd:bracket` avec `xsd:words`, et `tremolo-bar` par
/// `xsd:bend` / `xsd:with-bar` (voir `docs/agents/format-ref/musicxml.md`
/// « Repeats, jumps, directions, lyrics »). Aucun item n'est retiré du
/// dénominateur : c'est une déclaration de format, pas une table d'exclusion
/// de fichiers ni une relaxation de la clé de comparaison.
pub const XML_ITEM_KINDS: &[&str] = &[
    "annotation",
    "arpeggio",
    "articulate",
    "articulation",
    "barline",
    "barline-end",
    "barline-start",
    "beam",
    "breath",
    "channel",
    "chord-line",
    "clef",
    "credit",
    "credit-position",
    "dynamic",
    "dynamic-velocity",
    "fermata",
    "figured-bass",
    "glissando",
    "grace",
    "grace-note",
    "hairpin",
    "harmony",
    "instrument",
    "instrument-change",
    "jump",
    "key-sig",
    "line-break",
    "lyric",
    "marker",
    "measure",
    "measure-repeat",
    "multi-measure-rest",
    "note",
    "note-head",
    "ornament",
    "ottava",
    "pan",
    "pedal",
    "playback-transposition",
    "program",
    "rehearsal-mark",
    "repeat-count",
    "rest",
    "section-break",
    "slur",
    "spanner",
    "staff-text",
    "stem",
    "style",
    "system-text",
    "tbox-position",
    "tbox-text",
    "tempo",
    "text-line",
    "tie",
    "time-sig",
    "title",
    "tremolo",
    "tremolo-bar",
    "trill",
    "tuplet",
    "tuplet-bracket",
    "tuplet-member",
    "user-velocity",
    "velocity",
    "volta",
    "volume",
];
