// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 les auteurs de mscz2xml
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

//! Conversions des noms MuseScore vers les énumérations du modèle.
//!
//! Les tables reproduisent `TConv::fromXml` (`engraving/types/typesconv.cpp`,
//! généré) pour les valeurs rencontrées par le noyau musical. Les noms inconnus
//! renvoient `None` : le lecteur les ignore alors silencieusement.

use crate::model::{
    AccidentalType, BarLineType, BeamMode, ClefType, DurationType, GraceType, KeyMode,
    LyricsSyllabic, NoteHeadGroup, NoteHeadType, TimeSigType,
};

/// Figure de durée (`durationType`).
pub fn duration_type(name: &str) -> Option<DurationType> {
    Some(match name {
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
        "zero" => DurationType::Zero,
        "measure" => DurationType::Measure,
        _ => return None,
    })
}

/// Clef (`concertClefType`, `transposingClefType`, `<clef>` d'instrument).
pub fn clef_type(name: &str) -> Option<ClefType> {
    Some(match name {
        "G" => ClefType::G,
        "G8vb" => ClefType::G8Vb,
        "G8va" => ClefType::G8Va,
        "G15mb" => ClefType::G15Mb,
        "G15ma" => ClefType::G15Ma,
        "G1" => ClefType::G1,
        "C1" => ClefType::C1,
        "C2" => ClefType::C2,
        "C3" => ClefType::C3,
        "C4" => ClefType::C4,
        "C5" => ClefType::C5,
        "F" => ClefType::F,
        "F8vb" => ClefType::F8Vb,
        "F8va" => ClefType::F8Va,
        "F15mb" => ClefType::F15Mb,
        "F15ma" => ClefType::F15Ma,
        "F3" => ClefType::FBaritone,
        "F5" => ClefType::FSubbass,
        "PERC" | "Perc" => ClefType::Perc,
        "PERC2" => ClefType::Perc2,
        "TAB" => ClefType::Tab,
        "TAB4" => ClefType::Tab4,
        "TAB_SERIF" => ClefType::TabSerif,
        "TAB4_SERIF" => ClefType::Tab4Serif,
        _ => return None,
    })
}

/// Barre de mesure (`BarLine/subtype`).
pub fn bar_line_type(name: &str) -> Option<BarLineType> {
    Some(match name {
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
    })
}

/// Mode d'armure (`KeySig/mode`).
pub fn key_mode(name: &str) -> Option<KeyMode> {
    Some(match name {
        "major" => KeyMode::Major,
        "minor" => KeyMode::Minor,
        "none" => KeyMode::None,
        "unknown" => KeyMode::Unknown,
        _ => return None,
    })
}

/// Type d'indication de mesure (`TimeSig/subtype`).
pub fn time_sig_type(subtype: i64) -> TimeSigType {
    match subtype {
        1 => TimeSigType::FourFour,
        2 => TimeSigType::AllaBreve,
        3 => TimeSigType::CutBach,
        4 => TimeSigType::CutTriple,
        _ => TimeSigType::Normal,
    }
}

/// Altération affichée (`Accidental/subtype`, nom de symbole SMuFL).
pub fn accidental_type(name: &str) -> Option<AccidentalType> {
    Some(match name {
        "accidentalFlat" => AccidentalType::Flat,
        "accidentalNatural" => AccidentalType::Natural,
        "accidentalSharp" => AccidentalType::Sharp,
        "accidentalDoubleFlat" => AccidentalType::DoubleFlat,
        "accidentalDoubleSharp" => AccidentalType::DoubleSharp,
        "accidentalNaturalFlat" => AccidentalType::NaturalFlat,
        "accidentalNaturalSharp" => AccidentalType::NaturalSharp,
        "accidentalTripleFlat" => AccidentalType::TripleFlat,
        "accidentalTripleSharp" => AccidentalType::TripleSharp,
        _ => return None,
    })
}

/// Mode de ligature (`BeamMode`).
pub fn beam_mode(name: &str, version: u32) -> Option<BeamMode> {
    // Avant 4.1, MuseScore nommait les crochets un cran plus haut
    // (`rw/read400/tread.cpp` lu par `read410` : begin64/begin32 → begin32/begin16).
    let name = if version < 410 {
        match name {
            "begin64" => "begin32",
            "begin32" => "begin16",
            other => other,
        }
    } else {
        name
    };
    Some(match name {
        "auto" => BeamMode::Auto,
        "none" | "no" => BeamMode::None,
        "begin" => BeamMode::Begin,
        "begin16" => BeamMode::Begin16,
        "begin32" => BeamMode::Begin32,
        "mid" | "mid16" | "mid32" | "mid64" => BeamMode::Mid,
        "end" | "end16" | "end32" | "end64" => BeamMode::End,
        "forward hook" => BeamMode::ForwardHook,
        "backward hook" => BeamMode::BackwardHook,
        _ => return None,
    })
}

/// Type d'accord d'agrément, d'après le nom de la balise.
pub fn grace_type(tag: &str) -> Option<GraceType> {
    Some(match tag {
        "appoggiatura" => GraceType::Appoggiatura,
        "acciaccatura" => GraceType::Acciaccatura,
        "grace4" => GraceType::Grace4,
        "grace8after" => GraceType::Grace8After,
        "grace16" => GraceType::Grace16,
        "grace16after" => GraceType::Grace16After,
        "grace32" => GraceType::Grace32,
        "grace32after" => GraceType::Grace32After,
        _ => return None,
    })
}

/// Syllabisation des paroles (`Lyrics/syllabic`).
pub fn syllabic(name: &str) -> LyricsSyllabic {
    match name {
        "begin" => LyricsSyllabic::Begin,
        "middle" => LyricsSyllabic::Middle,
        "end" => LyricsSyllabic::End,
        _ => LyricsSyllabic::Single,
    }
}

/// Tête de note (`Note/head`), nom de groupe de têtes MuseScore.
///
/// Les noms suivent `TConv::fromXml` pour `NoteHeadGroup` ; les groupes non
/// modélisés gardent leur nom d'origine.
pub fn note_head(name: &str) -> Option<NoteHeadGroup> {
    Some(match name {
        "normal" | "auto" => NoteHeadGroup::Normal,
        "cross" => NoteHeadGroup::Cross,
        "plus" => NoteHeadGroup::Plus,
        "xcircle" => NoteHeadGroup::XCircle,
        "withx" => NoteHeadGroup::WithX,
        "triangle-up" => NoteHeadGroup::TriangleUp,
        "triangle-down" => NoteHeadGroup::TriangleDown,
        "slashed1" => NoteHeadGroup::Slashed1,
        "slashed2" => NoteHeadGroup::Slashed2,
        "diamond" => NoteHeadGroup::Diamond,
        "diamond-old" => NoteHeadGroup::DiamondOld,
        "circled" => NoteHeadGroup::Circled,
        "circled-large" => NoteHeadGroup::CircledLarge,
        "large-arrow" => NoteHeadGroup::LargeArrow,
        "altbrevis" => NoteHeadGroup::BrevisAlt,
        "slash" => NoteHeadGroup::Slash,
        "large-diamond" => NoteHeadGroup::LargeDiamond,
        "do" => NoteHeadGroup::Do,
        "re" => NoteHeadGroup::Re,
        "mi" => NoteHeadGroup::Mi,
        "fa" => NoteHeadGroup::Fa,
        "sol" => NoteHeadGroup::Sol,
        "la" => NoteHeadGroup::La,
        "ti" => NoteHeadGroup::Ti,
        "heavy-cross" => NoteHeadGroup::HeavyCross,
        "heavy-cross-hat" => NoteHeadGroup::HeavyCrossHat,
        other => NoteHeadGroup::Other(other.to_owned()),
    })
}

/// Tête de note demandée par `<headType>` (ronde, blanche, noire…).
pub fn head_type(name: &str) -> Option<NoteHeadType> {
    Some(match name {
        "auto" => NoteHeadType::Auto,
        "breve" => NoteHeadType::Breve,
        "whole" => NoteHeadType::Whole,
        "half" => NoteHeadType::Half,
        "quarter" => NoteHeadType::Quarter,
        _ => return None,
    })
}

/// Conversion d'une hauteur MIDI vers un `tpc` naturel.
///
/// Repli quand `<tpc>` manque : `Note::setupAfterRead` fait mieux (il consulte
/// l'armure), mais le corpus fournit toujours les deux champs.
pub fn natural_tpc(pitch: u8) -> i8 {
    match pitch % 12 {
        0 => crate::model::tpc::C,
        2 => crate::model::tpc::D,
        4 => crate::model::tpc::E,
        5 => crate::model::tpc::F,
        7 => crate::model::tpc::G,
        9 => crate::model::tpc::A,
        11 => crate::model::tpc::B,
        // Notes altérées : orthographe diésée par défaut.
        1 => crate::model::tpc::C_SHARP,
        3 => crate::model::tpc::D_SHARP,
        6 => crate::model::tpc::F_SHARP,
        8 => crate::model::tpc::G_SHARP,
        _ => crate::model::tpc::A_SHARP,
    }
}
