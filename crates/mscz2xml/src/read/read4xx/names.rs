//! Conversions des noms MuseScore vers les énumérations du modèle.
//!
//! Les tables reproduisent `TConv::fromXml` (`engraving/types/typesconv.cpp`,
//! généré) pour les valeurs rencontrées par le noyau musical. Les noms inconnus
//! renvoient `None` : le lecteur les ignore alors silencieusement.

use crate::model::{
    AccidentalType, ArticulationAnchor, ArticulationType, BarLineType, BeamMode, ClefType,
    DurationType, GraceType, KeyMode, LyricsSyllabic, NoteHeadType, TimeSigType,
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

/// Articulation (`Articulation/subtype`), ancre comprise dans le nom.
///
/// MuseScore encode l'ancre dans le suffixe `Above` / `Below`. Les articulations
/// composées (`articTenutoAccent`…) n'ont pas de variante dans le modèle : elles
/// sont rapprochées de leur composant principal avec un `TODO(parité)` chez
/// l'appelant.
pub fn articulation(name: &str) -> Option<(ArticulationType, ArticulationAnchor)> {
    let (base, anchor) = if let Some(base) = name.strip_suffix("Below") {
        (base, ArticulationAnchor::Down)
    } else if let Some(base) = name.strip_suffix("Above") {
        (base, ArticulationAnchor::Up)
    } else {
        (name, ArticulationAnchor::Auto)
    };

    let kind = match base {
        "articStaccato" => ArticulationType::Staccato,
        "articStaccatissimo" => ArticulationType::Staccatissimo,
        "articAccent" => ArticulationType::Accent,
        "articMarcato" => ArticulationType::Marcato,
        "articTenuto" => ArticulationType::Tenuto,
        "articTenutoAccent" => ArticulationType::Portato,
        "articTenutoStaccato" => ArticulationType::Portato,
        "fermata" | "fermataAbove" | "fermataBelow" => ArticulationType::Fermata,
        "fermataShort" => ArticulationType::FermataShort,
        "fermataLong" => ArticulationType::FermataLong,
        "fermataVeryShort" => ArticulationType::FermataVeryShort,
        "fermataVeryLong" => ArticulationType::FermataVeryLong,
        "breathMarkComma" | "breathMark" => ArticulationType::BreathMark,
        "caesura" | "caesuraCurved" | "caesuraShort" => ArticulationType::Caesura,
        "stringsUpBow" | "articUpBow" => ArticulationType::UpBow,
        "stringsDownBow" | "articDownBow" => ArticulationType::DownBow,
        "stringsHarmonic" | "articHarmonic" => ArticulationType::Harmonic,
        "articOpen" => ArticulationType::Open,
        "articStopped" => ArticulationType::Stopped,
        "articSnapPizzicato" => ArticulationType::SnapPizzicato,
        _ => return None,
    };
    Some((kind, anchor))
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
        "none" => BeamMode::None,
        "begin" => BeamMode::Begin,
        "mid" => BeamMode::Mid,
        "end" => BeamMode::End,
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
/// Le modèle ne connaît que quelques formes ; les autres têtes sont
/// rapprochées de la variante la plus proche et signalées par un
/// `TODO(parité)` chez l'appelant.
pub fn note_head(name: &str) -> Option<NoteHeadType> {
    Some(match name {
        "normal" | "auto" => NoteHeadType::Auto,
        "whole" => NoteHeadType::Whole,
        "half" => NoteHeadType::Half,
        "quarter" => NoteHeadType::Quarter,
        "breve" | "altbrevis" => NoteHeadType::Breve,
        "cross" | "heavy-cross" | "heavy-cross-hat" => NoteHeadType::Cross,
        "xcircle" | "withx" => NoteHeadType::X,
        "diamond" | "diamond-old" | "large-diamond" => NoteHeadType::Diamond,
        "triangle-up" | "triangle-down" | "large-arrow" => NoteHeadType::Triangle,
        "slash" | "slashed1" | "slashed2" => NoteHeadType::Slash,
        "square" => NoteHeadType::Square,
        _ => return None,
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
