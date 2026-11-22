// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Détection d'un MIDI mécanique : toutes les attaques et fins de note sont
// déjà exactement sur la grille de quantification. Extension volontaire
// au-delà de MuseScore 4.7.4, documentée dans docs/agents/midi.md.
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

//! Détection des pistes MIDI déjà parfaitement quantifiées.
//!
//! `Quantize::isHumanPerformance` (`importmidi_quant.cpp:399-439`) ne regarde
//! que les attaques tombant sur un temps, et déclare l'exécution humaine quand
//! moins de 60 % d'entre elles sont déjà sur la grille. Ici, pour reconnaître
//! un MIDI mécanique (par exemple celui que LilyPond grave : attaques et
//! releases exactement aux valeurs notées), **les deux extrémités** de chaque
//! note doivent coïncider avec la grille.

use scoretk_core::model::Fraction;

use super::chord::MTrack;
use super::quant::quantize_value;

/// Vrai si toutes les attaques et fins de note de la piste sont sur la grille.
pub fn is_mechanical_track(track: &MTrack, basic_quant: Fraction) -> bool {
    if track.chords.is_empty() || basic_quant.is_zero() {
        return false;
    }
    for (on_time, chord) in track.chords.iter() {
        if quantize_value(on_time, basic_quant) != on_time {
            return false;
        }
        for note in &chord.notes {
            if quantize_value(note.off_time, basic_quant) != note.off_time {
                return false;
            }
        }
    }
    true
}

/// Vrai si toutes les pistes portant des notes sont mécaniques.
///
/// Les pistes de percussion sont exclues : `shortenDrumNote`
/// (`importmidi_simplify.cpp:170-208`) raccourcit les notes de batterie même
/// sur une grille parfaite, et le court-circuiter fait régresser les fichiers
/// amont `perc_drums` et `perc_remove_ties`.
pub fn is_mechanical_file(tracks: &[MTrack], basic_quant: Fraction) -> bool {
    if tracks.iter().any(|track| track.drum_track) {
        return false;
    }
    let with_notes: Vec<&MTrack> = tracks
        .iter()
        .filter(|track| !track.chords.is_empty())
        .collect();
    !with_notes.is_empty()
        && with_notes
            .iter()
            .all(|track| is_mechanical_track(track, basic_quant))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{MidiChord, MidiNote};

    fn track_with(on_time: Fraction, off_time: Fraction) -> MTrack {
        let mut track = MTrack::default();
        let chord = MidiChord {
            notes: vec![MidiNote::new(60, 80, off_time, on_time)],
            ..MidiChord::default()
        };
        track.chords.insert(on_time, chord);
        track
    }

    #[test]
    fn grille_exacte_est_mecanique() {
        let quant = Fraction::new(1, 16);
        let track = track_with(Fraction::new(1, 4), Fraction::new(1, 2));
        assert!(is_mechanical_track(&track, quant));
        assert!(is_mechanical_file(&[track], quant));
    }

    #[test]
    fn fin_hors_grille_n_est_pas_mecanique() {
        let quant = Fraction::new(1, 16);
        let track = track_with(Fraction::new(1, 4), Fraction::new(23, 96));
        assert!(!is_mechanical_track(&track, quant));
        assert!(!is_mechanical_file(&[track], quant));
    }

    #[test]
    fn une_piste_de_batterie_desactive_lextension() {
        let quant = Fraction::new(1, 16);
        let good = track_with(Fraction::new(1, 4), Fraction::new(1, 2));
        let drum = MTrack {
            drum_track: true,
            ..track_with(Fraction::ZERO, Fraction::new(1, 8))
        };
        assert!(!is_mechanical_file(&[good, drum], quant));
    }

    #[test]
    fn une_piste_hors_grille_suffit_a_rejeter_le_fichier() {
        let quant = Fraction::new(1, 16);
        let good = track_with(Fraction::new(1, 4), Fraction::new(1, 2));
        let bad = track_with(Fraction::new(1, 4), Fraction::new(23, 96));
        assert!(!is_mechanical_file(&[good, bad], quant));
    }
}
