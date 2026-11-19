// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `abc2xml.py` de Willem Vree (LGPL, version 269), `doMaat`
// (ligne 674) et de la grammaire `measure`/`bar_right` (lignes 428-436),
// modifié (réécrit en Rust) en 2026. Aucune ligne du script n'est copiée.
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

//! Longueur ÉCRITE des mesures, d'après `abc2xml.py`.
//!
//! Dans la grammaire d'`abc2xml.py` (lignes 428-436), une `measure` est un
//! groupe d'éléments écrits terminé par une `bar_right` : aucun silence n'est
//! ajouté pour l'étirer jusqu'à la longueur nominale de `M:`. L'action
//! `doMaat` (ligne 674) ne fait que transformer les éléments du groupe
//! (`convertBroken`, `convertChord`) et renvoie ce groupe tel quel. Le
//! MusicXML produit n'a donc pas de `<forward>` de remplissage pour une
//! mesure courte.

use scoretk_core::model::Fraction;

/// Longueur d'une mesure, gardée à sa valeur ÉCRITE.
///
/// Port de `abc2xml.py::doMaat` (ligne 674) et des règles de grammaire
/// `measure`/`bar_right` (lignes 428-436). `actual` est la somme des durées
/// écrites dans la mesure ; `nominal` est la longueur de `M:`. L'anacrouse
/// (première mesure courte) est conservée à sa longueur écrite, comme avant
/// le port : c'est simplement le cas `actual < nominal`, traité ici comme
/// toute autre mesure positive.
pub(crate) fn do_maat(actual: Fraction, nominal: Fraction) -> Fraction {
    if actual.is_zero() {
        // Une mesure sans note reste une mesure : `abc2xml.py` écrit tout de
        // même un `<measure>` et le modèle exige une longueur positive.
        if nominal.is_positive() {
            nominal
        } else {
            Fraction::from_integer(1)
        }
    } else {
        // Ni `actual.max(nominal)`, ni complément jusqu'à `nominal` : c'est
        // le modèle, via `Measure::len`, qui pilote ensuite le `<forward>`
        // de l'export MusicXML (`move_to_tick` vers `measure.end_tick()`).
        actual
    }
}
