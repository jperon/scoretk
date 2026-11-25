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

//! ABC 2.1 §3.1 : `W:` est un texte imprimé, distinct des paroles `w:` (§5).
//! Il transporte les crédits du modèle dans les deux sens.

use scoretk::model::{Credit, CreditStyle, Fraction, Measure, Part, RichText, Score, TboxText};
use scoretk::{read, write, Format, Options};

fn partition_avec_credits() -> Score {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Pno"));
    score.add_staff(part);
    score.add_measure(Measure::new(0, Fraction::ZERO, Fraction::ONE));
    for text in ["Premier crédit", "Second crédit"] {
        score.add_credit(Credit::new(RichText::new(text), CreditStyle::Title));
    }
    score
}

#[test]
fn credits_vont_et_reviennent_par_le_champ_w_majuscule() {
    let source = partition_avec_credits();
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(text.contains("W:Premier crédit\n"), "{text}");
    assert!(text.contains("W:Second crédit\n"), "{text}");
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    let credits: Vec<String> = back
        .engraving
        .credits
        .iter()
        .map(|credit| credit.text.plain().to_owned())
        .collect();
    assert_eq!(credits, ["Premier crédit", "Second crédit"]);
}

#[test]
fn textes_de_cadre_vont_et_reviennent_par_la_directive_privee() {
    let mut source = partition_avec_credits();
    source.add_tbox_text(TboxText::new(RichText::new("Encart de titre"), 0));
    let abc = write(&source, Format::Abc, &Options::default()).expect("écriture ABC");
    let text = String::from_utf8(abc.clone()).expect("UTF-8");
    assert!(
        text.contains("%%scoretk tbox 0 Encart de titre\n"),
        "{text}"
    );
    let back = read(&abc, Format::Abc).expect("lecture ABC");
    assert_eq!(back.engraving.tbox_texts.len(), 1);
    assert_eq!(back.engraving.tbox_texts[0].text.plain(), "Encart de titre");
}

#[test]
fn w_majuscule_n_est_pas_pris_pour_des_paroles() {
    let score =
        read(b"X:1\nK:C\nW:Texte imprime\nC4|]\nw:parole\n", Format::Abc).expect("lecture ABC");
    assert_eq!(score.engraving.credits.len(), 1);
    assert_eq!(score.engraving.credits[0].text.plain(), "Texte imprime");
    assert_eq!(
        scoretk::model::inventory(&score)
            .items
            .iter()
            .filter(|item| item.kind == "lyric")
            .count(),
        1,
        "le `w:` minuscule reste attaché à la note"
    );
}
