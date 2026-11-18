// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/types/types.h` et `engraving/rw/` de MuseScore Studio,
// Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust) en
// 2026.
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

//! Tables de conversion `.mscx` → modèle.
//!
//! Les énumérations du modèle utilisent les valeurs de MuseScore ; ce module
//! traduit les noms et entiers écrits dans les fichiers. Les tables de
//! `TConv::fromXml` (`engraving/rw/types/typesconv.cpp`, généré et absent de
//! la copie de référence) sont reconstituées à partir des valeurs rencontrées
//! dans le corpus et de `engraving/types/types.h`.

use scoretk_core::model::Fraction;
use scoretk_core::model::{
    note::tpc, AccidentalType, ArticulationAnchor, BarLineType, BeamMode, ClefType, Duration,
    DurationType, KeyMode, LyricsSyllabic, NoteHeadType, OttavaType, Placement, StemDirection,
    TimeSigType, Transpose, TupletNumberType,
};

/// Figure de durée à partir du nom écrit dans `<durationType>`.
pub(crate) fn duration_type(name: &str) -> Option<DurationType> {
    let kind = match name.trim() {
        "long" => DurationType::Long,
        "breve" => DurationType::Breve,
        "whole" => DurationType::Whole,
        "half" => DurationType::Half,
        "quarter" => DurationType::Quarter,
        "eighth" => DurationType::Eighth,
        "16th" => DurationType::Sixteenth,
        "32nd" => DurationType::ThirtySecond,
        "64th" => DurationType::SixtyFourth,
        "128th" => DurationType::OneHundredTwentyEighth,
        "256th" => DurationType::TwoHundredFiftySixth,
        "512th" => DurationType::FiveHundredTwelfth,
        "1024th" => DurationType::OneThousandTwentyFourth,
        "measure" => DurationType::Measure,
        "zero" => DurationType::Zero,
        "invalid" => DurationType::Invalid,
        _ => return None,
    };
    Some(kind)
}

/// Plus longue [`Duration`] qui tient dans `ticks`, points compris.
///
/// Reproduit `TDuration::truncateToFraction` : on essaie les figures de la plus
/// longue à la plus courte et, pour chacune, le plus grand nombre de points qui
/// ne dépasse pas la durée demandée.
pub(crate) fn duration_from_ticks(ticks: Fraction) -> Duration {
    const ORDER: [DurationType; 13] = [
        DurationType::Long,
        DurationType::Breve,
        DurationType::Whole,
        DurationType::Half,
        DurationType::Quarter,
        DurationType::Eighth,
        DurationType::Sixteenth,
        DurationType::ThirtySecond,
        DurationType::SixtyFourth,
        DurationType::OneHundredTwentyEighth,
        DurationType::TwoHundredFiftySixth,
        DurationType::FiveHundredTwelfth,
        DurationType::OneThousandTwentyFourth,
    ];
    for kind in ORDER {
        for dots in (0..=4u8).rev() {
            let candidate = Duration::with_dots(kind, dots);
            if candidate.ticks() <= ticks {
                return candidate;
            }
        }
    }
    Duration::new(DurationType::Quarter)
}

/// Clef à partir du nom écrit dans `<concertClefType>` / `<defaultClef>`.
///
/// MuseScore force une clef de sol quand aucune clef valide n'a été lue
/// (`TRead::read(Clef*)`) ; les noms exotiques sont rapprochés de la variante
/// voisine du modèle.
pub(crate) fn clef_type(name: &str) -> Option<ClefType> {
    let clef = match name.trim() {
        "G" => ClefType::G,
        "G8vb" | "G8vbp" => ClefType::G8Vb,
        "G8va" | "G8vap" => ClefType::G8Va,
        "G15mb" | "G15mbp" => ClefType::G15Mb,
        "G15ma" | "G15map" => ClefType::G15Ma,
        "G1" => ClefType::G1,
        "F" => ClefType::F,
        "F8vb" | "F8vbp" => ClefType::F8Vb,
        "F8va" | "F8vap" => ClefType::F8Va,
        "F15mb" | "F15mbp" => ClefType::F15Mb,
        "F15ma" | "F15map" => ClefType::F15Ma,
        "F3" => ClefType::FBaritone,
        "F5" => ClefType::FSubbass,
        "C1" => ClefType::C1,
        "C2" => ClefType::C2,
        "C3" => ClefType::C3,
        "C4" => ClefType::C4,
        "C5" => ClefType::C5,
        "PERC" | "PERCUSSION" => ClefType::Perc,
        "PERC2" => ClefType::Perc2,
        "TAB" => ClefType::Tab,
        "TAB4" => ClefType::Tab4,
        "TAB_SERIF" => ClefType::TabSerif,
        "TAB4_SERIF" => ClefType::Tab4Serif,
        _ => return None,
    };
    Some(clef)
}

/// Type de barre de mesure.
pub(crate) fn bar_line_type(name: &str) -> Option<BarLineType> {
    let kind = match name.trim() {
        "normal" => BarLineType::Normal,
        "double" => BarLineType::Double,
        "start-repeat" => BarLineType::StartRepeat,
        "end-repeat" => BarLineType::EndRepeat,
        "end-start-repeat" => BarLineType::EndStartRepeat,
        "broken" => BarLineType::Broken,
        "end" => BarLineType::End,
        "dotted" => BarLineType::Dotted,
        "double-dotted" => BarLineType::DoubleDotted,
        "dashed" => BarLineType::Dashed,
        "short" => BarLineType::Short,
        "double-short" => BarLineType::DoubleShort,
        "tick" => BarLineType::Tick,
        _ => return None,
    };
    Some(kind)
}

/// Type d'altération affichée.
///
/// Le modèle ne conserve que les altérations nommées ; une altération
/// exotique (`AccidentalType::Other` n'a pas de nom dans le fichier) est
/// ignorée plutôt qu'approchée.
pub(crate) fn accidental_type(name: &str) -> AccidentalType {
    match name.trim() {
        "accidentalFlat" => AccidentalType::Flat,
        "accidentalNatural" => AccidentalType::Natural,
        "accidentalSharp" => AccidentalType::Sharp,
        "accidentalDoubleFlat" => AccidentalType::DoubleFlat,
        "accidentalDoubleSharp" => AccidentalType::DoubleSharp,
        "accidentalNaturalFlat" => AccidentalType::NaturalFlat,
        "accidentalNaturalSharp" => AccidentalType::NaturalSharp,
        "accidentalTripleFlat" => AccidentalType::TripleFlat,
        "accidentalTripleSharp" => AccidentalType::TripleSharp,
        _ => AccidentalType::None,
    }
}

/// Ancre verticale d'une articulation (`<anchor>`, valeurs pré-4.1).
pub(crate) fn articulation_anchor(value: Option<i32>) -> ArticulationAnchor {
    match value {
        Some(1) => ArticulationAnchor::Up,
        Some(2) => ArticulationAnchor::Down,
        _ => ArticulationAnchor::Auto,
    }
}

/// Mode d'une armure.
pub(crate) fn key_mode(name: &str) -> KeyMode {
    match name.trim() {
        "major" => KeyMode::Major,
        "minor" => KeyMode::Minor,
        "none" => KeyMode::None,
        _ => KeyMode::Unknown,
    }
}

/// Type graphique d'une indication de mesure (`<subtype>`).
pub(crate) fn time_sig_type(value: Option<i32>) -> TimeSigType {
    match value {
        Some(1) => TimeSigType::FourFour,
        Some(2) => TimeSigType::AllaBreve,
        Some(3) => TimeSigType::CutBach,
        Some(4) => TimeSigType::CutTriple,
        _ => TimeSigType::Normal,
    }
}

/// Affichage du nombre d'un n-olet (`<numberType>`).
pub(crate) fn tuplet_number_type(value: Option<i32>) -> TupletNumberType {
    match value {
        Some(1) => TupletNumberType::Ratio,
        Some(2) => TupletNumberType::None,
        _ => TupletNumberType::Number,
    }
}

/// Mode de ligature (`<BeamMode>`).
///
/// Les modes `begin16` et `begin32` conservent leur niveau de rupture, comme
/// `BeamMode::BEGIN16` / `BeamMode::BEGIN32` dans MuseScore. Les fichiers
/// antérieurs à 4.1 nomment ces ruptures un cran plus haut
/// (`begin64`/`begin32`, voir `rw/read400/tread.cpp`).
pub(crate) fn beam_mode(name: &str) -> BeamMode {
    match name.trim() {
        "none" | "no" => BeamMode::None,
        "begin" => BeamMode::Begin,
        "begin16" | "begin32" => BeamMode::Begin16,
        "begin64" => BeamMode::Begin32,
        "mid" | "mid16" | "mid32" | "mid64" => BeamMode::Mid,
        "end" | "end16" | "end32" | "end64" => BeamMode::End,
        "forward hook" => BeamMode::ForwardHook,
        "backward hook" => BeamMode::BackwardHook,
        _ => BeamMode::Auto,
    }
}

/// Direction de hampe (`<StemDirection>`).
pub(crate) fn stem_direction(name: &str) -> StemDirection {
    match name.trim() {
        "up" => StemDirection::Up,
        "down" => StemDirection::Down,
        _ => StemDirection::Auto,
    }
}

/// Syllabisation d'une syllabe de paroles.
pub(crate) fn syllabic(name: &str) -> LyricsSyllabic {
    match name.trim() {
        "begin" => LyricsSyllabic::Begin,
        "middle" => LyricsSyllabic::Middle,
        "end" => LyricsSyllabic::End,
        _ => LyricsSyllabic::Single,
    }
}

/// Placement vertical (`<placement>`).
pub(crate) fn placement(name: &str) -> Placement {
    match name.trim() {
        "below" => Placement::Below,
        _ => Placement::Above,
    }
}

/// Tête de note (`<head>`, entier de `NoteHeadType`).
pub(crate) fn note_head(value: Option<i32>) -> NoteHeadType {
    match value {
        Some(0) | None => NoteHeadType::Auto,
        Some(1) => NoteHeadType::Breve,
        Some(2) => NoteHeadType::Whole,
        Some(3) => NoteHeadType::Half,
        Some(4) => NoteHeadType::Quarter,
        Some(5) => NoteHeadType::Cross,
        Some(6) => NoteHeadType::Diamond,
        Some(7) => NoteHeadType::Triangle,
        Some(8) => NoteHeadType::Slash,
        Some(9) => NoteHeadType::Square,
        Some(10) => NoteHeadType::X,
        Some(other) => NoteHeadType::Other(other.clamp(0, 255) as u8),
    }
}

/// Type d'ottava (`<Ottava><subtype>`).
pub(crate) fn ottava_type(node: &super::xml::Node) -> OttavaType {
    match node.child("subtype").map(|n| n.trimmed_text()).as_deref() {
        Some("8vb") => OttavaType::OctaveDown,
        Some("15ma") => OttavaType::FifteenthUp,
        Some("15mb") => OttavaType::FifteenthDown,
        _ => OttavaType::OctaveUp,
    }
}

/// Plus petit `tpc` valide (`TPC_MIN`, `Fbbb`).
pub(crate) const TPC_MIN: i8 = -8;

/// Plus grand `tpc` valide (`TPC_MAX`, `B###`).
pub(crate) const TPC_MAX: i8 = 40;

/// Nom de la note désignée par un `tpc`.
///
/// Sert au texte des symboles d'accord : `11` donne `Eb`, `21` donne `C#`.
pub(crate) fn tpc_name(tpc: i8) -> String {
    if tpc == tpc::INVALID {
        return String::new();
    }
    const STEPS: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
    let offset = i32::from(tpc) - i32::from(TPC_MIN);
    let step = STEPS[(offset.rem_euclid(7)) as usize];
    let alteration = offset.div_euclid(7) - 3;
    let suffix = match alteration {
        -3 => "bbb",
        -2 => "bb",
        -1 => "b",
        0 => "",
        1 => "#",
        2 => "##",
        3 => "###",
        _ => "",
    };
    format!("{step}{suffix}")
}

/// Orthographe par défaut d'une hauteur, selon l'armure.
///
/// Approximation de `pitch2tpc` : dièses en armure de dièses, bémols en armure
/// de bémols. Utilisée seulement quand `<tpc>` manque.
pub(crate) fn pitch_to_tpc(pitch: u8, fifths: i8) -> i8 {
    // Index : do, do#, ré, ré#, mi, fa, fa#, sol, sol#, la, la#, si.
    const SHARPS: [i8; 12] = [14, 21, 16, 23, 18, 13, 20, 15, 22, 17, 24, 19];
    const FLATS: [i8; 12] = [14, 9, 16, 11, 18, 13, 8, 15, 10, 17, 12, 19];
    let index = usize::from(pitch % 12);
    if fifths >= 0 {
        SHARPS[index]
    } else {
        FLATS[index]
    }
}

/// Transposition d'une orthographe (`Transpose::transposeTpc`).
///
/// Les altérations suivent MuseScore : un degré diatonique vaut deux pas dans
/// l'ordre des quintes (`F C G D A E B`), et l'altération est recalculée pour
/// que la hauteur écrite soit juste. Renvoie [`tpc::INVALID`] si le résultat
/// sort de `-8..=40` (`Fbbb`…`B###`).
pub(crate) fn transpose_tpc(value: i8, interval: Transpose) -> i8 {
    const STEP_SEMITONES: [i32; 7] = [5, 0, 7, 2, 9, 4, 11];

    if value == tpc::INVALID {
        return value;
    }
    let offset = i32::from(value) - i32::from(TPC_MIN);
    let step = offset.rem_euclid(7) as usize;
    let alteration = offset.div_euclid(7) - 3;
    let new_step = (step as i32 + 2 * interval.diatonic).rem_euclid(7) as usize;
    let target = (STEP_SEMITONES[step] + alteration + interval.chromatic).rem_euclid(12);
    let mut new_alteration = (target - STEP_SEMITONES[new_step]).rem_euclid(12);
    if new_alteration > 6 {
        new_alteration -= 12;
    }
    let result = i32::from(TPC_MIN) + (new_alteration + 3) * 7 + new_step as i32;
    if (i32::from(TPC_MIN)..=i32::from(TPC_MAX)).contains(&result) {
        result as i8
    } else {
        tpc::INVALID
    }
}

/// Décalage du nombre de quintes produit par une transposition chromatique.
///
/// Transposer vers le haut de `chromatic` demi-tons décale l'armure de
/// `(7 * chromatic) mod 12` quintes, ramené dans `-5..=6`.
pub(crate) fn transpose_key_fifths(fifths: i8, chromatic: i32) -> i8 {
    let delta = (7 * i64::from(chromatic)).rem_euclid(12);
    let delta = if delta > 6 { delta - 12 } else { delta };
    let shifted = i64::from(fifths) + delta;
    shifted.clamp(i64::from(i8::MIN), i64::from(i8::MAX)) as i8
}
