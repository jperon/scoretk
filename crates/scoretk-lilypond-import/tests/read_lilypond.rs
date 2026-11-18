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

//! Petites sources LilyPond écrites à la main.
//!
//! Le corpus `tests/corpus/ly/` est produit par l'écrivain (donc en mode
//! `\fixed` et en anglais). Ces tests couvrent ce que l'écrivain n'émet pas :
//! notation relative, langue néerlandaise par défaut, variables, reprises,
//! transposition, paroles et commandes ignorées.

use std::collections::HashMap;

use scoretk_core::model::Fraction;
use scoretk_core::model::{
    ArticulationType, BarLineType, ChordRestKind, ClefType, DurationType, GraceType, KeyMode,
    LyricsSyllabic, SegmentItem, SegmentType, Track,
};
use scoretk_lilypond_import::{
    read_lilypond, read_lilypond_reporting, read_lilypond_with_includes,
};

/// Lit une musique dans une portée unique, en mode `\fixed c'`.
///
/// Le mode fixe place les noms non marqués au do central, comme le corpus ;
/// les tests d'octave pure appellent [`read_lilypond`] directement.
fn lire(music: &str) -> scoretk_core::model::Score {
    let source = format!(
        "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score {{ \\new Staff {{ \\fixed c' {{ {music} }} }} }}\n"
    );
    read_lilypond(&source).unwrap_or_else(|error| panic!("lecture : {error}"))
}

/// Hauteurs des notes de l'arène, dans l'ordre.
fn hauteurs(score: &scoretk_core::model::Score) -> Vec<u8> {
    score.notes.iter().map(|note| note.pitch).collect()
}

#[test]
fn relative_et_octaves() {
    let score = lire("\\relative c' { c4 d e f g a b c }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69, 71, 72]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn mode_fixe_et_absolu() {
    // `\fixed c'` : un nom sans marque vaut le do central.
    let score = lire("\\fixed c' { c4 c' c, }");
    assert_eq!(hauteurs(&score), vec![60, 72, 48]);
    // Notation absolue : `c` vaut deux octaves sous le do central.
    let score = read_lilypond("\\score { \\new Staff { c4 c' } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![48, 60]);
}

#[test]
fn notation_neerlandaise_par_defaut() {
    // Sans `\language`, LilyPond lit le néerlandais : `cis`, `des`, `es`.
    let score = read_lilypond("\\score { \\new Staff { cis4 des4 es4 } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![49, 49, 51]);
    let tpc: Vec<i8> = score.notes.iter().map(|note| note.tpc).collect();
    assert_eq!(
        tpc,
        vec![
            scoretk_core::model::tpc::C_SHARP,
            scoretk_core::model::tpc::D_FLAT,
            scoretk_core::model::tpc::E_FLAT
        ]
    );
}

#[test]
fn variables_et_reprise() {
    let score = lire("m = { c1 d1 } \\repeat volta 2 { \\m } e1");
    // Deux mesures de reprise, puis une mesure.
    assert_eq!(score.measures.len(), 3);
    assert_eq!(score.measures[0].start_bar_line, BarLineType::StartRepeat);
    assert_eq!(score.measures[1].end_bar_line, BarLineType::EndRepeat);
    assert_eq!(score.measures[1].repeat_count, 2);
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
}

#[test]
fn accords_articulations_et_liaison() {
    let score = lire("<c e g>4-. c4~ c4");
    assert_eq!(hauteurs(&score), vec![60, 64, 67, 60, 60]);
    let mesure = &score.measures[0];
    let segment = mesure
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::ChordRest)
        .expect("segment");
    let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(Track::from_raw(0)) else {
        panic!("accord attendu");
    };
    let chord = chord_rest.chord_ref().expect("accord");
    assert_eq!(chord.articulations.len(), 1);
    assert_eq!(chord.articulations[0].kind, ArticulationType::Staccato);
}

#[test]
fn note_relative_et_paroles() {
    let score = read_lilypond(
        r#"\score { \new Staff << \new Voice = "v" { \fixed c' { a4 b4 c'4 d'4 e'4 } } \addlyrics { A -- ve Ma -- ri -- a } >> }"#,
    )
    .expect("lecture");
    let syllabes: Vec<(LyricsSyllabic, String)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .flat_map(|chord_rest| &chord_rest.lyrics)
        .map(|lyrics| (lyrics.syllabic, lyrics.text.clone()))
        .collect();
    assert_eq!(
        syllabes,
        vec![
            (LyricsSyllabic::Begin, "A".to_owned()),
            (LyricsSyllabic::End, "ve".to_owned()),
            (LyricsSyllabic::Begin, "Ma".to_owned()),
            (LyricsSyllabic::Middle, "ri".to_owned()),
            (LyricsSyllabic::End, "a".to_owned()),
        ]
    );
}

#[test]
fn transposition() {
    let score = lire("\\transpose c d { c4 }");
    let note = &score.notes[0];
    assert_eq!(note.pitch, 62);
    assert_eq!(note.tpc, scoretk_core::model::tpc::D);
    assert_eq!(note.tpc_transposed, scoretk_core::model::tpc::C);
}

#[test]
fn armure_clef_et_mesure() {
    let score = lire("\\clef bass \\key bf \\major \\time 3/4 R2. \\bar \"|.\"");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert_eq!(score.measures[0].end_bar_line, BarLineType::End);
    let segment = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::HeaderClef)
        .expect("clef d'en-tête");
    let Some(SegmentItem::Clef(clef)) = segment.element(Track::from_raw(0)) else {
        panic!("clef attendue");
    };
    assert_eq!(clef.concert, ClefType::F);
    let segment = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::KeySig)
        .expect("armure");
    let Some(SegmentItem::KeySig(key)) = segment.element(Track::from_raw(0)) else {
        panic!("armure attendue");
    };
    assert_eq!(key.concert_key.fifths(), -2);
    assert_eq!(key.mode, KeyMode::Unknown);
    // Silence de mesure entière.
    let Some(SegmentItem::ChordRest(chord_rest)) = score.measures[0]
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentType::ChordRest)
        .and_then(|segment| segment.element(Track::from_raw(0)))
    else {
        panic!("silence attendu");
    };
    assert!(chord_rest.is_full_measure_rest());
}

#[test]
fn levees_et_n_olets() {
    let score = lire("\\partial 4 c4 | \\tuplet 3/2 { d8 e8 f8 } g2");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, Fraction::QUARTER);
    assert_eq!(score.measures[1].len, Fraction::ONE);
    assert!(score.measures[0].is_irregular());
    assert_eq!(score.tuplets.len(), 1);
    let tuplet = &score.tuplets[0];
    assert_eq!(tuplet.ratio, Fraction::new(3, 2));
    assert_eq!(tuplet.base_len, Fraction::new(1, 8));
}

#[test]
fn ornements() {
    let score = lire("\\grace { b8 } c4 \\afterGrace d4 { e16 }");
    let mut graces = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(Track::from_raw(0)) {
                if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                    graces.extend(
                        chord
                            .grace_chords
                            .iter()
                            .map(|grace| (grace.chord.grace, grace.duration.kind)),
                    );
                }
            }
        }
    }
    assert_eq!(
        graces,
        vec![
            (Some(GraceType::Appoggiatura), DurationType::Eighth),
            (Some(GraceType::Grace8After), DurationType::Sixteenth)
        ]
    );
}

#[test]
fn commandes_ignorees_avec_avertissement() {
    let source = "\\version \"2.24.0\"\n\
                  \\score { \\new Staff { c4 \\uneCommandeInconnue d4 #(ly:make-moment 1/4) e4 } }";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![48, 50, 52]);
    assert!(
        !warnings.is_empty(),
        "les commandes ignorées doivent être signalées"
    );
}

#[test]
fn musique_de_haut_niveau_relative() {
    let score = read_lilypond("\\relative c' { c4 d e f | g1 }").expect("lecture");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn musique_de_haut_niveau_absolue() {
    let score = read_lilypond("{ c'4 d' e' f' }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn musique_de_haut_niveau_simultanee() {
    let score = read_lilypond("<< \\new Staff { c'1 } \\new Staff { e'1 } >>").expect("lecture");
    assert_eq!(score.parts.len(), 2);
    assert_eq!(score.staves.len(), 2);
    assert_eq!(hauteurs(&score), vec![60, 64]);
}

#[test]
fn musique_de_haut_niveau_new_staff_relative() {
    let score = read_lilypond("\\new Staff \\relative { c'4 d e f }").expect("lecture");
    assert_eq!(score.staves.len(), 1);
    // Sans référence, `\relative` part du do central : `c'` monte d'une octave.
    assert_eq!(hauteurs(&score), vec![72, 74, 76, 77]);
}

#[test]
fn musique_de_haut_niveau_fixed() {
    let score = read_lilypond("\\fixed c' { c d e f }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
}

#[test]
fn musique_de_haut_niveau_transposee() {
    let score = read_lilypond("\\transpose c d { c'1 }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![62]);
}

#[test]
fn variable_musicale_de_haut_niveau() {
    let score = read_lilypond("melodie = \\relative { c'4 d e f }\n\\melodie").expect("lecture");
    // Sans référence, `\relative` part du do central : `c'` monte d'une octave.
    assert_eq!(hauteurs(&score), vec![72, 74, 76, 77]);
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn paroles_de_haut_niveau() {
    let score = read_lilypond("\\new Staff { c'1 } \\addlyrics { la }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    let syllabes: Vec<&str> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .flat_map(|chord_rest| &chord_rest.lyrics)
        .map(|lyrics| lyrics.text.as_str())
        .collect();
    assert_eq!(syllabes, vec!["la"]);
}

#[test]
fn livre_et_partie_de_livre() {
    let score = read_lilypond("\\book { \\header { workTitle = \"T\" } \\score { { c'1 } } }")
        .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    assert_eq!(score.metadata.work_title, "T");
    let score = read_lilypond("\\bookpart { \\score { { e'1 } } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![64]);
    // La musique de haut niveau est aussi acceptée dans un livre.
    let score = read_lilypond("\\book { { d'1 } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![62]);
}

#[test]
fn partition_explicite_prioritaire_et_premiere() {
    // Un `\score` explicite l'emporte sur la musique de haut niveau.
    let (score, warnings) =
        read_lilypond_reporting("{ c'1 }\n\\score { { d'1 } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![62]);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "partition supplémentaire ignorée")
            .count(),
        1
    );
    // Entre plusieurs partitions, la première gagne et les autres sont
    // signalées.
    let (score, warnings) =
        read_lilypond_reporting("\\score { { c'1 } }\n\\score { { d'1 } }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "partition supplémentaire ignorée")
            .count(),
        1
    );
}

#[test]
fn sans_musique_erreur() {
    let error =
        read_lilypond("\\version \"2.24.0\"\n\\header { title = \"x\" }\n\\markup { \"y\" }")
            .expect_err("aucune musique");
    assert!(
        error.to_string().contains("aucune musique"),
        "message : {error}"
    );
}

/// Paires (hauteur de départ, hauteur d'arrivée) des liaisons de prolongation.
fn liaisons(score: &scoretk_core::model::Score) -> Vec<(u8, u8)> {
    score
        .notes
        .iter()
        .filter_map(|note| {
            note.tie_forward
                .map(|next| (note.pitch, score.note(next).pitch))
        })
        .collect()
}

#[test]
fn liaison_apres_note_simple() {
    let score = lire("c4~ c4 d2");
    assert_eq!(liaisons(&score), vec![(60, 60)]);
    assert_eq!(
        score
            .notes
            .iter()
            .filter(|note| note.tie_back.is_some())
            .count(),
        1
    );
}

#[test]
fn liaison_apres_note_avec_nuance_avant() {
    let score = lire("c4\\p ~ c4 d2");
    assert_eq!(liaisons(&score), vec![(60, 60)]);
}

#[test]
fn liaison_apres_note_avec_nuance_apres() {
    let score = lire("c4 ~ \\p c4 d2");
    assert_eq!(liaisons(&score), vec![(60, 60)]);
}

#[test]
fn liaison_apres_accord() {
    let score = lire("<c e>4~ <c e>4 r2");
    assert_eq!(liaisons(&score), vec![(60, 60), (64, 64)]);
}

#[test]
fn liaison_interne_a_un_accord() {
    // Seul le do est lié : le mi ne porte pas de `~`.
    let score = lire("<c~ e>4 <c e>4 r2");
    assert_eq!(liaisons(&score), vec![(60, 60)]);
}

#[test]
fn pas_de_liaison_entre_hauteurs_differentes() {
    let score = lire("c4~ d4 r2");
    assert!(liaisons(&score).is_empty());
}

#[test]
fn liaison_a_travers_une_barre_de_mesure() {
    let score = lire("c1~ c1");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(liaisons(&score), vec![(60, 60)]);
    assert_eq!(
        score
            .notes
            .iter()
            .filter(|note| note.tie_back.is_some())
            .count(),
        1
    );
}

#[test]
fn time_denominateur_nul_ne_panique_pas() {
    // `Fraction::new` paniquait sur un dénominateur nul : l'indication est
    // désormais rejetée avec un avertissement.
    let (score, warnings) =
        read_lilypond_reporting("{ \\time 3/0 c'4 }").expect("lecture sans panique");
    assert_eq!(hauteurs(&score), vec![60]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message == "indication de mesure invalide ignorée"),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn partial_duree_nulle_ne_panique_pas() {
    let _ = read_lilypond_reporting("{ \\partial 0 c'4 }");
}

#[test]
fn measure_length_denominateur_nul_ne_panique_pas() {
    let _ = read_lilypond_reporting("{ \\set Timing.measureLength = #3/0 c'4 }");
}

#[test]
fn tuplet_denominateur_nul_ne_panique_pas() {
    let _ = read_lilypond_reporting("{ \\tuplet 3/0 c'4 }");
}

#[test]
fn times_ratio_nul_ne_panique_pas() {
    let _ = read_lilypond_reporting("{ \\times 0/2 c'4 }");
}

#[test]
fn multiplicateur_denominateur_nul_ne_panique_pas() {
    let _ = read_lilypond_reporting("{ c4*1/0 }");
}

#[test]
fn expressions_scheme_en_tete_ignorees_sans_bruit() {
    // Chaque expression Scheme est un seul datum : ni ses mots ni ses
    // parenthèses ne rejaillissent sur la musique qui suit.
    let (score, warnings) = read_lilypond_reporting(
        "#(ly:set-option 'warning-as-error #t)\n\
         #(define (f x) (+ x 1))\n\
         { c'4 d' e' f' }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("mot")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn expressions_scheme_dans_la_musique() {
    // Propriétés Scheme, quote, booléen, chaîne parenthésée, caractère,
    // commentaire et musique embarquée : la musique reste intacte.
    let (score, warnings) = read_lilypond_reporting(
        r#"\version "2.24.0"
\score { \new Staff { \fixed c' {
  \override NoteHead.color = #(rgb-color 1 0 0)
  c4 d
  #'(1 . 2)
  ##t
  #"chaîne ) avec parenthèse"
  #\)
  #(list 1 ; ) dans un commentaire
     2)
  #(define musique #{ c'4 #})
  e4 f
} } }
"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("mot")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn musique_embarquee_au_niveau_superieur() {
    // regression/embedded-strings-comments.ly : les chaînes et commentaires
    // LilyPond intérieurs ne doivent pas confondre le lecteur.
    let (score, warnings) = read_lilypond_reporting(
        r#"\version "2.21.0"
\layout { ragged-right = ##t }

$#{ c'4-"(#)" %
    e'-"($)" %{$%}g'2-"(%)" %{%}
#}
"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 64, 67]);
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("commande ignorée")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn musique_embarquee_dans_une_affectation() {
    // Un bloc `#{ … #}` est une valeur musicale, y compris après `=`.
    let score = lire("x = #{ c4 d4 #} \\x e4");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
}

#[test]
fn fonction_musicale_embarquee_et_appels_successifs() {
    // regression/trill-spanner-consecutive.ly : quatre appels, deux notes
    // chacun, réunis dans la partition implicite.
    let (score, warnings) = read_lilypond_reporting(
        r#"\version "2.23.9"

test =
#(define-music-function (inc) (number?)
   #{
     \override Score.SpacingSpanner.spacing-increment = #inc
     c'2\startTrillSpan c'2\startTrillSpan
   #})

\test 2.0
\test 2.1
\test 2.2
\test 2.3
"#,
    )
    .expect("lecture");
    assert_eq!(score.notes.len(), 8);
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("commande ignorée")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn fonction_musicale_a_parametre_musical() {
    let score = lire("dbl = #(define-music-function (m) (ly:music?) #{ $m $m #}) \\dbl { c4 d4 }");
    assert_eq!(hauteurs(&score), vec![60, 62, 60, 62]);
}

#[test]
fn fonction_scheme_pure_avertie_une_fois() {
    // Corps sans `#{ … #}` : les arguments sont consommés, mais rien n'est
    // joué et l'avertissement n'est émis qu'au premier appel.
    let (score, warnings) = read_lilypond_reporting(
        "f = #(define-music-function (x) (number?) (ly:message \"~a\" x))\n\
         \\f 1\n\\f 2\n{ c'4 }\n",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    let appels = warnings
        .iter()
        .filter(|warning| warning.message.contains("sans corps musical"))
        .count();
    assert_eq!(appels, 1, "avertissements : {warnings:?}");
}

#[test]
fn fonction_void_consommee_sans_avertissement() {
    // `define-void-function` : appel consommé sans musique ni avertissement.
    let (score, warnings) = read_lilypond_reporting(
        "v = #(define-void-function (x) (number?) (ly:message \"~a\" x))\n\
         \\v 3\n{ c'4 }\n",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    assert!(warnings.is_empty(), "avertissements : {warnings:?}");
}

#[test]
fn fonction_musicale_recursive_ne_panique_pas() {
    let (score, warnings) = read_lilypond_reporting(
        "loop = #(define-music-function (m) (ly:music?) #{ \\loop $m #})\n\
         \\loop { c'4 }\n{ d'4 }\n",
    )
    .expect("lecture sans panique");
    assert_eq!(hauteurs(&score), vec![62]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("profondeur d'expansion")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn measurelength_en_scheme() {
    let score = lire("\\set Timing.measureLength = #(ly:make-moment 3/4) c4 d e");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
}

/// Lit une source complète en exigeant l'absence d'avertissement sur
/// `\include`.
fn lire_sans_avertissement_d_include(source: &str) -> scoretk_core::model::Score {
    let (score, warnings) =
        read_lilypond_reporting(source).unwrap_or_else(|error| panic!("lecture : {error}"));
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("include")),
        "avertissements : {warnings:?}"
    );
    score
}

#[test]
fn include_italiano_puis_relative() {
    // Ancienne façon de choisir la langue des notes : `\include` du fichier de
    // langue livré par LilyPond vaut `\language "italiano"`.
    let score = lire_sans_avertissement_d_include(
        "\\version \"2.24.0\"\n\
         \\include \"italiano.ly\"\n\
         \\relative do' { do4 re mi fa sol1 sib4 fad }\n",
    );
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 70, 66]);
}

#[test]
fn include_english_alterations() {
    let score = lire_sans_avertissement_d_include(
        "\\include \"english.ly\"\n\
         \\score { \\new Staff { \\fixed c' { cs4 bf } } }\n",
    );
    assert_eq!(hauteurs(&score), vec![61, 70]);
}

#[test]
fn include_de_langue_dans_la_musique() {
    // `\include` vaut aussi au milieu de la musique, pas seulement au niveau
    // supérieur.
    let score = lire_sans_avertissement_d_include(
        "\\score { \\new Staff { \\include \"italiano.ly\" do'4 re' } }\n",
    );
    assert_eq!(hauteurs(&score), vec![60, 62]);
}

#[test]
fn language_espagnol() {
    let score = lire_sans_avertissement_d_include(
        "\\language \"español\"\n\
         \\score { \\new Staff { do'4 sib' } }\n",
    );
    assert_eq!(hauteurs(&score), vec![60, 70]);
}

#[test]
fn language_francais_precompose_et_decompose() {
    let precompose = lire_sans_avertissement_d_include(
        "\\language \"français\"\n\
         \\score { \\new Staff { ré'4 mib' } }\n",
    );
    assert_eq!(hauteurs(&precompose), vec![62, 63]);
    // Même nom avec « c » suivi de U+0327 (cédille combinante).
    let decompose = lire_sans_avertissement_d_include(
        "\\language \"franc\u{327}ais\"\n\
         \\score { \\new Staff { ré'4 } }\n",
    );
    assert_eq!(hauteurs(&decompose), vec![62]);
}

#[test]
fn une_note_par_fichier_de_langue() {
    let cas: [(&str, &str, u8); 12] = [
        ("catalan.ly", "dod'4", 61),
        ("deutsch.ly", "h'4", 71),
        ("english.ly", "cs'4", 61),
        ("espanol.ly", "dos'4", 61),
        ("francais.ly", "ré'4", 62),
        ("italiano.ly", "fad'4", 66),
        ("nederlands.ly", "cis'4", 61),
        ("norsk.ly", "cess'4", 59),
        ("portugues.ly", "dos'4", 61),
        ("suomi.ly", "as'4", 68),
        ("svenska.ly", "ciss'4", 61),
        ("vlaams.ly", "dok'4", 61),
    ];
    for (fichier, musique, attendu) in cas {
        let source = format!(
            "\\include \"{fichier}\"\n\
             \\score {{ \\new Staff {{ {musique} }} }}\n"
        );
        let score = lire_sans_avertissement_d_include(&source);
        assert_eq!(hauteurs(&score), vec![attendu], "{fichier}");
    }
}

#[test]
fn include_de_fichier_livre_ignore_sans_avertissement() {
    let score = lire_sans_avertissement_d_include(
        "\\include \"articulate.ly\"\n\
         \\include \"gregorian.ly\"\n\
         \\score { \\new Staff { c'4 d' } }\n",
    );
    assert_eq!(hauteurs(&score), vec![60, 62]);
}

/// Résolveur en mémoire : `resolve(nom, _)` renvoie le texte de `fichiers`.
fn resolveur<'a>(
    fichiers: &'a HashMap<&'a str, &'a str>,
) -> impl FnMut(&str, Option<&str>) -> Option<String> + 'a {
    move |nom, _| fichiers.get(nom).map(|texte| (*texte).to_owned())
}

#[test]
fn include_au_niveau_superieur_definit_une_variable() {
    let fichiers = HashMap::from([("macros.ly", "m = { c'4 d' }")]);
    let (score, warnings) = read_lilypond_with_includes(
        "\\include \"macros.ly\"\n{ \\m e'4 }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn include_au_milieu_d_une_voix() {
    let fichiers = HashMap::from([("more.ly", "d'4 e'")]);
    let (score, warnings) = read_lilypond_with_includes(
        "\\score { \\new Staff { c'4 \\include \"more.ly\" f'4 } }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn include_imbrique_avec_chemin_relatif() {
    let fichiers = HashMap::from([
        ("sub/a.ly", "\\include \"b.ly\"\na = { c'4 }"),
        ("b.ly", "b = { d'4 }"),
    ]);
    let mut appels = Vec::new();
    let (score, warnings) = {
        let mut resolve = |nom: &str, incluant: Option<&str>| {
            appels.push((nom.to_owned(), incluant.map(str::to_owned)));
            fichiers.get(nom).map(|texte| (*texte).to_owned())
        };
        read_lilypond_with_includes("\\include \"sub/a.ly\"\n{ \\a \\b e'4 }\n", &mut resolve)
            .expect("lecture")
    };
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        appels,
        vec![
            ("sub/a.ly".to_owned(), None),
            ("b.ly".to_owned(), Some("sub/a.ly".to_owned()))
        ]
    );
}

#[test]
fn include_cyclique_averti_sans_boucle() {
    let fichiers = HashMap::from([
        ("a.ly", "\\include \"b.ly\"\na = { c'4 }"),
        ("b.ly", "\\include \"a.ly\"\nb = { d'4 }"),
    ]);
    let (score, warnings) = read_lilypond_with_includes(
        "\\include \"a.ly\"\n{ \\a \\b }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("cyclique")),
        "{warnings:?}"
    );
}

#[test]
fn include_absent_averti() {
    let fichiers = HashMap::new();
    let (score, warnings) = read_lilypond_with_includes(
        "\\include \"absent.ly\"\n{ c'4 }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("\\include ignoré")),
        "{warnings:?}"
    );
}

#[test]
fn fichier_de_langue_ne_passe_pas_par_le_resolveur() {
    let mut appels = 0usize;
    let (score, warnings) = {
        let mut resolve = |_: &str, _: Option<&str>| {
            appels += 1;
            None
        };
        read_lilypond_with_includes("\\include \"italiano.ly\"\n{ do'4 re' }\n", &mut resolve)
            .expect("lecture")
    };
    assert_eq!(appels, 0);
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

fn lire_absolue(music: &str) -> scoretk_core::model::Score {
    let source = format!("\\score {{ \\new Staff {{ {music} }} }}\n");
    read_lilypond(&source).unwrap_or_else(|error| panic!("lecture : {error}"))
}
fn durees(score: &scoretk_core::model::Score) -> Vec<Fraction> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .map(|chord_rest| chord_rest.ticks.expect("durée mise à l'échelle"))
        .collect()
}
#[test]
fn scale_durations() {
    // `\scaleDurations 2/3` vaut `c4*2/3` sur chaque note : 1/6 de ronde,
    // sans crochet de n-olet.
    let score = lire_absolue("\\time 3/4 \\scaleDurations 2/3 { c'4 d' e' f' g' a' }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69]);
    assert_eq!(durees(&score), vec![Fraction::new(1, 6); 6]);
    assert_eq!(score.measures[0].len, Fraction::new(3, 4));
    assert_eq!(score.measures[0].nominal_len, Fraction::new(3, 4));
}
#[test]
fn scale_durations_fraction_scheme() {
    let score = lire_absolue("\\scaleDurations #'(2 . 3) { c'4 }");
    assert_eq!(durees(&score), vec![Fraction::new(1, 6)]);
    let score = lire_absolue("\\scaleDurations #(ly:make-moment 2 3) { c'4 }");
    assert_eq!(durees(&score), vec![Fraction::new(1, 6)]);
}
#[test]
fn scale_durations_imbrique() {
    // Les facteurs se composent par produit : 1/2 * 2/3 * 1/4 = 1/12.
    let score = lire_absolue("\\scaleDurations 1/2 { \\scaleDurations 2/3 { c'4 } }");
    assert_eq!(durees(&score), vec![Fraction::new(1, 12)]);
}
#[test]
fn scale_durations_denominateur_nul_ne_panique_pas() {
    let source = "\\score { \\new Staff { \\scaleDurations 3/0 { c'4 } } }";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture sans panique");
    assert_eq!(hauteurs(&score), vec![60]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("scaleDurations")),
        "avertissements : {warnings:?}"
    );
}

/// Paires (hauteur, décalage de portée) des notes de la voix 0, dans l'ordre.
fn notes_et_portees(score: &scoretk_core::model::Score) -> Vec<(u8, i8)> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter_map(|chord_rest| match &chord_rest.kind {
            ChordRestKind::Chord(chord) => Some(
                chord
                    .notes
                    .iter()
                    .map(|&id| (score.note(id).pitch, chord.staff_move))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn change_de_portee_dans_la_voix() {
    let (score, warnings) = read_lilypond_reporting(
        r#"\version "2.24.0"
\language "english"
\score {
  \new PianoStaff <<
    \new Staff = "haut" { \fixed c' {
      c4 d \change Staff = "bas" e f \change Staff = haut g a
    } }
    \new Staff = "bas" { r1 r1 }
  >>
}"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69]);
    assert_eq!(score.measures.len(), 2);
    // Les notes changent de portée sans quitter la voix d'origine.
    assert_eq!(
        notes_et_portees(&score),
        vec![(60, 0), (62, 0), (64, 1), (65, 1), (67, 0), (69, 0),]
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("ignoré")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn change_vers_portee_inconnue() {
    let (score, warnings) = read_lilypond_reporting(
        r#"\score { \new Staff = "seule" { \fixed c' { c4 \change Staff = "absente" d4 } } }"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    // La portée cible n'existe pas : avertissement, notes gardées sur place.
    assert_eq!(notes_et_portees(&score), vec![(60, 0), (62, 0)]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message == "portée inconnue : absente"),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn change_vers_portee_inconnue_avertissement_unique() {
    // Quatre notes après le `\change` : un seul avertissement, daté de la
    // commande et non de chaque accord (ni de 0:0).
    let source = r#"\score {
  \new Staff = "seule" { \fixed c' {
    c4 \change Staff = "absente" d e f g
  } }
}"#;
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
    assert_eq!(
        notes_et_portees(&score),
        vec![(60, 0), (62, 0), (64, 0), (65, 0), (67, 0)]
    );
    let inconnues: Vec<_> = warnings
        .iter()
        .filter(|warning| warning.message == "portée inconnue : absente")
        .collect();
    assert_eq!(inconnues.len(), 1, "avertissements : {warnings:?}");
    let ligne = source
        .lines()
        .position(|line| line.contains("\\change"))
        .expect("ligne du change");
    let colonne = source
        .lines()
        .find(|line| line.contains("\\change"))
        .expect("ligne du change")
        .find("\\change")
        .expect("colonne du change");
    assert_eq!(
        (inconnues[0].line, inconnues[0].column),
        (ligne as u32 + 1, colonne as u32 + 1),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn change_vers_portee_d_autre_partie() {
    let (score, warnings) = read_lilypond_reporting(
        r#"\version "2.24.0"
\language "english"
\score {
  \new Staff = "piano" { \fixed c' { c4 \change Staff = "flute" d e f g } }
  \new Staff = "flute" { \fixed c' { c'1 } }
}"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 72]);
    // "flute" existe mais appartient à une autre partie : pas de staffMove.
    assert_eq!(
        notes_et_portees(&score),
        vec![(60, 0), (62, 0), (64, 0), (65, 0), (67, 0)]
    );
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "portée inconnue : flute")
            .count(),
        1,
        "avertissements : {warnings:?}"
    );
}

#[test]
fn change_dans_beam_cross_staff() {
    // Source de régression externe : le test n'a pas d'effet si elle est
    // absente (extraction fraîche du dépôt, sans la copie LilyPond).
    let Ok(source) = std::fs::read_to_string(
        "/home/jacques/programmes/musique/lilypond-ref/input/regression/beam-cross-staff.ly",
    ) else {
        return;
    };
    assert!(source.contains("\\change"), "source de régression attendue");
    let (score, warnings) = read_lilypond_reporting(&source).expect("lecture");
    assert!(!score.notes.is_empty());
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.to_lowercase().contains("change")),
        "avertissements : {warnings:?}"
    );
}

fn longueurs(score: &scoretk_core::model::Score) -> Vec<Fraction> {
    score.measures.iter().map(|measure| measure.len).collect()
}
#[test]
fn cadenza_une_seule_mesure() {
    let score =
        lire("\\time 4/4 c'4 d' e' f' \\cadenzaOn c'8 d' e' f' g' a' b' c'' d'' \\cadenzaOff c'1");
    assert_eq!(
        longueurs(&score),
        vec![Fraction::ONE, Fraction::new(9, 8), Fraction::ONE,]
    );
    assert!(score.measures[1].is_irregular());
    assert_eq!(score.measures[1].start_tick, Fraction::ONE);
    assert_eq!(hauteurs(&score).len(), 14);
}
#[test]
fn cadenza_coupee_par_une_barre() {
    // Le `|` nu est un contrôle de mesure ignoré : seule la barre explicite
    // `\bar "|"` coupe la cadence, entre `d'8` et le reste.
    let score = lire("\\time 4/4 c'1 \\cadenzaOn d'8 \\bar \"|\" e'8 f' | g'8 \\cadenzaOff c'1");
    assert_eq!(
        longueurs(&score),
        vec![
            Fraction::ONE,
            Fraction::new(1, 8),
            Fraction::new(3, 8),
            Fraction::ONE,
        ]
    );
    assert_eq!(hauteurs(&score), vec![72, 74, 76, 77, 79, 72]);
}
#[test]
fn cadenza_avec_set_timing() {
    let score = lire(
        "\\time 4/4 c'1 \\set Timing.timing = ##f c'8 d' e' f' g' a' b' c'' d'' \\set Timing.timing = ##t c'1",
    );
    assert_eq!(
        longueurs(&score),
        vec![Fraction::ONE, Fraction::new(9, 8), Fraction::ONE,]
    );
}
#[test]
fn cadenza_sans_commande_ignoree() {
    // Motif de `input/regression/bar-number-after-cadenza.ly` : les deux
    // commandes de cadence et `\caesura` doivent être lues sans avertissement.
    let source = "\\version \"2.25.6\"\n\
                  \\new Score \\fixed c'' { \\repeat unfold 2 { \\cadenzaOn r4 \\cadenzaOff \\caesura c1 } }";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![72, 72]);
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("commande ignorée")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn bar_check_consomme_sans_couper_les_mesures() {
    // `|` est un contrôle de mesure (bar check de `lily/parser.yy` `'|'`) :
    // il est consommé et ignoré, ne crée aucune barre et ne coupe jamais la
    // trame des mesures. Un `|` en fin de source ne doit pas bloquer le
    // lecteur (boucle infinie observée quand le jeton n'était pas consommé).
    let debut = std::time::Instant::now();
    let score = lire("{ c'4 d' e' f' | g'1 | }");
    assert!(
        debut.elapsed() < std::time::Duration::from_secs(1),
        "la lecture d'un bar check ne doit pas boucler"
    );
    assert_eq!(hauteurs(&score), vec![72, 74, 76, 77, 79]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn cadenza_avec_bar_checks_une_seule_mesure_irreguliere() {
    // Les contrôles de mesure `|` ne coupent pas la cadence : toute la
    // cadence reste une seule mesure irrégulière.
    let score = lire("\\time 4/4 \\cadenzaOn c'8 | d'8 | e'8 \\cadenzaOff");
    assert_eq!(hauteurs(&score), vec![72, 74, 76]);
    assert_eq!(longueurs(&score), vec![Fraction::new(3, 8)]);
    assert!(score.measures[0].is_irregular());
}

#[test]
fn cadenza_coupee_par_bar_explicite_deux_mesures() {
    // Seule la barre explicite `\bar "…"` ferme la mesure pendant une
    // cadence ; le `|` nu reste sans effet.
    let score = lire("\\time 4/4 \\cadenzaOn c'8 \\bar \"|\" d'8 \\cadenzaOff");
    assert_eq!(hauteurs(&score), vec![72, 74]);
    assert_eq!(
        longueurs(&score),
        vec![Fraction::new(1, 8), Fraction::new(1, 8)]
    );
}
