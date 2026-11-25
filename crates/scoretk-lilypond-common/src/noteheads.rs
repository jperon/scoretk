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

//! Table des styles de tête de note partagée par les deux directions.
//!
//! Les noms standards viennent de la Notation Reference 2.26, annexe B.9
//! (`cross`, `xcircle`, `triangle`, `slash`, `harmonic`, `do`…), confirmés par
//! `scm/lily/output-lib.scm::select-head-glyph` et les glyphes réellement
//! présents dans `emmentaler-13.otf`. Un style standard est écrit pour chaque
//! groupe qui en a un ; le lecteur le relit directement, sans markup privé.
//!
//! Les groupes sans style standard (`Plus`, `WithX`, `TriangleDown`,
//! `Slashed1`, `Slashed2`, `DiamondOld`, `Circled`, `CircledLarge`,
//! `LargeArrow`, `BrevisAlt`, `LargeDiamond`, `HeavyCross`, `HeavyCrossHat`,
//! `Other`) restent approchés visuellement par `cross`/`triangle`/… mais leur
//! identité exacte n'a pas de carrier LilyPond : `has_standard_style` les
//! distingue et `out_of_reach` documente les valeurs exclues de la déclaration
//! au niveau de la valeur (et non du genre entier).

use scoretk_core::model::NoteHeadGroup;

/// Nom LilyPond d'un groupe de têtes, ou `None` pour la tête normale.
pub fn style(group: &NoteHeadGroup) -> Option<&'static str> {
    let name = match group {
        NoteHeadGroup::Normal => return None,
        NoteHeadGroup::Cross => "cross",
        NoteHeadGroup::Plus => "cross", // `plus` absent de LilyPond 2.26 ; le markup garde le groupe exact.
        NoteHeadGroup::XCircle => "xcircle",
        NoteHeadGroup::WithX => "cross",
        NoteHeadGroup::TriangleUp => "triangle",
        NoteHeadGroup::TriangleDown => "triangle",
        NoteHeadGroup::Slashed1 => "slash",
        NoteHeadGroup::Slashed2 => "slash",
        NoteHeadGroup::Diamond => "harmonic",
        NoteHeadGroup::DiamondOld => "harmonic",
        NoteHeadGroup::Circled => "xcircle",
        NoteHeadGroup::CircledLarge => "xcircle",
        NoteHeadGroup::LargeArrow => "triangle",
        NoteHeadGroup::BrevisAlt => "cross",
        NoteHeadGroup::Slash => "slash",
        NoteHeadGroup::LargeDiamond => "harmonic",
        NoteHeadGroup::Do => "do",
        NoteHeadGroup::Re => "re",
        NoteHeadGroup::Mi => "mi",
        NoteHeadGroup::Fa => "fa",
        NoteHeadGroup::Sol => "sol",
        NoteHeadGroup::La => "la",
        NoteHeadGroup::Ti => "ti",
        NoteHeadGroup::HeavyCross => "cross",
        NoteHeadGroup::HeavyCrossHat => "cross",
        NoteHeadGroup::Other(_) => "cross",
    };
    Some(name)
}

/// Identifiant exact du groupe, transporté par un markup privé quand le nom
/// LilyPond ne suffit pas à distinguer deux groupes proches.
pub fn name(group: &NoteHeadGroup) -> Option<&'static str> {
    let name = match group {
        NoteHeadGroup::Normal => return None,
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
        NoteHeadGroup::BrevisAlt => "brevis-alt",
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
        NoteHeadGroup::Other(_) => "other",
    };
    Some(name)
}

/// Groupe correspondant à l'identifiant exact.
pub fn group_from_name(ident: &str) -> Option<NoteHeadGroup> {
    [
        NoteHeadGroup::Cross,
        NoteHeadGroup::Plus,
        NoteHeadGroup::XCircle,
        NoteHeadGroup::WithX,
        NoteHeadGroup::TriangleUp,
        NoteHeadGroup::TriangleDown,
        NoteHeadGroup::Slashed1,
        NoteHeadGroup::Slashed2,
        NoteHeadGroup::Diamond,
        NoteHeadGroup::DiamondOld,
        NoteHeadGroup::Circled,
        NoteHeadGroup::CircledLarge,
        NoteHeadGroup::LargeArrow,
        NoteHeadGroup::BrevisAlt,
        NoteHeadGroup::Slash,
        NoteHeadGroup::LargeDiamond,
        NoteHeadGroup::Do,
        NoteHeadGroup::Re,
        NoteHeadGroup::Mi,
        NoteHeadGroup::Fa,
        NoteHeadGroup::Sol,
        NoteHeadGroup::La,
        NoteHeadGroup::Ti,
        NoteHeadGroup::HeavyCross,
        NoteHeadGroup::HeavyCrossHat,
        NoteHeadGroup::Other(String::new()),
    ]
    .into_iter()
    .find(|candidate| name(candidate) == Some(ident))
}

/// Groupe correspondant à un nom de style lu dans `\tweak style #'…`.
pub fn group(style: &str) -> Option<NoteHeadGroup> {
    // Le lexer livre `#'cross` (ou `'cross` dans l'ancienne syntaxe) comme un
    // seul jeton Scheme : retirer les deux préfixes, sinon aucune tête n'était
    // retrouvée et seul le markup privé permettait la relecture.
    let group = match style
        .trim()
        .trim_start_matches(['#', '\''])
        .to_ascii_lowercase()
        .as_str()
    {
        "cross" => NoteHeadGroup::Cross,
        "plus" => NoteHeadGroup::Plus,
        "xcircle" => NoteHeadGroup::XCircle,
        "triangle" => NoteHeadGroup::TriangleUp,
        "slash" => NoteHeadGroup::Slash,
        "harmonic" | "harmonic-mixed" => NoteHeadGroup::Diamond,
        "do" => NoteHeadGroup::Do,
        "re" => NoteHeadGroup::Re,
        "mi" => NoteHeadGroup::Mi,
        "fa" => NoteHeadGroup::Fa,
        "sol" => NoteHeadGroup::Sol,
        "la" => NoteHeadGroup::La,
        "ti" => NoteHeadGroup::Ti,
        _ => return None,
    };
    Some(group)
}

/// Vrai si le style LilyPond du groupe est standard **et** univoque : il
/// identifie le groupe à la relecture sans markup privé (Notation Reference
/// §B.9, `select-head-glyph`). `Plus` se dessine en croix mais LilyPond n'a pas
/// de glyphe `plus` ; `TriangleDown`, `Slashed1/2`, `DiamondOld`, `Circled`,
/// `CircledLarge`, `LargeArrow`, `BrevisAlt`, `LargeDiamond`, `HeavyCross`,
/// `HeavyCrossHat`, `WithX` et `Other` n'ont pas non plus de style dédié.
pub fn has_standard_style(group: &NoteHeadGroup) -> bool {
    matches!(
        group,
        NoteHeadGroup::Cross
            | NoteHeadGroup::XCircle
            | NoteHeadGroup::TriangleUp
            | NoteHeadGroup::Slash
            | NoteHeadGroup::Diamond
            | NoteHeadGroup::Do
            | NoteHeadGroup::Re
            | NoteHeadGroup::Mi
            | NoteHeadGroup::Fa
            | NoteHeadGroup::Sol
            | NoteHeadGroup::La
            | NoteHeadGroup::Ti
    )
}

/// Vrai si une valeur d'inventaire `Auto:<groupe>` désigne un groupe privé de
/// style LilyPond univoque. Seules ces valeurs sortent de la déclaration
/// `note-head` ; le genre entier y reste.
pub fn out_of_reach(value: &str) -> bool {
    if value.contains(":Other(") {
        return true;
    }
    // `Debug` de `NoteHeadGroup` donne `CircledLarge` alors que `name()`
    // écrit `circled-large` : comparer les identifiants normalisés.
    let normalize = |text: &str| {
        text.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let Some(group) = value.split(':').nth(1) else {
        return false;
    };
    let group = normalize(group);
    [
        "plus",
        "withx",
        "triangledown",
        "slashed1",
        "slashed2",
        "diamondold",
        "circled",
        "circledlarge",
        "largearrow",
        "brevisalt",
        "largediamond",
        "heavycross",
        "heavycrosshat",
        "other",
    ]
    .iter()
    .any(|candidate| normalize(candidate) == group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tous_les_groupes_du_modele_ont_un_style_relisible() {
        for expected in [
            NoteHeadGroup::Cross,
            NoteHeadGroup::Plus,
            NoteHeadGroup::XCircle,
            NoteHeadGroup::WithX,
            NoteHeadGroup::TriangleUp,
            NoteHeadGroup::TriangleDown,
            NoteHeadGroup::Slashed1,
            NoteHeadGroup::Slashed2,
            NoteHeadGroup::Diamond,
            NoteHeadGroup::DiamondOld,
            NoteHeadGroup::Circled,
            NoteHeadGroup::CircledLarge,
            NoteHeadGroup::LargeArrow,
            NoteHeadGroup::BrevisAlt,
            NoteHeadGroup::Slash,
            NoteHeadGroup::LargeDiamond,
            NoteHeadGroup::Do,
            NoteHeadGroup::Re,
            NoteHeadGroup::Mi,
            NoteHeadGroup::Fa,
            NoteHeadGroup::Sol,
            NoteHeadGroup::La,
            NoteHeadGroup::Ti,
            NoteHeadGroup::HeavyCross,
            NoteHeadGroup::HeavyCrossHat,
        ] {
            let visual = style(&expected).expect("un style visuel");
            assert!(
                !visual.starts_with("scoretk-"),
                "style compilable : {visual}"
            );
            let exact = name(&expected).expect("un identifiant exact");
            assert_eq!(group_from_name(exact), Some(expected));
        }
    }

    #[test]
    fn styles_scheme_avec_prefixe_sont_reconnus() {
        assert_eq!(group("#'cross"), Some(NoteHeadGroup::Cross));
        assert_eq!(group("#'xcircle"), Some(NoteHeadGroup::XCircle));
        assert_eq!(group("#'harmonic-mixed"), Some(NoteHeadGroup::Diamond));
        assert_eq!(group("#'do"), Some(NoteHeadGroup::Do));
        assert_eq!(group("'triangle"), Some(NoteHeadGroup::TriangleUp));
    }

    #[test]
    fn seuls_les_groupes_sans_style_standard_sont_hors_declaration() {
        for value in [
            "Auto:Cross",
            "Auto:XCircle",
            "Auto:TriangleUp",
            "Auto:Slash",
            "Auto:Diamond",
            "Auto:Do",
            "Auto:Re",
            "Auto:Mi",
            "Auto:Fa",
            "Auto:Sol",
            "Auto:La",
            "Auto:Ti",
        ] {
            assert!(!out_of_reach(value), "{value} a un style standard");
        }
        for value in [
            "Auto:Plus",
            "Auto:WithX",
            "Auto:TriangleDown",
            "Auto:Slashed1",
            "Auto:Slashed2",
            "Auto:DiamondOld",
            "Auto:Circled",
            "Auto:CircledLarge",
            "Auto:LargeArrow",
            "Auto:BrevisAlt",
            "Auto:LargeDiamond",
            "Auto:HeavyCross",
            "Auto:HeavyCrossHat",
            "Auto:Other(foo)",
        ] {
            assert!(out_of_reach(value), "{value} n'a pas de style univoque");
        }
    }
}
