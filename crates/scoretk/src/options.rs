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

//! Options d'écriture communes à tous les formats.

#[cfg(feature = "musicxml-export")]
use scoretk_musicxml_export::ExportOptions;

/// Options d'écriture MIDI.
///
/// Reprend les réglages utiles de `scoretk_midi_export::MidiOptions` sans
/// exposer la crate optionnelle : la façade reste utilisable avec les features
/// désactivées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiOptions {
    /// Déroule les reprises, voltas et sauts dans la sortie MIDI. Vrai par
    /// défaut, comme l'option `io/midi/expandRepeats` de MuseScore.
    pub expand_repeats: bool,
    /// Écrit les textes et repères que le SMF porte (`FF 01`, `FF 02`,
    /// `FF 06`) en plus des métadonnées habituelles. Faux par défaut.
    pub smf_text: bool,
}

impl Default for MidiOptions {
    fn default() -> Self {
        Self {
            expand_repeats: true,
            smf_text: false,
        }
    }
}

/// Options transmises à [`write`](crate::write).
///
/// Les valeurs par défaut sont celles de l'export MusicXML : logiciel annoncé
/// et date d'encodage constantes (sortie reproductible). La CLI substitue la
/// date du jour à `encoding_date` si l'utilisateur ne la fige pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Contenu de `<software>` (MusicXML).
    pub software: String,
    /// Contenu de `<encoding-date>` (MusicXML, `AAAA-MM-JJ`).
    pub encoding_date: String,
    /// Exporter les hauteurs en sons réels (MusicXML). `None` suit le style
    /// `concertPitch` enregistré dans la partition ; `Some(v)` surcharge.
    pub concert_pitch: Option<bool>,
    /// Réglages de la sortie MIDI (reprises).
    pub midi: MidiOptions,
    /// Vue moderne optionnelle du chant grégorien : clé de sol (`8vb` si le
    /// registre est bas) et portée à 5 lignes au lieu de la clé d'ut et des
    /// 4 lignes du `.gabc`. Additif : `false` par défaut.
    pub gabc_modern_clef: bool,
    /// Porteurs SMF standard au-delà de l'exporteur 4.7.4 (`FF 01`/`FF 02`/
    /// `FF 03`/`FF 06`/`FF 07`, balises RP-026, ticks exacts à la lecture).
    /// Additif : `false` par défaut, donc les octets MIDI restent conformes
    /// aux références `*.ref.mid`; voir `docs/agents/midi.md`.
    pub midi_full_support: bool,
    /// Porteurs MEI 5.1 additionnels au-delà de l'exporteur 4.7.4
    /// (vitesses des nuances, identité des instruments, ordre des credits…).
    /// Additif : `false` par défaut, donc les octets MEI restent conformes
    /// aux références `*.ref.mei` ; voir `docs/agents/mei.md` §10.
    pub mei_full_support: bool,
    /// Carrier SMuFL optionnel des neumes grégoriens (MusicXML). Additif,
    /// `false` par défaut : voir `docs/agents/gabc.md` §14.
    pub gabc_neume_smufl: bool,
    /// Porteurs XSD 4.0 standard au-delà de l'exporteur 4.7.4. Additif :
    /// `false` par défaut, donc les octets MusicXML restent conformes aux
    /// références `*.ref.musicxml`; voir `docs/agents/musicxml-export.md` §55.
    pub xml_full_support: bool,
    /// Export MusicXML additif : préserver les valeurs du modèle au lieu des
    /// valeurs de mise en page déduites par MuseScore 4.7.4. Faux par défaut,
    /// pour ne pas changer les octets des références.
    pub xml_lossless_round_trip: bool,
}

impl Default for Options {
    fn default() -> Self {
        // Les trois premiers champs viennent de l'export MusicXML. Sans la
        // feature `musicxml-export`, aucun MusicXML ne peut être écrit : ils
        // restent vides plutôt que de dupliquer les constantes de l'export.
        #[cfg(feature = "musicxml-export")]
        let (software, encoding_date, concert_pitch) = {
            let export = ExportOptions::default();
            (export.software, export.encoding_date, export.concert_pitch)
        };
        #[cfg(not(feature = "musicxml-export"))]
        let (software, encoding_date, concert_pitch) = (String::new(), String::new(), None);

        Self {
            software,
            encoding_date,
            concert_pitch,
            midi: MidiOptions::default(),
            midi_full_support: false,
            gabc_modern_clef: false,
            mei_full_support: false,
            gabc_neume_smufl: false,
            xml_full_support: false,
            xml_lossless_round_trip: false,
        }
    }
}

#[cfg(feature = "musicxml-export")]
impl Options {
    /// Convertit vers les options de l'export MusicXML.
    pub(crate) fn export_options(&self) -> ExportOptions {
        ExportOptions {
            software: self.software.clone(),
            encoding_date: self.encoding_date.clone(),
            concert_pitch: self.concert_pitch,
            // L'export MusicXML ordinaire ne pose aucun identifiant : les
            // références MuseScore 4.7.4 n'en contiennent pas. Seul
            // `write_playback` force `stable_ids`.
            stable_ids: false,
            gabc_modern_clef: self.gabc_modern_clef,
            gabc_neume_smufl: self.gabc_neume_smufl,
            full_support: self.xml_full_support,
            lossless_round_trip: self.xml_lossless_round_trip,
        }
    }
}
