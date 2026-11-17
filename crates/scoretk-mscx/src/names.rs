// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/types/types.h`, de `engraving/types/typesconv.cpp` et
// des tables de noms de `engraving/rw/` de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Conversions du modèle vers les noms MuseScore.
//!
//! Miroir des tables de `mscz2xml::read` : les valeurs écrites sont exactement
//! celles que `TConv::toXml` produit dans MuseScore, afin qu'un fichier relu
//! redonne le même modèle. Les fonctions sont totales : une valeur inconnue
//! garde son nom MuseScore quand le modèle le porte (`Other`), sinon elle
//! retombe sur la valeur par défaut du lecteur.

use mscz2xml::model::{
    AccidentalType, ArpeggioType, ArticulationAnchor, ArticulationType, BarLineType, BeamMode,
    BreathType, ChordLineType, ClefType, DurationType, FermataType, FiguredBassContLine,
    FiguredBassModifier, GlissandoType, GraceType, HairpinType, HookType, KeyMode, LineStyle,
    LyricsSyllabic, MarkerType, NoteHeadGroup, NoteHeadType, OrnamentType, OttavaType,
    TextLineStyle, TimeSigType, TremoloBarType, TrillType,
};

/// Figure de durée (`durationType`).
pub fn duration_type(value: DurationType) -> &'static str {
    match value {
        DurationType::Long => "long",
        DurationType::Breve => "breve",
        DurationType::Whole => "whole",
        DurationType::Half => "half",
        DurationType::Quarter => "quarter",
        DurationType::Eighth => "eighth",
        DurationType::Sixteenth => "16th",
        DurationType::ThirtySecond => "32nd",
        DurationType::SixtyFourth => "64th",
        DurationType::OneHundredTwentyEighth => "128th",
        DurationType::TwoHundredFiftySixth => "256th",
        DurationType::FiveHundredTwelfth => "512th",
        DurationType::OneThousandTwentyFourth => "1024th",
        DurationType::Zero => "zero",
        DurationType::Measure => "measure",
        DurationType::Invalid => "invalid",
    }
}

/// Clef (`concertClefType`, `transposingClefType`).
pub fn clef_type(value: ClefType) -> &'static str {
    match value {
        ClefType::G => "G",
        ClefType::G8Vb => "G8vb",
        ClefType::G8Va => "G8va",
        ClefType::G15Mb => "G15mb",
        ClefType::G15Ma => "G15ma",
        ClefType::G1 => "G1",
        ClefType::C1 => "C1",
        ClefType::C2 => "C2",
        ClefType::C3 => "C3",
        ClefType::C4 => "C4",
        ClefType::C5 => "C5",
        ClefType::F => "F",
        ClefType::F8Vb => "F8vb",
        ClefType::F8Va => "F8va",
        ClefType::F15Mb => "F15mb",
        ClefType::F15Ma => "F15ma",
        ClefType::FBaritone => "F3",
        ClefType::FSubbass => "F5",
        ClefType::Perc => "PERC",
        ClefType::Perc2 => "PERC2",
        ClefType::Tab => "TAB",
        ClefType::Tab4 => "TAB4",
        ClefType::TabSerif => "TAB_SERIF",
        ClefType::Tab4Serif => "TAB4_SERIF",
        ClefType::Invalid => "G",
    }
}

/// Barre de mesure (`BarLine/subtype`).
pub fn bar_line_type(value: BarLineType) -> &'static str {
    match value {
        BarLineType::Normal => "normal",
        BarLineType::Double => "double",
        BarLineType::StartRepeat => "start-repeat",
        BarLineType::EndRepeat => "end-repeat",
        BarLineType::EndStartRepeat => "end-start-repeat",
        BarLineType::Broken => "broken",
        BarLineType::End => "end",
        BarLineType::Dotted => "dotted",
        BarLineType::DoubleDotted => "double-dotted",
        BarLineType::Dashed => "dashed",
        BarLineType::Short => "short",
        BarLineType::DoubleShort => "double-short",
        BarLineType::Tick => "tick",
    }
}

/// Mode d'armure (`KeySig/mode`).
pub fn key_mode(value: KeyMode) -> &'static str {
    match value {
        KeyMode::Major => "major",
        KeyMode::Minor => "minor",
        KeyMode::None => "none",
        KeyMode::Unknown => "unknown",
    }
}

/// Numéro de sous-type d'une indication de mesure.
pub fn time_sig_subtype(value: TimeSigType) -> i32 {
    match value {
        TimeSigType::Normal => 0,
        TimeSigType::FourFour => 1,
        TimeSigType::AllaBreve => 2,
        TimeSigType::CutBach => 3,
        TimeSigType::CutTriple => 4,
    }
}

/// Nom SMuFL d'une altération (`Accidental/subtype`).
pub fn accidental_type(value: AccidentalType) -> Option<String> {
    Some(
        match value {
            AccidentalType::None => return None,
            AccidentalType::Flat => "accidentalFlat",
            AccidentalType::Natural => "accidentalNatural",
            AccidentalType::Sharp => "accidentalSharp",
            AccidentalType::DoubleFlat => "accidentalDoubleFlat",
            AccidentalType::DoubleSharp => "accidentalDoubleSharp",
            AccidentalType::NaturalFlat => "accidentalNaturalFlat",
            AccidentalType::NaturalSharp => "accidentalNaturalSharp",
            AccidentalType::TripleFlat => "accidentalTripleFlat",
            AccidentalType::TripleSharp => "accidentalTripleSharp",
            AccidentalType::Other(_) => return None,
        }
        .to_owned(),
    )
}

/// Nom MuseScore d'un groupe de têtes (`Note/head`).
pub fn note_head(value: &NoteHeadGroup) -> Option<String> {
    Some(
        match value {
            NoteHeadGroup::Normal => "normal",
            NoteHeadGroup::Cross => "cross",
            NoteHeadGroup::Plus => "plus",
            NoteHeadGroup::XCircle => "xcircle",
            NoteHeadGroup::WithX => "withx",
            NoteHeadGroup::TriangleUp => "triangle-up",
            NoteHeadGroup::TriangleDown => "triangle-down",
            NoteHeadGroup::Slashed1 => "slashed1",
            NoteHeadGroup::Slashed2 => "slashed2",
            NoteHeadGroup::Diamond => "diamond",
            NoteHeadGroup::DiamondOld => "diamond-old",
            NoteHeadGroup::Circled => "circled",
            NoteHeadGroup::CircledLarge => "circled-large",
            NoteHeadGroup::LargeArrow => "large-arrow",
            NoteHeadGroup::BrevisAlt => "altbrevis",
            NoteHeadGroup::Slash => "slash",
            NoteHeadGroup::LargeDiamond => "large-diamond",
            NoteHeadGroup::Do => "do",
            NoteHeadGroup::Re => "re",
            NoteHeadGroup::Mi => "mi",
            NoteHeadGroup::Fa => "fa",
            NoteHeadGroup::Sol => "sol",
            NoteHeadGroup::La => "la",
            NoteHeadGroup::Ti => "ti",
            NoteHeadGroup::HeavyCross => "heavy-cross",
            NoteHeadGroup::HeavyCrossHat => "heavy-cross-hat",
            NoteHeadGroup::Other(name) => name.as_str(),
        }
        .to_owned(),
    )
}

/// Nom MuseScore d'un type de tête (`Note/headType`).
pub fn head_type(value: NoteHeadType) -> Option<&'static str> {
    Some(match value {
        NoteHeadType::Auto => "auto",
        NoteHeadType::Breve => "breve",
        NoteHeadType::Whole => "whole",
        NoteHeadType::Half => "half",
        NoteHeadType::Quarter => "quarter",
        NoteHeadType::Cross
        | NoteHeadType::Diamond
        | NoteHeadType::Triangle
        | NoteHeadType::Slash
        | NoteHeadType::Square
        | NoteHeadType::X
        | NoteHeadType::Other(_) => return None,
    })
}

/// Nom MuseScore d'un mode de ligature (`BeamMode`).
pub fn beam_mode(value: BeamMode) -> &'static str {
    match value {
        BeamMode::Auto => "auto",
        BeamMode::None => "none",
        BeamMode::Begin => "begin",
        BeamMode::Begin16 => "begin16",
        BeamMode::Begin32 => "begin32",
        BeamMode::Mid => "mid",
        BeamMode::End => "end",
        BeamMode::ForwardHook => "forward hook",
        BeamMode::BackwardHook => "backward hook",
    }
}

/// Nom de balise d'un accord d'agrément (`<appoggiatura/>`, …).
pub fn grace_tag(value: GraceType) -> &'static str {
    match value {
        GraceType::Appoggiatura => "appoggiatura",
        GraceType::Acciaccatura => "acciaccatura",
        GraceType::Grace4 => "grace4",
        GraceType::Grace8After => "grace8after",
        GraceType::Grace16 => "grace16",
        GraceType::Grace16After => "grace16after",
        GraceType::Grace32 => "grace32",
        GraceType::Grace32After => "grace32after",
    }
}

/// Syllabisation (`Lyrics/syllabic`) ; `None` pour la valeur par défaut.
pub fn syllabic(value: LyricsSyllabic) -> Option<&'static str> {
    match value {
        LyricsSyllabic::Single => None,
        LyricsSyllabic::Begin => Some("begin"),
        LyricsSyllabic::Middle => Some("middle"),
        LyricsSyllabic::End => Some("end"),
    }
}

/// Nom SMuFL d'une articulation, avec le suffixe d'ancre qui fixe son sens.
///
/// MuseScore encode le sens du glyphe dans le nom (`articAccentBelow`) : le
/// lecteur en tire `Articulation::up`, distinct de l'ancre explicite.
pub fn articulation(value: &ArticulationType, up: bool) -> String {
    let base = match value {
        ArticulationType::Staccato => "articStaccato",
        ArticulationType::Staccatissimo => "articStaccatissimo",
        ArticulationType::Accent => "articAccent",
        ArticulationType::Marcato => "articMarcato",
        ArticulationType::Tenuto => "articTenuto",
        ArticulationType::Portato => "articPortato",
        ArticulationType::SoftAccent => "articSoftAccent",
        ArticulationType::SoftAccentStaccato => "articSoftAccentStaccato",
        ArticulationType::SoftAccentTenuto => "articSoftAccentTenuto",
        ArticulationType::SoftAccentTenutoStaccato => "articSoftAccentTenutoStaccato",
        ArticulationType::Stress => "articStress",
        ArticulationType::Unstress => "articUnstress",
        ArticulationType::Spiccato => "articStaccatissimoStroke",
        ArticulationType::Fermata => "fermata",
        ArticulationType::FermataShort => "fermataShort",
        ArticulationType::FermataLong => "fermataLong",
        ArticulationType::FermataVeryShort => "fermataVeryShort",
        ArticulationType::FermataVeryLong => "fermataVeryLong",
        ArticulationType::BreathMark => "breathMarkComma",
        ArticulationType::Caesura => "caesura",
        ArticulationType::UpBow => "stringsUpBow",
        ArticulationType::DownBow => "stringsDownBow",
        ArticulationType::Harmonic => "stringsHarmonic",
        ArticulationType::Open => "brassMuteOpen",
        ArticulationType::Stopped => "brassMuteClosed",
        ArticulationType::SnapPizzicato => "pluckedSnapPizzicato",
        ArticulationType::Other(name) => return name.clone(),
    };
    if up {
        base.to_owned()
    } else {
        format!("{base}Below")
    }
}

/// Ancre explicite d'une articulation (`<anchor>` : 0 auto, 1 haut, 2 bas).
pub fn articulation_anchor(value: ArticulationAnchor) -> i32 {
    match value {
        ArticulationAnchor::Auto => 0,
        ArticulationAnchor::Up => 1,
        ArticulationAnchor::Down => 2,
    }
}

/// Nom SMuFL d'un point d'orgue (`Fermata/subtype`).
pub fn fermata_type(value: FermataType) -> &'static str {
    match value {
        FermataType::Normal => "fermata",
        FermataType::Short => "fermataShort",
        FermataType::Long => "fermataLong",
        FermataType::VeryShort => "fermataVeryShort",
        FermataType::VeryLong => "fermataVeryLong",
        FermataType::LongHenze => "fermataLongHenze",
        FermataType::ShortHenze => "fermataShortHenze",
        FermataType::Curlew => "curlewSign",
    }
}

/// Nom SMuFL d'un ornement (`Ornament/subtype`).
pub fn ornament_type(value: &OrnamentType) -> String {
    match value {
        OrnamentType::TrillMark => "ornamentTrill",
        OrnamentType::Turn => "ornamentTurn",
        OrnamentType::InvertedTurn => "ornamentTurnInverted",
        OrnamentType::TurnSlash => "ornamentTurnSlash",
        OrnamentType::VerticalTurn => "ornamentTurnUp",
        OrnamentType::InvertedVerticalTurn => "ornamentTurnUpS",
        OrnamentType::Mordent => "ornamentMordent",
        OrnamentType::InvertedMordent => "ornamentShortTrill",
        OrnamentType::InvertedMordentLong => "ornamentTremblement",
        OrnamentType::MordentLong => "ornamentPrallMordent",
        OrnamentType::InvertedMordentLongApproachBelow => "ornamentUpPrall",
        OrnamentType::InvertedMordentLongApproachAbove => "ornamentLinePrall",
        OrnamentType::MordentLongApproachBelow => "ornamentUpMordent",
        OrnamentType::MordentLongApproachAbove => "ornamentDownMordent",
        OrnamentType::InvertedMordentLongDepartureBelow => "ornamentPrallDown",
        OrnamentType::InvertedMordentLongDepartureAbove => "ornamentPrallUp",
        OrnamentType::Haydn => "ornamentHaydn",
        OrnamentType::Schleifer => "ornamentPrecompSlide",
        OrnamentType::Other(name) => return name.clone(),
    }
    .to_owned()
}

/// Sub-type d'un trille (`Trill/subtype`).
pub fn trill_type(value: TrillType) -> &'static str {
    match value {
        TrillType::TrillLine => "trill",
        TrillType::PureLine => "pure",
        TrillType::UpperPrallLine => "upprall",
        TrillType::PrallPrallLine => "prallprall",
        TrillType::DownPrallLine => "downprall",
    }
}

/// Figure d'un trémolo (`r8`/`c8`), à partir du nombre de barres.
pub fn tremolo_figure(count: u8) -> Option<&'static str> {
    Some(match count {
        1 => "8",
        2 => "16",
        3 => "32",
        4 => "64",
        5 => "128",
        6 => "256",
        _ => return None,
    })
}

/// Valeur numérique d'un arpège (`Arpeggio/subtype`).
pub fn arpeggio_type(value: ArpeggioType) -> &'static str {
    match value {
        ArpeggioType::Normal => "0",
        ArpeggioType::Up => "1",
        ArpeggioType::Down => "2",
        ArpeggioType::Bracket => "3",
    }
}

/// Valeur numérique d'une ligne d'accord (`ChordLine/subtype`).
pub fn chord_line_type(value: ChordLineType) -> &'static str {
    match value {
        ChordLineType::Fall => "1",
        ChordLineType::Doit => "2",
        ChordLineType::Plop => "3",
        ChordLineType::Scoop => "4",
    }
}

/// Nom d'un style de trait (`lineStyle`).
pub fn line_style(value: LineStyle) -> &'static str {
    match value {
        LineStyle::Solid => "solid",
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
    }
}

/// Sous-type d'un glissando (`0` droit, `1` ondulé).
pub fn glissando_type(value: GlissandoType) -> &'static str {
    match value {
        GlissandoType::Straight(_) => "0",
        GlissandoType::Wavy => "1",
    }
}

/// Libellé d'un marqueur de reprise (`<label>`).
pub fn marker_label(value: MarkerType) -> &'static str {
    match value {
        MarkerType::Segno => "segno",
        MarkerType::VarSegno => "varsegno",
        MarkerType::Coda => "codab",
        MarkerType::VarCoda => "varcoda",
        MarkerType::Codetta => "codetta",
        MarkerType::Fine => "fine",
        MarkerType::ToCoda => "coda",
        MarkerType::ToCodaSym => "codasym",
        MarkerType::DaCoda => "dacoda",
        MarkerType::DaDoubleCoda => "dadblcoda",
        MarkerType::User => "",
    }
}

/// Symbole d'une respiration (`Breath/symbol`), `None` pour la virgule.
pub fn breath_symbol(value: BreathType) -> Option<&'static str> {
    match value {
        BreathType::Comma => None,
        BreathType::Caesura => Some("caesura"),
        BreathType::CaesuraCurved => Some("caesuraCurved"),
        BreathType::CaesuraShort => Some("caesuraShort"),
        BreathType::CaesuraThick => Some("caesuraThick"),
        BreathType::CaesuraSingle => Some("caesuraSingleStroke"),
        BreathType::Tick => Some("breathTick"),
        BreathType::Upbow => Some("stringsUpBow"),
    }
}

/// Sous-type d'un soufflet (`<HairPin><subtype>`).
///
/// `0` crescendo dessiné, `1` diminuendo dessiné, `2` crescendo en trait,
/// `3` diminuendo en trait (`TRead::read(Hairpin*)`).
pub fn hairpin_subtype(kind: HairpinType, line: bool) -> i32 {
    match (kind, line) {
        (HairpinType::Crescendo, false) => 0,
        (HairpinType::Diminuendo, false) => 1,
        (HairpinType::Crescendo, true) => 2,
        (HairpinType::Diminuendo, true) => 3,
    }
}

/// Sous-type d'une ottava (`<Ottava><subtype>`).
pub fn ottava_subtype(value: OttavaType) -> &'static str {
    match value {
        OttavaType::OctaveUp => "8va",
        OttavaType::OctaveDown => "8vb",
        OttavaType::FifteenthUp => "15ma",
        OttavaType::FifteenthDown => "15mb",
    }
}

/// Valeur numérique d'un crochet (`beginHookType`, `endHookType`).
pub fn hook_type(value: HookType) -> i32 {
    match value {
        HookType::None => 0,
        HookType::Hook90 => 1,
        HookType::Hook90T => 2,
        HookType::Arrow => 3,
        HookType::ArrowFilled => 4,
    }
}

/// Valeur numérique d'un style de trait de ligne de texte (`lineStyle`).
pub fn text_line_style(value: TextLineStyle) -> i32 {
    match value {
        TextLineStyle::Solid => 0,
        TextLineStyle::Dashed => 1,
        TextLineStyle::Dotted => 2,
    }
}

/// Valeur numérique d'un modificateur de chiffrage (`prefix`/`suffix`).
pub fn figured_bass_modifier(value: FiguredBassModifier) -> i32 {
    match value {
        FiguredBassModifier::None => 0,
        FiguredBassModifier::DoubleFlat => 1,
        FiguredBassModifier::Flat => 2,
        FiguredBassModifier::Natural => 3,
        FiguredBassModifier::Sharp => 4,
        FiguredBassModifier::DoubleSharp => 5,
        FiguredBassModifier::Cross => 6,
        FiguredBassModifier::Backslash => 7,
        FiguredBassModifier::Slash => 8,
    }
}

/// Valeur numérique d'un trait de prolongation de chiffrage.
pub fn figured_bass_cont_line(value: FiguredBassContLine) -> i32 {
    match value {
        FiguredBassContLine::None => 0,
        FiguredBassContLine::Simple => 1,
        FiguredBassContLine::Extended => 2,
    }
}

/// Valeur numérique d'un type de barre de vibrato (`<TremoloBar><type>`).
pub fn tremolo_bar_type(value: TremoloBarType) -> i32 {
    match value {
        TremoloBarType::Dip => 0,
        TremoloBarType::Dive => 1,
        TremoloBarType::ReleaseUp => 2,
        TremoloBarType::InvertedDip => 3,
        TremoloBarType::Return => 4,
        TremoloBarType::ReleaseDown => 5,
        TremoloBarType::Custom => 6,
    }
}
