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

//! Identifiants XML stables, calculés à partir des seules positions du modèle.
//!
//! L'export MusicXML n'écrit aucun `id` par défaut, pour rester identique aux
//! références produites par MuseScore 4.7.4. Ces fonctions ne servent qu'à
//! l'option [`crate::export::ExportOptions::stable_ids`] : elles sont pures et
//! ne dépendent ni de l'horloge, ni d'un ordre d'itération de table de hachage.
//!
//! # Mesures
//!
//! [`measure_xml_id`] produit `p{part}-m{measure}`. La partie est incluse dans
//! l'identifiant pour que celui-ci soit **unique dans tout le document** : un
//! document MusicXML `partwise` répète la *présence* du même `<measure>` dans
//! chaque `<part>`, et sans le préfixe de partie le même `id` apparaîtrait une
//! fois par partie. Avec `p{part}`, chaque `<measure>` du document a un `id`
//! distinct, conformément à la recommandation MusicXML d'unicité globale. Les
//! indices sont ceux du modèle (`Score::parts`, `Score::measures`), pas le
//! numéro affiché : une mesure d'anacrouse ou un saut de section ne décale
//! ainsi jamais les identifiants.
//!
//! # Notes
//!
//! [`note_xml_id`] produit `m{measure}-s{staff}-v{voice}-c{chord}-n{note}`.
//! L'identifiant est unique par `(staff, measure)` : `c{chord}` est le rang du
//! `ChordRest` dans sa voix, **silences compris**, dans l'ordre de parcours de
//! `writeMeasureTracks` (`engraving/rw/write/exportmusicxml.cpp`), et
//! `n{note}` l'index de la note dans `Chord::notes`. Un silence est une
//! `<note>` : il reçoit le même identifiant avec `n0`, sans collision possible
//! avec un accord, puisque deux `ChordRest` d'une même voix ont des rangs
//! distincts.
//!
//! Les ornements (`grace`) précèdent ou suivent l'accord principal partageant
//! le même `ChordRest` ; ils reçoivent [`grace_note_xml_id`], qui conserve le
//! rang `c{chord}` et ajoute la position dans `Chord::grace_chords`, le côté
//! (`b` avant, `a` après) et l'index de la note dans l'accord d'agrément.
//!
//! Les trémolos à deux accords ne demandent pas de fonction dédiée : dans le
//! modèle, les deux accords sont deux `ChordRest` distincts (`TremoloKind::
//! Start` et `Stop`) et reçoivent donc des rangs différents ; [`note_xml_id`]
//! suffit.

/// Identifiant XML d'une mesure : `p{part}-m{measure}`.
///
/// `part` et `measure` sont des indices du modèle (base zéro). L'identifiant
/// est unique dans tout le document : la présence répétée de la même mesure
/// dans chaque `<part>` du document `partwise` est distinguée par le préfixe
/// de partie.
pub fn measure_xml_id(part: usize, measure: usize) -> String {
    format!("p{part}-m{measure}")
}

/// Identifiant XML d'une note ou d'un silence :
/// `m{measure}-s{staff}-v{voice}-c{chord}-n{note}`.
///
/// `chord` est le rang du `ChordRest` dans sa voix, silences compris ;
/// `note` vaut zéro pour un silence. L'identifiant est unique par
/// `(staff, measure)`.
pub fn note_xml_id(measure: usize, staff: u32, voice: u8, chord: usize, note: usize) -> String {
    format!("m{measure}-s{staff}-v{voice}-c{chord}-n{note}")
}

/// Identifiant XML d'une note d'ornement :
/// `m{measure}-s{staff}-v{voice}-c{chord}-g{grace_index}{b|a}n{note}`.
///
/// `grace_index` est la position dans `Chord::grace_chords` et `before`
/// distingue un ornement avant (`b`) d'un ornement après (`a`) l'accord
/// principal. `note` est l'index dans l'accord d'agrément.
pub fn grace_note_xml_id(
    measure: usize,
    staff: u32,
    voice: u8,
    chord: usize,
    grace_index: usize,
    note: usize,
    before: bool,
) -> String {
    let side = if before { 'b' } else { 'a' };
    format!("m{measure}-s{staff}-v{voice}-c{chord}-g{grace_index}{side}n{note}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesure_identifiant_stable() {
        assert_eq!(measure_xml_id(0, 0), "p0-m0");
        assert_eq!(measure_xml_id(2, 17), "p2-m17");
    }

    #[test]
    fn note_identifiant_stable() {
        assert_eq!(note_xml_id(0, 0, 0, 0, 0), "m0-s0-v0-c0-n0");
        assert_eq!(note_xml_id(3, 5, 2, 9, 1), "m3-s5-v2-c9-n1");
    }

    #[test]
    fn ornement_avant_et_apres() {
        assert_eq!(
            grace_note_xml_id(1, 0, 0, 2, 0, 0, true),
            "m1-s0-v0-c2-g0bn0"
        );
        assert_eq!(
            grace_note_xml_id(1, 0, 0, 2, 3, 1, false),
            "m1-s0-v0-c2-g3an1"
        );
    }
}
