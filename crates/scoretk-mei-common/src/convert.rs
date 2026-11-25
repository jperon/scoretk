// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `importexport/mei/internal/meiconverter.cpp` de MuseScore Studio
// 4.7.4, Copyright (C) MuseScore Limited and others, modifié (réécrit en Rust)
// en 2026. Les noms des fonctions reprennent ceux du C++ (`durFromMEI`,
// `tstampToFraction`, `pitchFromMEI`, …).
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

//! Attribute/element conversions shared by the MEI reader and writer.
//!
//! MEI 5.1: `@dur` is a denominator (`4` = quarter), middle C is `c`/`3`,
//! `@tstamp` is a one-based index in beats of the current meter.

use scoretk_core::model::{
    AccidentalType, ArticulationType, BarLineType, BeamMode, ClefType, CreditStyle, Duration,
    DurationType, Fraction, GraceType, JumpType, Key, KeySig, MarkerType, Measure, NoteHeadGroup,
    Placement, StemDirection, TimeSig, TimeSigType,
};

use crate::ids::MEASURE_REPEAT_TYPE;

/// `Convert::hasTypeValue` (`meiconverter.cpp:3731`): space-separated tokens.
pub fn has_type_value(type_str: &str, value: &str) -> bool {
    type_str.split(' ').any(|token| token == value)
}

/// `Convert::getTypeValueWithPrefix` (`meiconverter.cpp:3745`).
pub fn get_type_value_with_prefix<'a>(type_str: &'a str, prefix: &'a str) -> Option<&'a str> {
    get_type_values_with_prefix(type_str, prefix).next()
}

/// `Convert::getTypeValuesWithPrefix` (`meiconverter.cpp:3766`).
pub fn get_type_values_with_prefix<'a>(
    type_str: &'a str,
    prefix: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    type_str
        .split(' ')
        .filter_map(move |token| token.strip_prefix(prefix).filter(|rest| !rest.is_empty()))
}

/// `Convert::durFromMEI` (`meiconverter.cpp:2916`): `@dur` is a denominator.
pub fn dur_from_mei(value: &str) -> Option<DurationType> {
    match value {
        "long" => Some(DurationType::Long),
        "breve" => Some(DurationType::Breve),
        "1" => Some(DurationType::Whole),
        "2" => Some(DurationType::Half),
        "4" => Some(DurationType::Quarter),
        "8" => Some(DurationType::Eighth),
        "16" => Some(DurationType::Sixteenth),
        "32" => Some(DurationType::ThirtySecond),
        "64" => Some(DurationType::SixtyFourth),
        "128" => Some(DurationType::OneHundredTwentyEighth),
        "256" => Some(DurationType::TwoHundredFiftySixth),
        "512" => Some(DurationType::FiveHundredTwelfth),
        "1024" => Some(DurationType::OneThousandTwentyFourth),
        _ => None,
    }
}

/// Exact `(kind, dots)` whose written ticks equal `ticks`, if representable.
///
/// Used for `@dur.ges`/`@dots.ges`, the MEI 5.1 gestural duration attributes
/// (`att.gestural.log`, `data.DURATION.GESTURAL`): MuseScore stores a written
/// `durationType` and an independent `actualTicks()` for the same chord.
pub fn duration_from_ticks(ticks: Fraction) -> Option<(DurationType, u8)> {
    for kind in [
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
        DurationType::Breve,
        DurationType::Long,
    ] {
        for dots in 0..=3 {
            if Duration::with_dots(kind, dots).ticks() == ticks {
                return Some((kind, dots));
            }
        }
    }
    None
}

/// `Convert::durToMEI` (`meiconverter.cpp:2939`).
pub fn dur_to_mei(duration: DurationType) -> &'static str {
    match duration {
        DurationType::Long => "long",
        DurationType::Breve => "breve",
        DurationType::Whole => "1",
        DurationType::Half => "2",
        DurationType::Quarter => "4",
        DurationType::Eighth => "8",
        DurationType::Sixteenth => "16",
        DurationType::ThirtySecond => "32",
        DurationType::SixtyFourth => "64",
        DurationType::OneHundredTwentyEighth => "128",
        DurationType::TwoHundredFiftySixth => "256",
        DurationType::FiveHundredTwelfth => "512",
        DurationType::OneThousandTwentyFourth => "1024",
        _ => "4",
    }
}

/// `Convert::accidFromMEI` (`meiconverter.cpp:160`): written accidental.
pub fn accid_from_mei(value: &str) -> Option<AccidentalType> {
    Some(match value {
        "n" => AccidentalType::Natural,
        "f" => AccidentalType::Flat,
        "ff" => AccidentalType::DoubleFlat,
        "tf" => AccidentalType::TripleFlat,
        "s" => AccidentalType::Sharp,
        "x" => AccidentalType::DoubleSharp,
        "ts" => AccidentalType::TripleSharp,
        "nf" => AccidentalType::NaturalFlat,
        "ns" => AccidentalType::NaturalSharp,
        "ss" => AccidentalType::DoubleSharp,
        _ => return None,
    })
}

/// `Convert::accidToMEI` (`meiconverter.cpp:181`).
pub fn accid_to_mei(kind: AccidentalType) -> Option<&'static str> {
    Some(match kind {
        AccidentalType::None => return None,
        AccidentalType::Natural => "n",
        AccidentalType::Flat => "f",
        AccidentalType::DoubleFlat => "ff",
        AccidentalType::TripleFlat => "tf",
        AccidentalType::Sharp => "s",
        AccidentalType::DoubleSharp => "x",
        AccidentalType::TripleSharp => "ts",
        AccidentalType::NaturalFlat => "nf",
        AccidentalType::NaturalSharp => "ns",
        AccidentalType::Other(_) => "n",
    })
}

/// Sounding alteration of a written/gestural accidental, in semitones.
///
/// Port of `Convert::accidGesFromMEI` (`meiconverter.cpp:200`) combined with
/// `AccidentalWrittenToGestural`: written and gestural values share their
/// semitone meaning except for `ss`/`x` and `ff`, which MEI spells differently.
pub fn alter_from_accid(value: &str) -> i32 {
    match value {
        "f" => -1,
        "ff" => -2,
        "tf" => -3,
        "s" => 1,
        "ss" | "x" => 2,
        "ts" => 3,
        "nf" | "ns" | "n" => 0,
        _ => 0,
    }
}

/// `Convert::barlineFromMEI` (`meiconverter.cpp:574`).
pub fn barline_from_mei(value: &str) -> BarLineType {
    match value {
        "single" => BarLineType::Normal,
        "dbl" | "dbldashed" | "dbldotted" | "dblsegno" => BarLineType::Double,
        "rptstart" => BarLineType::StartRepeat,
        "rptend" => BarLineType::EndRepeat,
        "rptboth" => BarLineType::EndStartRepeat,
        "dashed" => BarLineType::Broken,
        "end" => BarLineType::End,
        "dotted" => BarLineType::Dotted,
        "heavy" => BarLineType::Short,
        "dblheavy" => BarLineType::DoubleShort,
        _ => BarLineType::Normal,
    }
}

/// `Convert::barlineToMEI` (`meiconverter.cpp:604`).
pub fn barline_to_mei(barline: BarLineType) -> &'static str {
    match barline {
        BarLineType::Normal => "single",
        BarLineType::Double | BarLineType::DoubleDotted => "dbl",
        BarLineType::StartRepeat => "rptstart",
        BarLineType::EndRepeat => "rptend",
        BarLineType::EndStartRepeat => "rptboth",
        BarLineType::Broken => "dashed",
        BarLineType::End => "end",
        BarLineType::Dotted => "dotted",
        BarLineType::Short => "heavy",
        BarLineType::DoubleShort => "dblheavy",
        BarLineType::Tick | BarLineType::Dashed => "dotted",
    }
}

/// `Convert::stemFromMEI` (`meiconverter.cpp:3150`).
///
/// MEI's `@stem.dir` is `up`/`down`; anything else leaves the stem automatic.
pub fn stem_from_mei(value: &str) -> Option<StemDirection> {
    match value {
        "up" => Some(StemDirection::Up),
        "down" => Some(StemDirection::Down),
        _ => None,
    }
}

/// `Convert::stemToMEI` (`meiconverter.cpp:3172`).
///
/// A chord with an automatic stem has no `@stem.dir`, as libmei omits
/// `STEMDIRECTION_NONE`.
pub fn stem_to_mei(direction: StemDirection) -> Option<&'static str> {
    match direction {
        StemDirection::Up => Some("up"),
        StemDirection::Down => Some("down"),
        StemDirection::Auto => None,
    }
}

/// MEI measurement in inches (`att.pages`, `data.MEASUREMENTUNSIGNED`).
///
/// The schema allows a unit suffix; a bare value is in virtual units, so a
/// real-world value always carries one here.
pub fn inches_to_mei(value: f64) -> String {
    format!("{value:.5}in")
}

/// Millimetres as an MEI measurement (`@vu.height`).
pub fn mm_to_mei(value: f64) -> String {
    format!("{value:.5}mm")
}

/// Font size in printer's points (`data.FONTSIZENUMERIC`).
pub fn points_to_mei(value: f64) -> String {
    format!("{value:.2}pt")
}

/// Exact name of a MuseScore grace type, appended to `ids::GRACE_TYPE`.
pub fn grace_type_name(grace: GraceType) -> &'static str {
    match grace {
        GraceType::Acciaccatura => "Acciaccatura",
        GraceType::Appoggiatura => "Appoggiatura",
        GraceType::Grace4 => "Grace4",
        GraceType::Grace8After => "Grace8After",
        GraceType::Grace16 => "Grace16",
        GraceType::Grace16After => "Grace16After",
        GraceType::Grace32 => "Grace32",
        GraceType::Grace32After => "Grace32After",
    }
}

/// Inverse of [`grace_type_name`].
pub fn grace_type_from_name(name: &str) -> Option<GraceType> {
    Some(match name {
        "Acciaccatura" => GraceType::Acciaccatura,
        "Appoggiatura" => GraceType::Appoggiatura,
        "Grace4" => GraceType::Grace4,
        "Grace8After" => GraceType::Grace8After,
        "Grace16" => GraceType::Grace16,
        "Grace16After" => GraceType::Grace16After,
        "Grace32" => GraceType::Grace32,
        "Grace32After" => GraceType::Grace32After,
        _ => return None,
    })
}

/// Standard MEI `@grace` value: `acc` steals from the following note,
/// `unacc` (an "after" grace) from the previous one (`data.GRACE`).
pub fn grace_standard(grace: GraceType) -> &'static str {
    match grace {
        GraceType::Grace8After | GraceType::Grace16After | GraceType::Grace32After => "unacc",
        _ => "acc",
    }
}

/// Inverse of [`inches_to_mei`], also accepting the other schema units.
pub fn measurement_to_inches(value: &str) -> Option<f64> {
    let (number, unit) = value.split_at(value.find(|c: char| !c.is_ascii_digit() && c != '.')?);
    let number = number.parse::<f64>().ok()?;
    let factor = match unit {
        "in" => 1.0,
        "mm" => 1.0 / 25.4,
        "cm" => 1.0 / 2.54,
        "pt" => 1.0 / 72.0,
        "pc" => 1.0 / 6.0,
        _ => return None,
    };
    Some(number * factor)
}

/// Millimetres from an MEI measurement (`@vu.height`).
pub fn measurement_to_mm(value: &str) -> Option<f64> {
    measurement_to_inches(value).map(|inches| inches * 25.4)
}

/// Inverse of [`points_to_mei`].
pub fn font_size_to_points(value: &str) -> Option<f64> {
    if let Some(size) = value.strip_suffix("pt") {
        return size.parse::<f64>().ok();
    }
    None
}

/// `Convert::textToMEI` (`meiconverter.cpp:3409`).
///
/// Returns the cell index (top-left → bottom-right, row major), the `@type`
/// written on the inner `<rend>`, and its `@fontsize` term when the style has
/// one. MuseScore places the texts by style, not by their engraved position.
pub fn text_to_mei(style: &CreditStyle) -> (u8, &'static str, Option<&'static str>) {
    match style {
        CreditStyle::Title => (1, "title", Some("x-large")),
        CreditStyle::Subtitle => (1, "subtitle", Some("large")),
        CreditStyle::Composer => (8, "composer", None),
        CreditStyle::Lyricist => (6, "poet", None),
        CreditStyle::Other(name) if name.eq_ignore_ascii_case("instrument_excerpt") => {
            (0, "instrument_excerpt", None)
        }
        CreditStyle::Other(_) => (4, "frame", None),
    }
}

/// Cell of `Convert::textToMEI` → the `@valign`/`@halign` of the outer `<rend>`
/// (`meiexporter.cpp:417-433`).
pub fn cell_to_align(cell: u8) -> (&'static str, &'static str) {
    let valign = if cell < 3 {
        "top"
    } else if cell < 6 {
        "middle"
    } else {
        "bottom"
    };
    let halign = match cell % 3 {
        0 => "left",
        1 => "center",
        _ => "right",
    };
    (valign, halign)
}

/// `Convert::articToMEI` (`meiconverter.cpp:447`), restricted to the values
/// the importer maps back to a distinct pivot articulation.
pub fn artic_to_mei(kind: &ArticulationType) -> Option<&'static str> {
    match kind {
        ArticulationType::Staccato => Some("stacc"),
        ArticulationType::Staccatissimo | ArticulationType::Spiccato => Some("stacciss"),
        ArticulationType::Accent => Some("acc"),
        ArticulationType::Marcato => Some("marc"),
        ArticulationType::Tenuto => Some("ten"),
        ArticulationType::Portato => Some("port"),
        ArticulationType::Stress => Some("stress"),
        ArticulationType::Unstress => Some("unstress"),
        ArticulationType::UpBow => Some("upbow"),
        ArticulationType::DownBow => Some("dnbow"),
        ArticulationType::Harmonic => Some("harm"),
        ArticulationType::Open => Some("open"),
        ArticulationType::Stopped => Some("stop"),
        ArticulationType::SnapPizzicato => Some("snap"),
        _ => None,
    }
}

/// Suffix of `Convert::markerToMEI`'s `@type` (`mscore-marker-*`).
pub fn marker_type_to_mei(kind: MarkerType) -> &'static str {
    match kind {
        MarkerType::Segno => "segno",
        MarkerType::VarSegno => "varsegno",
        MarkerType::Coda => "codab",
        MarkerType::VarCoda => "varcoda",
        MarkerType::Codetta => "codetta",
        MarkerType::Fine => "fine",
        MarkerType::ToCoda => "to-coda",
        MarkerType::ToCodaSym => "codasym",
        MarkerType::DaCoda => "dacoda",
        MarkerType::DaDoubleCoda => "dadblcoda",
        MarkerType::User => "user",
    }
}

/// Inverse of [`marker_type_to_mei`].
pub fn marker_type_from_mei(value: &str) -> MarkerType {
    match value {
        "segno" => MarkerType::Segno,
        "varsegno" => MarkerType::VarSegno,
        "codab" => MarkerType::Coda,
        "varcoda" => MarkerType::VarCoda,
        "codetta" => MarkerType::Codetta,
        "fine" => MarkerType::Fine,
        "to-coda" => MarkerType::ToCoda,
        "codasym" => MarkerType::ToCodaSym,
        "dacoda" => MarkerType::DaCoda,
        "dadblcoda" => MarkerType::DaDoubleCoda,
        _ => MarkerType::User,
    }
}

/// Suffix of `Convert::jumpToMEI`'s `@type` (`mscore-jump-*`).
pub fn jump_type_to_mei(kind: JumpType) -> &'static str {
    match kind {
        JumpType::Dc => "dc",
        JumpType::DcAlFine => "dc-al-fine",
        JumpType::DcAlCoda => "dc-al-coda",
        JumpType::DsAlCoda => "ds-al-coda",
        JumpType::DsAlFine => "ds-al-fine",
        JumpType::Ds => "ds",
        JumpType::DcAlDoubleCoda => "dc-al-dblcoda",
        JumpType::DsAlDoubleCoda => "ds-al-dblcoda",
        JumpType::Dss => "dss",
        JumpType::DssAlCoda => "dss-al-coda",
        JumpType::DssAlDoubleCoda => "dss-al-dblcoda",
        JumpType::DssAlFine => "dss-al-fine",
        JumpType::User => "user",
    }
}

/// Inverse of [`jump_type_to_mei`].
pub fn jump_type_from_mei(value: &str) -> JumpType {
    match value {
        "dc" => JumpType::Dc,
        "dc-al-fine" => JumpType::DcAlFine,
        "dc-al-coda" => JumpType::DcAlCoda,
        "ds-al-coda" => JumpType::DsAlCoda,
        "ds-al-fine" => JumpType::DsAlFine,
        "ds" => JumpType::Ds,
        "dc-al-dblcoda" => JumpType::DcAlDoubleCoda,
        "ds-al-dblcoda" => JumpType::DsAlDoubleCoda,
        "dss" => JumpType::Dss,
        "dss-al-coda" => JumpType::DssAlCoda,
        "dss-al-dblcoda" => JumpType::DssAlDoubleCoda,
        "dss-al-fine" => JumpType::DssAlFine,
        _ => JumpType::User,
    }
}

/// `@head.shape` (`data.HEADSHAPE`, MEI 5.1). Only the shapes with a distinct
/// standard term are carried; the others stay out of reach rather than being
/// encoded in a private vocabulary.
pub fn head_shape_to_mei(group: &NoteHeadGroup) -> Option<&'static str> {
    match group {
        NoteHeadGroup::Cross | NoteHeadGroup::HeavyCross | NoteHeadGroup::HeavyCrossHat => {
            Some("x")
        }
        NoteHeadGroup::Plus => Some("+"),
        NoteHeadGroup::Circled | NoteHeadGroup::CircledLarge => Some("circle"),
        NoteHeadGroup::Diamond | NoteHeadGroup::DiamondOld | NoteHeadGroup::LargeDiamond => {
            Some("diamond")
        }
        NoteHeadGroup::Slash | NoteHeadGroup::Slashed1 | NoteHeadGroup::Slashed2 => Some("slash"),
        NoteHeadGroup::TriangleUp => Some("isotriangle"),
        NoteHeadGroup::TriangleDown => Some("piewedge"),
        _ => None,
    }
}

/// Inverse of [`head_shape_to_mei`].
pub fn head_shape_from_mei(value: &str) -> Option<NoteHeadGroup> {
    match value {
        "x" => Some(NoteHeadGroup::Cross),
        "+" => Some(NoteHeadGroup::Plus),
        "circle" => Some(NoteHeadGroup::Circled),
        "diamond" => Some(NoteHeadGroup::Diamond),
        "slash" => Some(NoteHeadGroup::Slash),
        "isotriangle" => Some(NoteHeadGroup::TriangleUp),
        "piewedge" => Some(NoteHeadGroup::TriangleDown),
        _ => None,
    }
}

/// Noms de glyphes SMuFL des têtes de note, indexés par type de durée
/// (`whole`, `half`, `quarter`, `double whole`).
///
/// Portage de la table `noteHeads` de `engraving/dom/note.cpp:89` et des noms
/// de `engraving/types/symnames.cpp`. MEI 5.1 `att.noteHeads` porte
/// `@glyph.name` et `@glyph.auth` : `@glyph.auth="smufl"` est la valeur
/// standard (`MEI.shared.xml`, `data.GLYPH_AUTH`).
pub fn head_shape_to_smufl(group: &NoteHeadGroup, kind: DurationType) -> Option<&'static str> {
    let index = match kind {
        DurationType::Long | DurationType::Breve => 3,
        DurationType::Whole => 0,
        DurationType::Half => 1,
        _ => 2,
    };
    let names: &[&str; 4] = match group {
        NoteHeadGroup::Cross => &[
            "noteheadXWhole",
            "noteheadXHalf",
            "noteheadXBlack",
            "noteheadXDoubleWhole",
        ],
        NoteHeadGroup::Plus => &[
            "noteheadPlusWhole",
            "noteheadPlusHalf",
            "noteheadPlusBlack",
            "noteheadPlusDoubleWhole",
        ],
        NoteHeadGroup::XCircle => &[
            "noteheadCircleXWhole",
            "noteheadCircleXHalf",
            "noteheadCircleX",
            "noteheadCircleXDoubleWhole",
        ],
        NoteHeadGroup::WithX => &[
            "noteheadWholeWithX",
            "noteheadHalfWithX",
            "noteheadVoidWithX",
            "noteheadDoubleWholeWithX",
        ],
        NoteHeadGroup::TriangleUp => &[
            "noteheadTriangleUpWhole",
            "noteheadTriangleUpHalf",
            "noteheadTriangleUpBlack",
            "noteheadTriangleUpDoubleWhole",
        ],
        NoteHeadGroup::TriangleDown => &[
            "noteheadTriangleDownWhole",
            "noteheadTriangleDownHalf",
            "noteheadTriangleDownBlack",
            "noteheadTriangleDownDoubleWhole",
        ],
        NoteHeadGroup::Slashed1 => &[
            "noteheadSlashedWhole1",
            "noteheadSlashedHalf1",
            "noteheadSlashedBlack1",
            "noteheadSlashedDoubleWhole1",
        ],
        NoteHeadGroup::Slashed2 => &[
            "noteheadSlashedWhole2",
            "noteheadSlashedHalf2",
            "noteheadSlashedBlack2",
            "noteheadSlashedDoubleWhole2",
        ],
        NoteHeadGroup::Diamond => &[
            "noteheadDiamondWhole",
            "noteheadDiamondHalf",
            "noteheadDiamondBlack",
            "noteheadDiamondDoubleWhole",
        ],
        NoteHeadGroup::DiamondOld => &[
            "noteheadDiamondWholeOld",
            "noteheadDiamondHalfOld",
            "noteheadDiamondBlackOld",
            "noteheadDiamondDoubleWholeOld",
        ],
        NoteHeadGroup::Circled => &[
            "noteheadCircledWhole",
            "noteheadCircledHalf",
            "noteheadCircledBlack",
            "noteheadCircledDoubleWhole",
        ],
        NoteHeadGroup::CircledLarge => &[
            "noteheadCircledWholeLarge",
            "noteheadCircledHalfLarge",
            "noteheadCircledBlackLarge",
            "noteheadCircledDoubleWholeLarge",
        ],
        NoteHeadGroup::LargeArrow => &[
            "noteheadLargeArrowUpWhole",
            "noteheadLargeArrowUpHalf",
            "noteheadLargeArrowUpBlack",
            "noteheadLargeArrowUpDoubleWhole",
        ],
        // Le groupe `BrevisAlt` utilise les glyphes normaux pour whole/half/
        // quarter (note.cpp:119) ; seul le double whole a un glyphe propre,
        // utilisé ici pour distinguer le groupe de manière non ambiguë.
        NoteHeadGroup::BrevisAlt => &["noteheadDoubleWholeSquare"; 4],
        NoteHeadGroup::Slash => &[
            "noteheadSlashWhiteWhole",
            "noteheadSlashWhiteHalf",
            "noteheadSlashHorizontalEnds",
            "noteheadSlashWhiteWhole",
        ],
        NoteHeadGroup::LargeDiamond => &[
            "noteShapeDiamondWhite",
            "noteShapeDiamondWhite",
            "noteShapeDiamondBlack",
            "noteShapeDiamondDoubleWhole",
        ],
        NoteHeadGroup::HeavyCross => &["noteheadHeavyX"; 4],
        NoteHeadGroup::HeavyCrossHat => &["noteheadHeavyXHat"; 4],
        NoteHeadGroup::Do => &["noteDoWhole", "noteDoHalf", "noteDoBlack", "noteDoBlack"],
        NoteHeadGroup::Re => &["noteReWhole", "noteReHalf", "noteReBlack", "noteReBlack"],
        NoteHeadGroup::Mi => &["noteMiWhole", "noteMiHalf", "noteMiBlack", "noteMiBlack"],
        NoteHeadGroup::Fa => &["noteFaWhole", "noteFaHalf", "noteFaBlack", "noteFaBlack"],
        NoteHeadGroup::Sol => &["noteSoWhole", "noteSoHalf", "noteSoBlack", "noteSoBlack"],
        NoteHeadGroup::La => &["noteLaWhole", "noteLaHalf", "noteLaBlack", "noteLaBlack"],
        NoteHeadGroup::Ti => &["noteTiWhole", "noteTiHalf", "noteTiBlack", "noteTiBlack"],
        _ => return None,
    };
    Some(names[index])
}

/// Inverse de [`head_shape_to_smufl`] : un `@glyph.name` SMuFL standard
/// (`@glyph.auth="smufl"`) donne le groupe de tête du modèle.
pub fn head_shape_from_smufl(name: &str) -> Option<NoteHeadGroup> {
    Some(match name {
        "noteheadXWhole" | "noteheadXHalf" | "noteheadXBlack" | "noteheadXDoubleWhole" => {
            NoteHeadGroup::Cross
        }
        "noteheadPlusWhole"
        | "noteheadPlusHalf"
        | "noteheadPlusBlack"
        | "noteheadPlusDoubleWhole" => NoteHeadGroup::Plus,
        "noteheadCircleXWhole"
        | "noteheadCircleXHalf"
        | "noteheadCircleX"
        | "noteheadCircleXDoubleWhole" => NoteHeadGroup::XCircle,
        "noteheadWholeWithX"
        | "noteheadHalfWithX"
        | "noteheadVoidWithX"
        | "noteheadDoubleWholeWithX" => NoteHeadGroup::WithX,
        "noteheadTriangleUpWhole"
        | "noteheadTriangleUpHalf"
        | "noteheadTriangleUpBlack"
        | "noteheadTriangleUpDoubleWhole" => NoteHeadGroup::TriangleUp,
        "noteheadTriangleDownWhole"
        | "noteheadTriangleDownHalf"
        | "noteheadTriangleDownBlack"
        | "noteheadTriangleDownDoubleWhole" => NoteHeadGroup::TriangleDown,
        "noteheadSlashedWhole1"
        | "noteheadSlashedHalf1"
        | "noteheadSlashedBlack1"
        | "noteheadSlashedDoubleWhole1" => NoteHeadGroup::Slashed1,
        "noteheadSlashedWhole2"
        | "noteheadSlashedHalf2"
        | "noteheadSlashedBlack2"
        | "noteheadSlashedDoubleWhole2" => NoteHeadGroup::Slashed2,
        "noteheadDiamondWhole"
        | "noteheadDiamondHalf"
        | "noteheadDiamondBlack"
        | "noteheadDiamondDoubleWhole" => NoteHeadGroup::Diamond,
        "noteheadDiamondWholeOld"
        | "noteheadDiamondHalfOld"
        | "noteheadDiamondBlackOld"
        | "noteheadDiamondDoubleWholeOld" => NoteHeadGroup::DiamondOld,
        "noteheadCircledWhole"
        | "noteheadCircledHalf"
        | "noteheadCircledBlack"
        | "noteheadCircledDoubleWhole" => NoteHeadGroup::Circled,
        "noteheadCircledWholeLarge"
        | "noteheadCircledHalfLarge"
        | "noteheadCircledBlackLarge"
        | "noteheadCircledDoubleWholeLarge" => NoteHeadGroup::CircledLarge,
        "noteheadLargeArrowUpWhole"
        | "noteheadLargeArrowUpHalf"
        | "noteheadLargeArrowUpBlack"
        | "noteheadLargeArrowUpDoubleWhole" => NoteHeadGroup::LargeArrow,
        "noteheadDoubleWholeSquare" => NoteHeadGroup::BrevisAlt,
        "noteheadSlashWhiteWhole" | "noteheadSlashWhiteHalf" | "noteheadSlashHorizontalEnds" => {
            NoteHeadGroup::Slash
        }
        "noteShapeDiamondWhite" | "noteShapeDiamondBlack" | "noteShapeDiamondDoubleWhole" => {
            NoteHeadGroup::LargeDiamond
        }
        "noteheadHeavyX" => NoteHeadGroup::HeavyCross,
        "noteheadHeavyXHat" => NoteHeadGroup::HeavyCrossHat,
        "noteDoWhole" | "noteDoHalf" | "noteDoBlack" => NoteHeadGroup::Do,
        "noteReWhole" | "noteReHalf" | "noteReBlack" => NoteHeadGroup::Re,
        "noteMiWhole" | "noteMiHalf" | "noteMiBlack" => NoteHeadGroup::Mi,
        "noteFaWhole" | "noteFaHalf" | "noteFaBlack" => NoteHeadGroup::Fa,
        "noteSoWhole" | "noteSoHalf" | "noteSoBlack" => NoteHeadGroup::Sol,
        "noteLaWhole" | "noteLaHalf" | "noteLaBlack" => NoteHeadGroup::La,
        "noteTiWhole" | "noteTiHalf" | "noteTiBlack" => NoteHeadGroup::Ti,
        _ => return None,
    })
}

/// `Convert::beamFromMEI` (`meiconverter.cpp:626`).
pub fn beam_from_mei(type_str: &str, prefix: &str) -> BeamMode {
    if has_type_value(type_str, &format!("{prefix}-begin")) {
        BeamMode::Begin
    } else if has_type_value(type_str, &format!("{prefix}-mid")) {
        BeamMode::Mid
    } else if has_type_value(type_str, &format!("{prefix}-none")) {
        BeamMode::None
    } else {
        BeamMode::Auto
    }
}

/// `Convert::beamToMEI` (`meiconverter.cpp:642`).
pub fn beam_to_mei(beam: BeamMode, prefix: &str) -> String {
    let suffix = match beam {
        BeamMode::Begin => "-begin",
        BeamMode::Mid => "-mid",
        BeamMode::None => "-none",
        _ => return String::new(),
    };
    format!("{prefix}{suffix}")
}

/// `Convert::breaksecFromMEI` (`meiconverter.cpp:662`): 1 → BEGIN16, 2 → BEGIN32.
pub fn breaksec_from_mei(value: i32) -> BeamMode {
    match value {
        1 => BeamMode::Begin16,
        2 => BeamMode::Begin32,
        _ => BeamMode::Auto,
    }
}

/// `Convert::breaksecToMEI` (`meiconverter.cpp:677`).
pub fn breaksec_to_mei(beam: BeamMode) -> Option<i32> {
    match beam {
        BeamMode::Begin16 => Some(1),
        BeamMode::Begin32 => Some(2),
        _ => None,
    }
}

/// `Convert::keyFromMEI` (`meiconverter.cpp:2066`): `@keysig` as `Ns`/`Nf`/`0`.
pub fn key_from_mei(value: &str) -> Option<Key> {
    if let Some(count) = value.strip_suffix('s') {
        count.parse::<i8>().ok().map(Key::new)
    } else if let Some(count) = value.strip_suffix('f') {
        count.parse::<i8>().ok().map(|value| Key::new(-value))
    } else if value == "0" || value == "n" {
        Some(Key::C)
    } else {
        None
    }
}

/// `Convert::keyToMEI` (`meiconverter.cpp:2080`).
pub fn key_to_mei(key: Key) -> String {
    let fifths = i32::from(key.fifths()).clamp(-7, 7);
    match fifths.cmp(&0) {
        std::cmp::Ordering::Greater => format!("{fifths}s"),
        std::cmp::Ordering::Less => format!("{}f", -fifths),
        std::cmp::Ordering::Equal => "0".to_owned(),
    }
}

/// `Convert::meterFromMEI` (`meiconverter.cpp:2286`).
///
/// `count` is the raw `@meter.count` (`"3+2"` is not supported, like 4.7.4),
/// `unit` is `@meter.unit`, `sym` is `@meter.sym`.
pub fn meter_from_mei(
    count: Option<&str>,
    unit: Option<i32>,
    sym: Option<&str>,
) -> (Fraction, TimeSigType) {
    let mut fraction = match (count, unit) {
        (Some(count), Some(unit)) => {
            let numerator = count
                .split('+')
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(if sym == Some("common") { 4 } else { 2 });
            Fraction::try_new(numerator, i64::from(unit)).unwrap_or(Fraction::ZERO)
        }
        _ => Fraction::ZERO,
    };
    let kind = match sym {
        Some("common") => {
            fraction = Fraction::new(4, 4);
            TimeSigType::FourFour
        }
        Some("cut") => {
            fraction = Fraction::new(2, 2);
            TimeSigType::AllaBreve
        }
        _ => TimeSigType::Normal,
    };
    if fraction.is_zero() {
        fraction = Fraction::new(4, 4);
    }
    (fraction, kind)
}

/// `Convert::meterToMEI` (`meiconverter.cpp:2321`).
pub fn meter_to_mei(time: &TimeSig) -> (Option<String>, Option<i32>, Option<&'static str>) {
    match time.kind {
        TimeSigType::FourFour => (None, None, Some("common")),
        TimeSigType::AllaBreve => (None, None, Some("cut")),
        _ => (
            Some(time.numerator.to_string()),
            Some(time.denominator),
            None,
        ),
    }
}

/// `Convert::curvedirFromMEI` (`meiconverter.cpp:1112`) : `@curvedir` d'un
/// `<slur>`/`<tie>`/`<lv>` MEI 5.1 (`att.curvature`).
pub fn curvedir_from_mei(value: Option<&str>) -> Option<Placement> {
    match value {
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => None,
    }
}

/// `Convert::curvedirToMEI` (`meiconverter.cpp:1123`).
pub fn curvedir_to_mei(placement: Option<Placement>) -> Option<&'static str> {
    match placement {
        Some(Placement::Above) => Some("above"),
        Some(Placement::Below) => Some("below"),
        _ => None,
    }
}

/// Result of `Convert::measureFromMEI` (`meiconverter.cpp:2211`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeasureInfo {
    /// True when `@n` is absent.
    pub irregular: bool,
    /// Value of `@n` when it starts with a digit.
    pub n: usize,
    /// `@left="rptstart"`.
    pub repeat_start: bool,
    /// End barline from `@right`.
    pub end_bar_line: BarLineType,
    /// `@right` is `rptend` or `rptboth`.
    pub repeat_end: bool,
    /// MuseScore-specific `@type="mscore-repeat-N"`.
    pub repeat_count: u8,
}

/// `Convert::measureFromMEI` (`meiconverter.cpp:2211`).
pub fn measure_from_mei(
    n: Option<&str>,
    left: Option<&str>,
    right: Option<&str>,
    typ: Option<&str>,
) -> MeasureInfo {
    let mut info = MeasureInfo {
        irregular: n.is_none(),
        ..MeasureInfo::default()
    };
    let n = n.unwrap_or_default();
    if n.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        info.n = n.parse::<usize>().unwrap_or(0);
    }
    if let Some(left) = left {
        info.repeat_start = barline_from_mei(left) == BarLineType::StartRepeat;
    }
    if let Some(right) = right {
        info.end_bar_line = barline_from_mei(right);
        info.repeat_end = matches!(
            info.end_bar_line,
            BarLineType::EndRepeat | BarLineType::EndStartRepeat
        );
    }
    if let Some(typ) = typ {
        if let Some(count) = get_type_value_with_prefix(typ, MEASURE_REPEAT_TYPE) {
            info.repeat_count = count.parse::<u8>().unwrap_or(0);
        }
    }
    info
}

/// Attributes written by `Convert::measureToMEI` (`meiconverter.cpp:2254`).
///
/// Returns `(key, value)` pairs in the order used by MuseScore.
pub fn measure_to_mei(measure: &Measure, measure_n: &mut i32) -> Vec<(&'static str, String)> {
    let mut result = Vec::new();
    if measure.len != measure.nominal_len {
        result.push(("metcon", "false".to_owned()));
    }
    if !measure.is_irregular() {
        *measure_n += 1;
        result.push(("n", measure_n.to_string()));
    }
    if measure.start_bar_line.is_repeat_start() {
        result.push(("left", "rptstart".to_owned()));
    }
    if measure.repeat_end() && measure.repeat_count > 0 {
        result.push((
            "type",
            format!("{MEASURE_REPEAT_TYPE}{}", measure.repeat_count),
        ));
    }
    // MuseScore leaves `@right` unset for a normal barline (its
    // `barlineToMEI` returns `BARRENDITION_NONE`, which libmei omits).
    if measure.end_bar_line != BarLineType::Normal {
        result.push(("right", barline_to_mei(measure.end_bar_line).to_owned()));
    }
    result
}

/// Written pitch converted by `Convert::pitchFromMEI` (`meiconverter.cpp:2765`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitchInfo {
    /// MIDI pitch, including the staff's chromatic transposition.
    pub pitch: u8,
    /// Tonal pitch class (MuseScore `Tpc`).
    pub tpc: i8,
    /// Written accidental, if `@accid` is present.
    pub accidental: Option<AccidentalType>,
}

/// `step2tpc` of `engraving/dom/pitchspelling.cpp`: C=14, D=16, …, B=19.
const STEP_TPC: [i8; 7] = [14, 16, 18, 13, 15, 17, 19];

/// `Convert::pitchFromMEI` (`meiconverter.cpp:2765`).
///
/// `accid` is `@accid`, `accid_ges` is `@accid.ges`, `transpose` is the
/// staff's `Interval::chromatic`.
pub fn pitch_from_mei(
    pname: Option<&str>,
    oct: Option<i32>,
    accid: Option<&str>,
    accid_ges: Option<&str>,
    transpose: i32,
) -> PitchInfo {
    let step = match pname
        .and_then(|name| name.as_bytes().first())
        .map(u8::to_ascii_lowercase)
    {
        Some(b'c') => 0,
        Some(b'd') => 1,
        Some(b'e') => 2,
        Some(b'f') => 3,
        Some(b'g') => 4,
        Some(b'a') => 5,
        Some(b'b') => 6,
        _ => 0,
    };
    let oct = oct.unwrap_or(3);
    let written = accid.or(accid_ges);
    let alter = written.map(alter_from_accid).unwrap_or(0);
    let accidental = accid.and_then(accid_from_mei);
    let pitch = [0, 2, 4, 5, 7, 9, 11][step] + alter + (oct + 1) * 12 + transpose;
    PitchInfo {
        pitch: pitch.clamp(0, 127) as u8,
        tpc: STEP_TPC[step] + 7 * alter as i8,
        accidental,
    }
}

/// `Convert::pitchToMEI` (`meiconverter.cpp:2820`): written pitch and
/// accidentals, with `@oct.ges` when the sounding pitch differs.
pub fn pitch_to_mei(
    pitch: u8,
    tpc: i8,
    accidental: Option<AccidentalType>,
    transpose: i32,
) -> (String, i32, Option<&'static str>, Option<&'static str>) {
    // Inverse of `step2tpc`: the TPCs of C..B are 14,16,18,13,15,17,19.
    let index = (i32::from(tpc) * 4).rem_euclid(7) as usize;
    let pname = ['c', 'd', 'e', 'f', 'g', 'a', 'b'][index];
    let alter = (i32::from(tpc) + 8).div_euclid(7) - 3;
    let written_alter = accidental.map(|kind| match kind {
        AccidentalType::Flat | AccidentalType::NaturalFlat => -1,
        AccidentalType::DoubleFlat | AccidentalType::TripleFlat => -2,
        AccidentalType::Sharp | AccidentalType::NaturalSharp => 1,
        AccidentalType::DoubleSharp | AccidentalType::TripleSharp => 2,
        _ => 0,
    });
    let oct = (i32::from(pitch) - transpose - alter) / 12 - 1;
    let written = written_alter.and_then(|value| {
        if value == 0 {
            accid_to_mei(AccidentalType::Natural)
        } else {
            accid_to_mei(accidental.unwrap_or(AccidentalType::None))
        }
    });
    let ges = if alter != 0 && Some(alter) != written_alter {
        Some(match alter {
            -3 => "tf",
            -2 => "ff",
            -1 => "f",
            1 => "s",
            2 => "ss",
            3 => "ts",
            _ => "n",
        })
    } else {
        None
    };
    (pname.to_string(), oct, written, ges)
}

/// `Convert::clefFromMEI` (`meiconverter.cpp:810`): `@shape`, `@line`,
/// `@dis` and `@dis.place` → `ClefType`.
pub fn clef_from_mei(
    shape: Option<&str>,
    line: i32,
    dis: Option<&str>,
    dis_place: Option<&str>,
) -> ClefType {
    let below = dis_place == Some("below");
    let octave = match dis {
        Some("8") => {
            if below {
                -1
            } else {
                1
            }
        }
        Some("15") => {
            if below {
                -2
            } else {
                2
            }
        }
        _ => 0,
    };
    match shape.unwrap_or("none") {
        "G" => match (line, octave) {
            (1, 0) => ClefType::G1,
            (2, 0) => ClefType::G,
            (2, -1) => ClefType::G8Vb,
            (2, 1) => ClefType::G8Va,
            (2, -2) => ClefType::G15Mb,
            (2, 2) => ClefType::G15Ma,
            _ => ClefType::Invalid,
        },
        "F" => match (line, octave) {
            (3, 0) => ClefType::FBaritone,
            (4, 0) => ClefType::F,
            (5, 0) => ClefType::FSubbass,
            (4, -1) => ClefType::F8Vb,
            (4, 1) => ClefType::F8Va,
            (4, -2) => ClefType::F15Mb,
            (4, 2) => ClefType::F15Ma,
            _ => ClefType::Invalid,
        },
        "C" => match (line, octave) {
            (1, _) => ClefType::C1,
            (2, _) => ClefType::C2,
            (3, _) => ClefType::C3,
            (4, _) => ClefType::C4,
            (5, _) => ClefType::C5,
            _ => ClefType::Invalid,
        },
        "perc" => ClefType::Perc,
        _ => ClefType::Invalid,
    }
}

/// `Convert::clefToMEI` (`meiconverter.cpp:933`).
pub fn clef_to_mei(
    clef: ClefType,
) -> (
    &'static str,
    i32,
    Option<&'static str>,
    Option<&'static str>,
) {
    let shape = match clef.sign() {
        scoretk_core::model::ClefSign::G => "G",
        scoretk_core::model::ClefSign::F => "F",
        scoretk_core::model::ClefSign::C => "C",
        scoretk_core::model::ClefSign::Percussion => "perc",
        scoretk_core::model::ClefSign::Tab | scoretk_core::model::ClefSign::None => "none",
    };
    let line = if shape == "perc" {
        0
    } else {
        i32::from(clef.line())
    };
    let octave = clef.octave_change();
    let (dis, place) = match octave {
        -1 => (Some("8"), Some("below")),
        1 => (Some("8"), Some("above")),
        -2 => (Some("15"), Some("below")),
        2 => (Some("15"), Some("above")),
        _ => (None, None),
    };
    (shape, line, dis, place)
}

/// `Convert::tstampFromFraction` (`meiconverter.cpp:3828`).
pub fn tstamp_from_fraction(fraction: Fraction, denominator: i64) -> f64 {
    fraction.as_f64() * denominator as f64 + 1.0
}

/// Denominator of the *unreduced* time signature of a measure.
///
/// `Fraction::new(4, 4)` is reduced to `1/1` in the model, but `@tstamp` is
/// counted in beats of the written `4/4`: recover the denominator from
/// `Measure::time_sig_numerator` and the reduced nominal length.
pub fn tstamp_denominator(measure: &Measure) -> i64 {
    let nominal = measure.nominal_len;
    let scale = if nominal.numerator() != 0 {
        measure.time_sig_numerator / nominal.numerator()
    } else {
        1
    };
    nominal.denominator().max(1) * scale.max(1)
}

/// `Convert::tstampToFraction` (`meiconverter.cpp:3838`): continued-fraction
/// approximation with ten cycles and a precision of 0.01.
pub fn tstamp_to_fraction(tstamp: f64, denominator: i64) -> Fraction {
    let value = (tstamp - 1.0).max(0.0);
    let sign = if value > 0.0 { 1.0 } else { -1.0 };
    let value = value * sign;
    let mut whole = value as i64;
    let mut decimal = value - whole as f64;
    let mut vec1 = [whole as f64, 1.0];
    let mut vec2 = [1.0, 0.0];
    let mut counter = 0;
    while decimal > 0.01 && counter < 10 {
        let new_number = 1.0 / decimal;
        whole = new_number as i64;
        let temporary = vec1;
        vec1 = [
            whole as f64 * vec1[0] + vec2[0],
            whole as f64 * vec1[1] + vec2[1],
        ];
        vec2 = temporary;
        decimal = new_number - whole as f64;
        counter += 1;
    }
    Fraction::new(sign as i64 * vec1[0] as i64, vec1[1] as i64)
        / Fraction::from_integer(denominator)
}

/// `Convert::placeFromMEI` (`meiconverter.cpp:2875`): above for anything but
/// `below`.
pub fn place_from_mei(value: Option<&str>) -> scoretk_core::model::Placement {
    if value == Some("below") {
        scoretk_core::model::Placement::Below
    } else {
        scoretk_core::model::Placement::Above
    }
}

/// `Convert::placeToMEI` (`meiconverter.cpp:2886`).
pub fn place_to_mei(place: scoretk_core::model::Placement) -> &'static str {
    match place {
        scoretk_core::model::Placement::Below => "below",
        _ => "above",
    }
}

/// Builds the pivot `KeySig` of a `scoreDef`/`staffDef`.
pub fn key_sig(value: Option<&str>) -> KeySig {
    KeySig::new(value.and_then(key_from_mei).unwrap_or(Key::C))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_is_a_denominator() {
        assert_eq!(dur_from_mei("4"), Some(DurationType::Quarter));
        assert_eq!(dur_from_mei("breve"), Some(DurationType::Breve));
        assert_eq!(dur_to_mei(DurationType::Eighth), "8");
    }

    #[test]
    fn smufl_note_heads_cover_the_shapes_without_a_headshape_term() {
        use scoretk_core::model::NoteHeadGroup;
        for (group, glyph) in [
            (NoteHeadGroup::Cross, "noteheadXBlack"),
            (NoteHeadGroup::Plus, "noteheadPlusBlack"),
            (NoteHeadGroup::XCircle, "noteheadCircleX"),
            (NoteHeadGroup::Do, "noteDoBlack"),
            (NoteHeadGroup::La, "noteLaBlack"),
            (NoteHeadGroup::HeavyCrossHat, "noteheadHeavyXHat"),
        ] {
            assert_eq!(
                head_shape_to_smufl(&group, DurationType::Quarter),
                Some(glyph)
            );
            assert_eq!(head_shape_from_smufl(glyph), Some(group));
        }
        assert_eq!(
            head_shape_to_smufl(&NoteHeadGroup::Cross, DurationType::Half),
            Some("noteheadXHalf")
        );
        assert!(head_shape_to_smufl(&NoteHeadGroup::Normal, DurationType::Quarter).is_none());
    }

    #[test]
    fn pitch_uses_the_mei_octave_offset() {
        // MuseScore's `pitchFromMEI`: pitch = step + alter + (oct + 1) * 12.
        let pitch = pitch_from_mei(Some("c"), Some(4), None, None, 0);
        assert_eq!(pitch.pitch, 60);
        assert_eq!(pitch.tpc, 14);
    }

    #[test]
    fn key_and_time_signatures() {
        assert_eq!(key_from_mei("3s"), Some(Key::A));
        assert_eq!(key_from_mei("2f"), Some(Key::B_FLAT));
        assert_eq!(key_to_mei(Key::new(-4)), "4f");
        let (fraction, kind) = meter_from_mei(Some("4"), Some(4), Some("common"));
        assert_eq!(fraction, Fraction::new(4, 4));
        assert_eq!(kind, TimeSigType::FourFour);
    }

    #[test]
    fn tstamp_round_trip() {
        let stamp = tstamp_from_fraction(Fraction::QUARTER, 4);
        assert!((stamp - 2.0).abs() < 1e-9);
        assert_eq!(tstamp_to_fraction(stamp, 4), Fraction::QUARTER);
        assert_eq!(tstamp_to_fraction(0.0, 4), Fraction::ZERO);
    }

    #[test]
    fn clefs_and_barlines() {
        assert_eq!(clef_from_mei(Some("G"), 2, None, None), ClefType::G);
        assert_eq!(clef_from_mei(Some("F"), 4, None, None), ClefType::F);
        assert_eq!(barline_from_mei("rptstart"), BarLineType::StartRepeat);
        assert_eq!(barline_to_mei(BarLineType::Double), "dbl");
    }

    #[test]
    fn repeat_marks_keep_musescore_types() {
        use scoretk_core::model::{JumpType, MarkerType};
        assert_eq!(
            marker_type_from_mei(marker_type_to_mei(MarkerType::ToCoda)),
            MarkerType::ToCoda
        );
        assert_eq!(
            jump_type_from_mei(jump_type_to_mei(JumpType::DsAlCoda)),
            JumpType::DsAlCoda
        );
    }

    #[test]
    fn note_heads_use_standard_mei_shapes() {
        use scoretk_core::model::NoteHeadGroup;
        assert_eq!(head_shape_to_mei(&NoteHeadGroup::Cross), Some("x"));
        assert_eq!(head_shape_to_mei(&NoteHeadGroup::Plus), Some("+"));
        assert_eq!(head_shape_to_mei(&NoteHeadGroup::Diamond), Some("diamond"));
        assert_eq!(head_shape_from_mei("circle"), Some(NoteHeadGroup::Circled));
        assert_eq!(head_shape_to_mei(&NoteHeadGroup::Do), None);
    }

    #[test]
    fn articulations_use_mei_names() {
        use scoretk_core::model::ArticulationType;
        assert_eq!(artic_to_mei(&ArticulationType::Staccato), Some("stacc"));
        assert_eq!(artic_to_mei(&ArticulationType::Accent), Some("acc"));
        assert_eq!(artic_to_mei(&ArticulationType::Tenuto), Some("ten"));
        assert_eq!(artic_to_mei(&ArticulationType::Marcato), Some("marc"));
    }

    #[test]
    fn gestural_durations_are_exact() {
        use scoretk_core::model::DurationType;
        assert_eq!(
            duration_from_ticks(Fraction::new(1, 2)),
            Some((DurationType::Half, 0))
        );
        assert_eq!(
            duration_from_ticks(Fraction::new(3, 16)),
            Some((DurationType::Eighth, 1))
        );
        assert_eq!(duration_from_ticks(Fraction::new(1, 3)), None);
    }

    #[test]
    fn measurements_and_font_sizes_use_mei_units() {
        assert_eq!(inches_to_mei(0.3937), "0.39370in");
        assert!((measurement_to_inches("0.39370in").unwrap() - 0.3937).abs() < 1e-9);
        assert!((measurement_to_inches("10mm").unwrap() - 0.3937008).abs() < 1e-6);
        assert!((measurement_to_mm("1.76400mm").unwrap() - 1.764).abs() < 1e-9);
        assert_eq!(points_to_mei(24.0), "24.00pt");
        assert_eq!(font_size_to_points("24pt"), Some(24.0));
        assert_eq!(font_size_to_points("x-large"), None);
    }

    #[test]
    fn frame_texts_are_placed_in_mei_cells() {
        use scoretk_core::model::CreditStyle;
        assert_eq!(
            text_to_mei(&CreditStyle::Title),
            (1, "title", Some("x-large"))
        );
        assert_eq!(
            text_to_mei(&CreditStyle::Subtitle),
            (1, "subtitle", Some("large"))
        );
        assert_eq!(text_to_mei(&CreditStyle::Composer), (8, "composer", None));
        assert_eq!(text_to_mei(&CreditStyle::Lyricist), (6, "poet", None));
        assert_eq!(cell_to_align(1), ("top", "center"));
        assert_eq!(cell_to_align(6), ("bottom", "left"));
        assert_eq!(cell_to_align(8), ("bottom", "right"));
    }

    #[test]
    fn stem_directions_round_trip() {
        use scoretk_core::model::StemDirection;
        assert_eq!(stem_from_mei("up"), Some(StemDirection::Up));
        assert_eq!(stem_from_mei("down"), Some(StemDirection::Down));
        assert_eq!(stem_from_mei("none"), None);
        assert_eq!(stem_to_mei(StemDirection::Up), Some("up"));
        assert_eq!(stem_to_mei(StemDirection::Down), Some("down"));
        assert_eq!(stem_to_mei(StemDirection::Auto), None);
    }
}
