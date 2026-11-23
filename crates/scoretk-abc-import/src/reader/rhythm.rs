// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `abc2xml.py` de Willem Vree (LGPL, version 269), `simplify`
// (ligne 590), `doBroken`/`convertBroken` (lignes 594/614), `note_length`
// (lignes 362/473), `tuplet_start` (lignes 395/484/2012) et `mkTmod`
// (ligne 728), modifié (réécrit en Rust) en 2026. Aucune ligne du script
// n'est copiée. Le comptage des agréments (`doGrace`, ligne 680) est cité dans
// `reader/music.rs`, qui conserve la durée écrite dans le modèle pivot.
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

//! Durées écrites, rythme brisé, n-olets et agréments, d'après `abc2xml.py`.
//!
//! Les fonctions de ce module sont pures : elles traduisent les actions de
//! grammaire et les transformations de mesure du script en valeurs du modèle
//! pivot. La consommation des caractères reste dans `reader/music.rs`, qui
//! délègue ici les calculs en citant la fonction Python d'origine.

use scoretk_core::model::Fraction;

/// Plus grand diviseur commun de deux entiers positifs (Euclide).
///
/// Port de `abc2xml.py::simplify` (ligne 590) : `while b: a, b = b, a % b`.
fn pgcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let reste = a % b;
        a = b;
        b = reste;
    }
    a.max(1)
}

/// Réduit `(a, b)` comme `abc2xml.py::simplify` (ligne 590).
pub(crate) fn simplify(a: i64, b: i64) -> (i64, i64) {
    let diviseur = pgcd(a, b);
    (a / diviseur, b / diviseur)
}

/// Multiplicateur de durée lu par la grammaire `note_length`
/// (`abc2xml.py` lignes 362 et 473).
///
/// La grammaire est `Optional(number, 1) + Group(ZeroOrMore('/')) +
/// Optional(number, 2)` et l'action produit `(num, (den << slashs) >> 1)`.
/// Le texte `3/2` donne donc `(3, 2)` (3/2 unité), `//` donne `(1, 4)` et
/// `2/3` donne `(2, 3)`. Le résultat est un multiple de l'unité `L:` ; le
/// lecteur le multiplie ensuite par cette unité. Un dénominateur nul est
/// interprété comme 1 par `mkNote` (ligne 1310), comme dans le script.
pub(crate) fn note_length(text: &str) -> Option<Fraction> {
    if text.is_empty() {
        return Some(Fraction::ONE);
    }
    // Le texte est `[num][slashs][den]` : le premier `/` est le séparateur
    // du `split`, les suivants appartiennent au groupe de slashs.
    let (avant, reste, separateur) = match text.split_once('/') {
        Some((a, r)) => (a, r, true),
        None => (text, "", false),
    };
    // `Optional(number, 1)` : un nombre absent vaut 1 (par exemple `//`).
    let num: i64 = if avant.is_empty() {
        1
    } else {
        avant.parse().ok()?
    };
    let slashs_sup = reste.chars().take_while(|c| *c == '/').count();
    let slashs = if separateur { 1 + slashs_sup } else { 0 };
    if slashs > 20 {
        return None;
    }
    let chiffres = &reste[slashs_sup..];
    // `Optional(number, 2)` : le dénominateur par défaut vaut 2.
    let den: i64 = if chiffres.is_empty() {
        2
    } else {
        chiffres.parse().ok()?
    };
    if num == 0 {
        // ABC 2.1 §4.3 ne définit pas de durée nulle ; `abc2xml.py::abcdur`
        // (ligne 502) renvoie une chaîne vide pour `dur == 0`, ce qui écrit
        // la note sans multiplicateur, donc avec l'unité `L:` par défaut
        // (`note.test__abcZeroLength.abc`).Suivre l'alternative plutôt que
        // rejeter l'air entier.
        return Some(Fraction::ONE);
    }
    // Action de grammaire (ligne 473) : `(den << slashs) >> 1`. Un décalage
    // qui déborde i64 est une durée invalide, pas un panic.
    let decale = den.checked_shl(slashs as u32)?;
    let (num, den) = simplify(num, decale >> 1);
    let den = if den == 0 { 1 } else { den };
    Fraction::try_new(num, den)
}

/// Facteurs appliqués au rythme brisé, d'après `abc2xml.py::doBroken`
/// (ligne 594) appelé par `convertBroken` (ligne 614).
///
/// Le marqueur suit la note qu'il modifie : `doBroken(prev, brk, cur)` change
/// la durée de `prev` (facteur `ici`) et celle de `cur` (facteur `suivant`).
/// `>` vaut 3/2 + 1/2, `<` 1/2 + 3/2, `>>` 7/4 + 1/4 et `<<` 1/4 + 7/4.
/// Au-delà de deux caractères, le script renonce (`else: return`) tout en
/// supprimant le marqueur : les deux facteurs valent alors 1.
pub(crate) fn broken_factors(symbol: char, levels: usize) -> (Fraction, Fraction) {
    // `Fraction::new` réduit automatiquement, comme `simplify` dans le script.
    match (symbol, levels) {
        ('>', 1) => (Fraction::new(3, 2), Fraction::new(1, 2)),
        ('<', 1) => (Fraction::new(1, 2), Fraction::new(3, 2)),
        ('>', 2) => (Fraction::new(7, 4), Fraction::new(1, 4)),
        ('<', 2) => (Fraction::new(1, 4), Fraction::new(7, 4)),
        _ => (Fraction::ONE, Fraction::ONE),
    }
}

/// Résout `(tmnum, tmden, ntup)` d'un n-olet, d'après `abc2xml.py`
/// `tuplet_start` (lignes 395, 484 et 2012).
///
/// Le script lit `n`, `into`, `nts` puis applique : `if into == 0: into = 3
/// if n in [2,4,8] else 2` et `if nts == 0: nts = n`. Le triplet `(3` garde
/// donc 3 notes pour 2 (tmnum=3, tmden=2), tandis que `(2` est une duole
/// (tmnum=2, tmden=3). `mkTmod` (ligne 728) écrit ensuite ces deux nombres
/// dans `<time-modification>` : le modèle les porte via le ratio du n-olet.
pub(crate) fn tuplet_start(numbers: &[i64]) -> (i64, i64, i64) {
    let n = numbers.first().copied().unwrap_or(0);
    let mut into = numbers.get(1).copied().unwrap_or(0);
    let mut nts = numbers.get(2).copied().unwrap_or(0);
    if into == 0 {
        into = if [2, 4, 8].contains(&n) { 3 } else { 2 };
    }
    if nts == 0 {
        nts = n;
    }
    (n, into, nts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longueurs_ecrites() {
        assert_eq!(note_length(""), Some(Fraction::ONE));
        assert_eq!(note_length("2"), Some(Fraction::from_integer(2)));
        assert_eq!(note_length("/"), Some(Fraction::new(1, 2)));
        assert_eq!(note_length("//"), Some(Fraction::new(1, 4)));
        assert_eq!(note_length("3/2"), Some(Fraction::new(3, 2)));
        assert_eq!(note_length("2/3"), Some(Fraction::new(2, 3)));
        assert_eq!(note_length("3//2"), Some(Fraction::new(3, 4)));
        assert_eq!(note_length("/0"), Some(Fraction::ONE));
        assert_eq!(note_length("x"), None);
    }

    #[test]
    fn n_olets_et_rythme_brise() {
        assert_eq!(tuplet_start(&[3]), (3, 2, 3));
        assert_eq!(tuplet_start(&[2]), (2, 3, 2));
        assert_eq!(tuplet_start(&[8]), (8, 3, 8));
        assert_eq!(tuplet_start(&[6]), (6, 2, 6));
        assert_eq!(tuplet_start(&[3, 2, 4]), (3, 2, 4));
        assert_eq!(
            broken_factors('>', 1),
            (Fraction::new(3, 2), Fraction::new(1, 2))
        );
        assert_eq!(
            broken_factors('<', 2),
            (Fraction::new(1, 4), Fraction::new(7, 4))
        );
        assert_eq!(broken_factors('>', 3), (Fraction::ONE, Fraction::ONE));
    }
}
