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
//! (`cross`, `plus`, `xcircle`, `triangle`, `slash`, `harmonic`, `do`…). Les
//! groupes de MuseScore sans équivalent exact reçoivent un nom privé
//! `scoretk-…`, ignoré par LilyPond mais relisible par notre lecteur.

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
    let group = match style
        .trim()
        .trim_start_matches('\'')
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
}
