//! Traduction des noms SMuFL et des valeurs MuseScore vers le modèle.
//!
//! Ces fonctions sont partagées par les lecteurs 3.x et 4.x : les deux
//! formats écrivent les mêmes noms de symboles, seuls les conteneurs XML
//! diffèrent. Elles ne manipulent que des chaînes, ce qui évite de coupler les
//! deux lecteurs.

use crate::model::{
    ArpeggioType, ArticulationAnchor, ArticulationType, ChordLineType, FermataType, GlissandoType,
    LineStyle, OrnamentType, TrillType,
};

/// Découpe un nom SMuFL en base et ancre verticale.
///
/// MuseScore encode l'ancre dans le suffixe `Above` / `Below` du nom.
pub(crate) fn split_anchor(name: &str) -> (&str, ArticulationAnchor) {
    if let Some(base) = name.strip_suffix("Below") {
        (base, ArticulationAnchor::Down)
    } else if let Some(base) = name.strip_suffix("Above") {
        (base, ArticulationAnchor::Up)
    } else {
        (name, ArticulationAnchor::Auto)
    }
}

/// Articulations produites par un nom SMuFL.
///
/// MuseScore éclate les articulations composées en deux symboles simples à la
/// lecture (`splitArticulations`, `engraving/dom/articulation.cpp`) : un
/// `articTenutoStaccatoBelow` devient un staccato et un tenuto, tous deux
/// ancrés en dessous. L'ordre retenu est celui du corpus de référence
/// (`staccato` avant `tenuto`, `tenuto` avant `accent`).
pub(crate) fn articulations(name: &str) -> Vec<(ArticulationType, ArticulationAnchor)> {
    let (base, anchor) = split_anchor(name);
    let (first, second) = match base {
        "articAccentStaccato" => (ArticulationType::Staccato, ArticulationType::Accent),
        "articTenutoAccent" => (ArticulationType::Tenuto, ArticulationType::Accent),
        "articTenutoStaccato" => (ArticulationType::Staccato, ArticulationType::Tenuto),
        "articMarcatoStaccato" => (ArticulationType::Staccato, ArticulationType::Marcato),
        "articMarcatoTenuto" => (ArticulationType::Tenuto, ArticulationType::Marcato),
        _ => {
            return simple_articulation(base, anchor)
                .map(|kind| vec![(kind, anchor)])
                .unwrap_or_default()
        }
    };
    vec![(first, anchor), (second, anchor)]
}

/// Articulation simple, sans éclatement.
fn simple_articulation(base: &str, anchor: ArticulationAnchor) -> Option<ArticulationType> {
    let _ = anchor;
    Some(match base {
        "articStaccato" => ArticulationType::Staccato,
        "articStaccatissimo" | "articStaccatissimoWedge" => ArticulationType::Staccatissimo,
        "articAccent" => ArticulationType::Accent,
        "articMarcato" => ArticulationType::Marcato,
        "articTenuto" => ArticulationType::Tenuto,
        "articStress" => ArticulationType::Stress,
        "articUnstress" => ArticulationType::Unstress,
        "articSoftAccent" => ArticulationType::SoftAccent,
        "articSoftAccentStaccato" => ArticulationType::SoftAccentStaccato,
        "articSoftAccentTenuto" => ArticulationType::SoftAccentTenuto,
        "articSoftAccentTenutoStaccato" => ArticulationType::SoftAccentTenutoStaccato,
        "articStaccatissimoStroke" => ArticulationType::Spiccato,
        "fermata" => ArticulationType::Fermata,
        "fermataShort" => ArticulationType::FermataShort,
        "fermataLong" => ArticulationType::FermataLong,
        "fermataVeryShort" => ArticulationType::FermataVeryShort,
        "fermataVeryLong" => ArticulationType::FermataVeryLong,
        "breathMarkComma" | "breathMark" => ArticulationType::BreathMark,
        "caesura" | "caesuraCurved" | "caesuraShort" => ArticulationType::Caesura,
        "stringsUpBow" | "articUpBow" => ArticulationType::UpBow,
        "stringsDownBow" | "articDownBow" => ArticulationType::DownBow,
        "stringsHarmonic" | "articHarmonic" => ArticulationType::Harmonic,
        "brassMuteOpen" | "articOpen" => ArticulationType::Open,
        "brassMuteClosed" | "articStopped" => ArticulationType::Stopped,
        "pluckedSnapPizzicato" | "articSnapPizzicato" => ArticulationType::SnapPizzicato,
        _ => return None,
    })
}

/// Point d'orgue à partir de son nom SMuFL.
pub(crate) fn fermata_type(name: &str) -> Option<FermataType> {
    let (base, _) = split_anchor(name);
    Some(match base {
        "fermata" => FermataType::Normal,
        "fermataShort" => FermataType::Short,
        "fermataLong" => FermataType::Long,
        "fermataVeryShort" => FermataType::VeryShort,
        "fermataVeryLong" => FermataType::VeryLong,
        "fermataLongHenze" => FermataType::LongHenze,
        "fermataShortHenze" => FermataType::ShortHenze,
        "curlewSign" => FermataType::Curlew,
        _ => return None,
    })
}

/// Ornement à partir de son nom SMuFL.
pub(crate) fn ornament_type(name: &str) -> OrnamentType {
    match name {
        "ornamentTrill" => OrnamentType::TrillMark,
        "ornamentTurn" => OrnamentType::Turn,
        "ornamentTurnInverted" => OrnamentType::InvertedTurn,
        "ornamentTurnSlash" => OrnamentType::TurnSlash,
        "ornamentTurnUp" => OrnamentType::VerticalTurn,
        "ornamentTurnUpS" => OrnamentType::InvertedVerticalTurn,
        "ornamentMordent" => OrnamentType::Mordent,
        "ornamentShortTrill" => OrnamentType::InvertedMordent,
        "ornamentTremblement" => OrnamentType::InvertedMordentLong,
        "ornamentPrallMordent" => OrnamentType::MordentLong,
        "ornamentUpPrall" => OrnamentType::InvertedMordentLongApproachBelow,
        "ornamentPrecompMordentUpperPrefix" | "ornamentLinePrall" => {
            OrnamentType::InvertedMordentLongApproachAbove
        }
        "ornamentUpMordent" => OrnamentType::MordentLongApproachBelow,
        "ornamentDownMordent" => OrnamentType::MordentLongApproachAbove,
        "ornamentPrallDown" => OrnamentType::InvertedMordentLongDepartureBelow,
        "ornamentPrallUp" => OrnamentType::InvertedMordentLongDepartureAbove,
        "ornamentHaydn" => OrnamentType::Haydn,
        "ornamentPrecompSlide" => OrnamentType::Schleifer,
        other => OrnamentType::Other(other.to_owned()),
    }
}

/// Trille à partir de son nom MuseScore (`<Trill><subtype>`).
pub(crate) fn trill_type(name: &str) -> TrillType {
    match name {
        "trill" => TrillType::TrillLine,
        "pure" => TrillType::PureLine,
        "upprall" => TrillType::UpperPrallLine,
        "prallprall" => TrillType::PrallPrallLine,
        "downprall" => TrillType::DownPrallLine,
        _ => TrillType::TrillLine,
    }
}

/// Nombre de barres d'un trémolo et nature du trémolo.
///
/// `r8`…`r256` désignent un trémolo simple, `c8`…`c256` un trémolo à deux
/// notes (`TremoloType` dans `engraving/dom/tremolo.h`). Le type 5.0 valant
/// `r8`/`c8` sans préfixe est accepté.
pub(crate) fn tremolo_bars(name: &str) -> Option<(bool, u8)> {
    let (double, figure) = match name.as_bytes().first() {
        Some(b'c') => (true, &name[1..]),
        Some(b'r') => (false, &name[1..]),
        _ => (false, name),
    };
    let count = match figure {
        "8" => 1,
        "16" => 2,
        "32" => 3,
        "64" => 4,
        "128" => 5,
        "256" => 6,
        _ => return None,
    };
    Some((double, count))
}

/// Arpège à partir de `<subtype>` (nom ou numéro historique).
///
/// Les valeurs numériques de MuseScore 3 sont `0` normal, `1` haut, `2` bas,
/// `3` crochet ; les fichiers 4.x écrivent les mêmes numéros.
pub(crate) fn arpeggio_type(value: &str) -> ArpeggioType {
    match value.trim() {
        "1" | "up" => ArpeggioType::Up,
        "2" | "down" => ArpeggioType::Down,
        "3" | "bracket" => ArpeggioType::Bracket,
        _ => ArpeggioType::Normal,
    }
}

/// Ligne d'accord à partir de `<subtype>` (nom ou numéro).
pub(crate) fn chord_line_type(value: &str) -> Option<ChordLineType> {
    Some(match value.trim() {
        "1" | "fall" | "falloff" => ChordLineType::Fall,
        "2" | "doit" => ChordLineType::Doit,
        "3" | "plop" => ChordLineType::Plop,
        "4" | "scoop" => ChordLineType::Scoop,
        _ => return None,
    })
}

/// Style de trait à partir de son nom (`<lineStyle>`, `LineType`).
pub(crate) fn line_style(name: &str) -> LineStyle {
    match name.trim() {
        "dashed" => LineStyle::Dashed,
        "dotted" => LineStyle::Dotted,
        _ => LineStyle::Solid,
    }
}

/// Glissando à partir de `<subtype>` et du style de trait.
///
/// MuseScore 3 utilise `0` (droit) et `1` (ondulé) ; les fichiers 4.x
/// écrivent aussi `straight` et `wavy`.
pub(crate) fn glissando_type(subtype: Option<&str>, style: LineStyle) -> GlissandoType {
    match subtype.map(str::trim) {
        Some("1" | "wavy") => GlissandoType::Wavy,
        _ => GlissandoType::Straight(style),
    }
}
