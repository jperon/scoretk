//! Direction de hampe (`Chord::up()` / `StemDirection`).
//!
//! MuseScore calcule la direction d'une hampe pendant la mise en page
//! (`ChordLayout::computeUp` dans `rendering/score/chordlayout.cpp`) :
//!
//! 1. une direction explicite l'emporte, sauf si l'accord est ligaturé ou
//!    appartient à un trémolo à deux notes ;
//! 2. un accord ligaturé prend la direction de sa ligature ;
//! 3. un trémolo à deux notes prend la sienne ;
//! 4. une portée à plusieurs voix met les hampes vers le haut sur les voix
//!    paires (1 et 3) et vers le bas sur les voix impaires (2 et 4) ;
//! 5. une note d'ornement va vers le haut ;
//! 6. sinon, la direction se déduit des hauteurs : une note sous la ligne
//!    médiane va vers le haut, au-dessus vers le bas, et un accord est
//!    départagé par paires en partant des extrêmes
//!    (`ChordLayout::computeAutoStemDirection`).
//!
//! La géométrie n'étant pas dans le modèle, on remplace les positions par la
//! ligne de chaque note, calculée comme `Note::updateLine` : `relStep` à partir
//! du `Tpc` et de la hauteur écrite, et de la clef en vigueur. Les cas qui
//! dépendent de la mise en page (portées de tablature, accords croisés entre
//! portées, ligatures d'ornements) retombent sur les règles ci-dessus.

use crate::model::{
    Chord, ClefType, Fraction, Measure, Note, Score, SegmentItem, SegmentType, StaffId,
    StemDirection, Tick, Track,
};

use super::pitch;

/// Direction de hampe d'un accord, comme `Chord::up()`.
///
/// `start` est le tick du segment et `actual_ticks` la durée réelle de
/// l'accord (zéro pour un ornement). `beam_up` donne la direction de la
/// ligature quand l'accord en fait partie.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stem_up(
    score: &Score,
    chord: &Chord,
    measure: &Measure,
    track: Track,
    start: Tick,
    actual_ticks: Fraction,
    concert_pitch: bool,
    transpose_chromatic: i32,
    beam_up: Option<bool>,
) -> bool {
    // 1. Direction explicite : elle ne s'applique pas à un accord ligaturé ni
    //    à un trémolo à deux notes (`computeUp`).
    if chord.stem != StemDirection::Auto && beam_up.is_none() && !chord.tremolo_two_chord {
        return chord.stem == StemDirection::Up;
    }

    // 2. Accord ligaturé : la ligature impose sa direction.
    if let Some(up) = beam_up {
        return up;
    }

    // 3. Trémolo à deux notes : `computeUpTremoloCase`. La moitié n'est pas
    //    modélisée ; les accords d'un trémolo suivent la règle de hauteur.
    if chord.tremolo_two_chord {
        return auto_up(
            score,
            chord,
            start,
            track,
            concert_pitch,
            transpose_chromatic,
        );
    }

    // 4. Portée à plusieurs voix au même moment.
    if has_other_voices(measure, track, start, actual_ticks) {
        return track.raw().is_multiple_of(2);
    }

    // 5. Note d'ornement.
    if chord.grace.is_some() {
        return true;
    }

    // 6. Accord croisé entre portées : la hampe suit le sens du déplacement
    //    (`ldata->up = staffMove() > 0`).
    if chord.staff_move != 0 {
        return chord.staff_move > 0;
    }

    // 7. Hauteurs des notes.
    auto_up(
        score,
        chord,
        start,
        track,
        concert_pitch,
        transpose_chromatic,
    )
}

/// Vrai si une autre voix de la même portée occupe l'intervalle donné
/// (`Measure::hasVoices`).
///
/// Le C++ ignore les éléments invisibles ; le modèle ne conserve pas la
/// visibilité d'un accord ou d'un silence, on considère donc tout élément
/// présent comme audible.
pub(crate) fn has_other_voices(
    measure: &Measure,
    track: Track,
    start: Tick,
    actual_ticks: Fraction,
) -> bool {
    if !actual_ticks.is_positive() {
        return false;
    }
    let end = start + actual_ticks;
    let staff = track.staff();
    for segment in &measure.segments {
        if segment.kind != SegmentType::ChordRest {
            continue;
        }
        if segment.tick >= end {
            break;
        }
        for voice in 0..crate::model::VOICES as u8 {
            if voice == track.voice() {
                continue;
            }
            let other = Track::new(staff, voice);
            let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(other) else {
                continue;
            };
            // Un silence « gap » ne compte pas (`Rest::isGap`).
            if chord_rest.rest_ref().is_some_and(|rest| rest.gap) {
                continue;
            }
            let ticks = chord_rest.ticks_in_measure(measure.len);
            let other_end = segment.tick + ticks;
            if other_end > start {
                return true;
            }
        }
    }
    false
}

/// Direction automatique d'après les lignes des notes.
fn auto_up(
    score: &Score,
    chord: &Chord,
    tick: Tick,
    track: Track,
    concert_pitch: bool,
    transpose_chromatic: i32,
) -> bool {
    let clef = active_clef(score, track.staff(), tick, concert_pitch);
    let offset = clef_pitch_offset(clef);
    let mut lines: Vec<i32> = chord
        .notes
        .iter()
        .map(|id| note_distance(score, *id, offset, concert_pitch, transpose_chromatic))
        .collect();
    lines.sort_unstable();
    auto_stem_direction(&lines) > 0
}

/// `ChordLayout::computeAutoStemDirection` : compare les notes par paires en
/// partant des extrêmes. Renvoie `1` (haut), `-1` (bas) ou `0` (milieu).
pub(crate) fn auto_stem_direction(distances: &[i32]) -> i32 {
    let mut left = 0;
    let mut right = distances.len().saturating_sub(1);
    while left <= right && !distances.is_empty() {
        let left_note = distances[left];
        let right_note = distances[right];
        let net = left_note + right_note;
        if net == 0 {
            left += 1;
            if right == 0 {
                break;
            }
            right -= 1;
            continue;
        }
        return if net > 0 { 1 } else { -1 };
    }
    0
}

/// Distance à la ligne médiane de chaque note d'un accord, pour une position
/// donnée (la clef peut changer au fil de la mesure).
pub(crate) fn chord_distances(
    score: &Score,
    chord: &Chord,
    tick: Tick,
    staff: StaffId,
    concert_pitch: bool,
    transpose_chromatic: i32,
) -> Vec<i32> {
    let staff = moved_staff(score, staff, chord.staff_move);
    let clef = active_clef(score, staff, tick, concert_pitch);
    let offset = clef_pitch_offset(clef);
    chord
        .notes
        .iter()
        .map(|id| note_distance(score, *id, offset, concert_pitch, transpose_chromatic))
        .collect()
}

/// Distance d'une note à la ligne médiane (`Note::line() - middleLine`).
fn note_distance(
    score: &Score,
    id: crate::model::NoteId,
    pitch_offset: i32,
    concert_pitch: bool,
    transpose_chromatic: i32,
) -> i32 {
    let note: &Note = score.note(id);
    let (pitch, tpc) = written_pitch_and_tpc(note, concert_pitch, transpose_chromatic);
    let line = pitch_offset - abs_step(tpc, pitch);
    // `StaffType::middleLine()` vaut 4 pour une portée standard.
    line - 4
}

/// Hauteur écrite et `Tpc` utilisés par la mise en page.
fn written_pitch_and_tpc(note: &Note, concert_pitch: bool, transpose_chromatic: i32) -> (i32, i8) {
    if concert_pitch {
        (i32::from(note.pitch), note.tpc)
    } else {
        (
            i32::from(note.pitch) - transpose_chromatic,
            if pitch::tpc_is_valid(note.tpc_transposed) {
                note.tpc_transposed
            } else {
                note.tpc
            },
        )
    }
}

/// Portée effective d'un accord (`Chord::vStaffIdx`).
pub(crate) fn moved_staff(score: &Score, staff: StaffId, movement: i8) -> StaffId {
    let target = i64::from(staff.raw()) + i64::from(movement);
    if target >= 0 && (target as usize) < score.staves.len() {
        StaffId::from_raw(target as u32)
    } else {
        staff
    }
}

/// Clef en vigueur pour une portée à une position (`Staff::clef`).
fn active_clef(score: &Score, staff: StaffId, tick: Tick, concert_pitch: bool) -> ClefType {
    let track = Track::new(staff, 0);
    // Les segments sont triés : on garde la dernière clef avant la position
    // courante.
    let mut found = None;
    for candidate in &score.measures {
        if candidate.start_tick > tick {
            break;
        }
        for segment in &candidate.segments {
            if segment.tick > tick {
                break;
            }
            if segment.kind != SegmentType::HeaderClef && segment.kind != SegmentType::Clef {
                continue;
            }
            if let Some(SegmentItem::Clef(clef)) = segment.element(track) {
                found = Some(*clef);
            }
        }
    }
    let clef = found.unwrap_or_else(|| score.staff(staff).default_clef);
    if concert_pitch {
        clef.concert
    } else if clef.transposed != ClefType::Invalid {
        clef.transposed
    } else {
        clef.concert
    }
}

/// `ClefInfo::pitchOffset` : position de la ligne 0 pour chaque clef.
fn clef_pitch_offset(clef: ClefType) -> i32 {
    match clef {
        ClefType::G => 45,
        ClefType::G15Mb => 31,
        ClefType::G8Vb => 38,
        ClefType::G8Va => 52,
        ClefType::G15Ma => 59,
        ClefType::G1 => 47,
        ClefType::C1 => 43,
        ClefType::C2 => 41,
        ClefType::C3 => 39,
        ClefType::C4 => 37,
        ClefType::C5 => 35,
        ClefType::F => 33,
        ClefType::F15Mb => 19,
        ClefType::F8Vb => 26,
        ClefType::F8Va => 40,
        ClefType::F15Ma => 47,
        ClefType::FBaritone => 35,
        ClefType::FSubbass => 31,
        ClefType::Perc | ClefType::Perc2 => 45,
        ClefType::Tab | ClefType::Tab4 | ClefType::TabSerif | ClefType::Tab4Serif => 45,
        ClefType::Invalid => 45,
    }
}

/// `absStep(tpc, pitch)` : degré diatonique absolu (`engraving/dom/utils.cpp`).
fn abs_step(tpc: i8, pitch: i32) -> i32 {
    if !pitch::tpc_is_valid(tpc) {
        return midi_abs_step(pitch);
    }
    let step = tpc_step(tpc);
    let mut line = step + (pitch / 12) * 7;
    let tpc_pitch = tpc_pitch(tpc);
    if tpc_pitch < 0 {
        line += 7;
    } else {
        line -= (tpc_pitch / 12) * 7;
    }
    line
}

/// Degré diatonique d'un `Tpc` (`tpc2step`).
fn tpc_step(tpc: i8) -> i32 {
    const STEPS: [i32; 7] = [3, 0, 4, 1, 5, 2, 6];
    STEPS[(i32::from(tpc) + 8).rem_euclid(7) as usize]
}

/// Hauteur de référence d'un `Tpc` dans l'octave (`tpc2pitch`).
fn tpc_pitch(tpc: i8) -> i32 {
    const NATURAL: [i32; 7] = [5, 0, 7, 2, 9, 4, 11];
    NATURAL[tpc_step(tpc) as usize] + pitch::tpc_alter(tpc)
}

/// Degré diatonique absolu d'une hauteur MIDI sans `Tpc` exploitable.
fn midi_abs_step(pitch: i32) -> i32 {
    const STEPS: [i32; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
    (pitch / 12) * 7 + STEPS[(pitch % 12) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direction_automatique_par_paires() {
        // Une note sous la ligne médiane (distance positive) va vers le haut.
        assert_eq!(auto_stem_direction(&[2]), 1);
        // Au-dessus (distance négative) vers le bas.
        assert_eq!(auto_stem_direction(&[-2]), -1);
        // Exactement sur la ligne médiane : `direction > 0` est faux, donc bas.
        assert_eq!(auto_stem_direction(&[0]), 0);
        // Un accord de part et d'autre s'annule, puis décide à l'intérieur.
        assert_eq!(auto_stem_direction(&[-2, 0, 2]), 0);
        assert_eq!(auto_stem_direction(&[-3, -1, 2]), -1);
        assert_eq!(auto_stem_direction(&[-2, 1, 3]), 1);
    }

    #[test]
    fn degre_diatonique_ut_central() {
        // Do 4 en clef de sol : dixième degré sous la première ligne.
        assert_eq!(abs_step(crate::model::tpc::C, 60), 35);
        assert_eq!(45 - abs_step(crate::model::tpc::C, 60), 10);
        // Si 4, ligne médiane (4).
        assert_eq!(45 - abs_step(crate::model::tpc::B, 71), 4);
    }
}
