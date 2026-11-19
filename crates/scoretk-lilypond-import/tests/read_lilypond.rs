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
    LyricsSyllabic, SegmentItem, SegmentType, StaffId, Track, Transpose,
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

/// Paroles de la piste 0, avec la hauteur de leur accord, le couplet, la
/// syllabisation, le texte et la prolongation, dans l'ordre.
fn paroles(score: &scoretk_core::model::Score) -> Vec<(u8, usize, LyricsSyllabic, String, bool)> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .flat_map(|chord_rest| {
            let pitch = chord_rest
                .chord_ref()
                .and_then(|chord| chord.notes.first().copied())
                .map(|id| score.note(id).pitch)
                .unwrap_or(0);
            chord_rest.lyrics.iter().map(move |lyrics| {
                (
                    pitch,
                    lyrics.verse,
                    lyrics.syllabic,
                    lyrics.text.clone(),
                    lyrics.melisma.is_some(),
                )
            })
        })
        .collect()
}

/// Durées écrites (sans mise à l'échelle) de la piste 0, dans l'ordre.
fn durees_ecrites(score: &scoretk_core::model::Score) -> Vec<Fraction> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .map(|chord_rest| chord_rest.duration.ticks())
        .collect()
}

#[test]
fn relative_et_octaves() {
    let score = lire("\\relative c' { c4 d e f g a b c }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69, 71, 72]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn controle_octave_est_absolu() {
    // `n=''` est un contrôle d'octave (`ly/relative-octave-check.cc`) : la
    // hauteur est imposée dans l'octave marquée, et non décalée de deux
    // octaves depuis la note précédente comme une simple marque `n''`.
    let score = read_lilypond(
        "\\score { \\new Staff { \\relative c' { c4 g'4 a=''4 b4 <a='' d fis a>4 } } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 67, 81, 83, 81, 86, 90, 93]);
    assert_eq!(score.measures.len(), 2);
    // Seules les marques APRÈS `=` fixent l'octave contrôlée : la marque
    // avant `=` appartient à la hauteur relative. `a'='` sonne donc la4
    // (69), comme LilyPond, et non la5.
    let note = lire("\\relative c' { c4 g'4 a'='4 b4 }");
    assert_eq!(hauteurs(&note), vec![60, 67, 69, 71]);
    // Cas réel (Debussy, Arabesque 2) : marques des deux côtés de `=`. La
    // note vérifiée est imposée dans l'octave des marques après `=`, et la
    // note relative suivante repart de cette hauteur.
    let reelles = lire("\\relative c' { a4 a,,,=,4 a,='4 a4 }");
    assert_eq!(hauteurs(&reelles), vec![57, 45, 69, 69]);
    assert_eq!(reelles.measures.len(), 1);
    // Cas non concerné : sans `=`, la marque reste relative.
    let sans = lire("\\relative c' { c4 g'4 a''4 b4 }");
    assert_eq!(hauteurs(&sans), vec![60, 67, 93, 95]);
}

#[test]
fn relative_reference_place_la_premiere_note() {
    // La première note se place, comme les suivantes, à l'octave la plus
    // proche de la référence : `g` après `c''` descend d'une quarte et vaut
    // g' (67), pas g'' (79), y compris à travers une variable musicale.
    let score = lire(
        "phrase = { g8 e f g a b c d }\n\
         \\relative c'' { \\phrase | <c, e g>2 r2 }",
    );
    assert_eq!(
        hauteurs(&score),
        vec![67, 64, 65, 67, 69, 71, 72, 74, 60, 64, 67]
    );
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::HALF,
            Fraction::HALF,
        ]
    );
}

#[test]
fn relative_degre_de_la_reference() {
    // Le degré de la référence compte : `c` après `f'` est c' (quarte
    // descendante), tandis que `g` après `c'` est le sol sous le do central.
    let apres_fa = read_lilypond("\\relative f' { c4 }").expect("lecture");
    assert_eq!(hauteurs(&apres_fa), vec![60]);
    let apres_do = read_lilypond("\\relative c' { g4 }").expect("lecture");
    assert_eq!(hauteurs(&apres_do), vec![55]);
}

#[test]
fn relative_sans_reference_part_de_fa() {
    // Sans hauteur, LilyPond 2.18+ prend `f` sous le do central comme
    // référence (`ly/music-functions-init.ly`, fonction `relative`) : `c''`
    // vaut donc 72, et `c,` après `d''` vaut c' (60).
    let score = read_lilypond("\\relative { c''4 d r2 | c,1 }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![72, 74, 60]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::HALF,
            Fraction::ONE
        ]
    );
}

#[test]
fn relative_premiere_note_deja_juste() {
    // Cas déjà juste : `e` et `g` restent dans l'octave de la référence.
    let score = read_lilypond("\\relative c' { e4~ e g2 }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![64, 64, 67]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(liaisons(&score), vec![(64, 64)]);
}

#[test]
fn relative_accord_reference_premiere_note() {
    // `Event_chord::to_relative_octave` (`lily/music.cc`) place chaque note
    // de l'accord par rapport à la précédente, mais rend la première comme
    // référence pour la suite : `<f d'>` vaut fa' ré'', `<d b'>` ré' si', et
    // le `c` qui suit se place par rapport au ré' de la première note.
    let score = lire("\\relative c' { <f d'>4 <d b'>8 c4 | <c e g>2 r2 }");
    assert_eq!(hauteurs(&score), vec![65, 74, 62, 71, 60, 60, 64, 67]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::HALF,
            Fraction::HALF
        ]
    );
}

#[test]
fn relative_repetition_accord() {
    // `q` répète les hauteurs résolues du dernier accord, pas les marques
    // écrites : les répétitions gardent les mêmes octaves. La référence
    // reste la première note de l'accord répété.
    let score = lire(
        "motif = { <f d'>4 <d b'>8 }\n\
         \\relative c' { \\motif q8 c4 | <c e g>4 q q2 }",
    );
    assert_eq!(
        hauteurs(&score),
        vec![65, 74, 62, 71, 62, 71, 60, 60, 64, 67, 60, 64, 67, 60, 64, 67]
    );
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn accords_en_mode_fixe_inchanges() {
    // Hors `\relative`, l'octave d'un accord ne dépend que des marques : sous
    // `\fixed c'`, le nom non marqué vaut do central, donc `b'` vaut si''
    // (MIDI 83), et la note précédente n'y change rien.
    let score = lire("\\fixed c' { <f d'>4 <d b'> <c e g>2 }");
    assert_eq!(hauteurs(&score), vec![65, 74, 62, 83, 60, 64, 67]);
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn hauteur_hors_plage_avertit() {
    // Une hauteur qui sort de 0..127 est écartée, mais pas en silence.
    let (score, warnings) =
        read_lilypond_reporting("\\fixed c' { c4 c'''''''' }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("hors de la plage MIDI")),
        "{warnings:?}"
    );
}

#[test]
fn unfold_relative_converti_une_seule_fois() {
    // `\repeat` n'a pas de `to-relative-callback` (`lily/music.cc`) : son corps
    // est converti en octaves absolues une seule fois avant d'être rejoué.
    // Relire le corps à chaque passage ferait monter d'une octave les `g''` de
    // la seconde reprise.
    let score = lire(
        "motif = { g''8 e g e }\n\
         \\relative c' { \\repeat unfold 2 { \\motif | <c e g>4 r4 | } c'4 }",
    );
    assert_eq!(
        hauteurs(&score),
        vec![79, 76, 79, 76, 72, 76, 79, 79, 76, 79, 76, 72, 76, 79, 84]
    );
    assert_eq!(score.measures.len(), 3);
    assert_eq!(durees_ecrites(&score).len(), 13);
}

#[test]
fn octave_relative_repetee_sature_sans_panique() {
    // Une suite de hauteurs très marquées fait sortir l'octave relative de la
    // plage MIDI : les notes sont écartées avec un avertissement, sans panique
    // ni débordement silencieux.
    let source = format!("\\relative {{ {} }}", "c''''''4 ".repeat(30));
    let (score, warnings) = read_lilypond_reporting(&source).expect("lecture");
    let pitches = hauteurs(&score);
    assert!(pitches.iter().all(|pitch| *pitch <= 127));
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("hors de la plage MIDI")),
        "avertissements : {warnings:?}"
    );
    // Cas non concerné : en mode fixe, l'octave ne dérive pas.
    let (fixed, fixed_warnings) =
        read_lilypond_reporting("\\fixed c' { c'4 d e f }").expect("lecture");
    assert_eq!(hauteurs(&fixed), vec![72, 62, 64, 65]);
    assert!(
        fixed_warnings.is_empty(),
        "avertissements : {fixed_warnings:?}"
    );
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
    // `\transpose` transpose la musique écrite elle-même : `e4 b c d` devient
    // `fis4 cis4 d4 e4`, en hauteur comme en classe tonale écrite. Il n'y a
    // pas d'instrument transpositeur, donc `tpc` et `tpc_transposed` portent
    // tous deux la hauteur transposée.
    let score = lire("\\transpose c d { \\relative c' { e4 b4 c4 d4 } }");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(hauteurs(&score), vec![66, 61, 62, 64]);
    let tpc: Vec<i8> = score.notes.iter().map(|note| note.tpc).collect();
    assert_eq!(
        tpc,
        vec![
            scoretk_core::model::tpc::F_SHARP,
            scoretk_core::model::tpc::C_SHARP,
            scoretk_core::model::tpc::D,
            scoretk_core::model::tpc::E
        ]
    );
    let tpc_transposed: Vec<i8> = score.notes.iter().map(|note| note.tpc_transposed).collect();
    assert_eq!(tpc_transposed, tpc);
    let durees: Vec<Fraction> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .map(|chord_rest| chord_rest.duration.ticks())
        .collect();
    assert_eq!(durees, vec![Fraction::QUARTER; 4]);
}

/// Source réduite de la voix Piccolo de `Ad_te_Suspiramus.ly` : clef octave,
/// `\relative` dans une variable, barres de contrôle, accord, silence et
/// liaison, avec ou sans `\transposition`.
fn source_transposition(transposition: &str) -> String {
    format!(
        "piccolo = \\relative d'' {{\n\
         \\clef \"treble^8\" \\numericTimeSignature \\time 4/4 \\key bes \\major\n\
         {transposition} | %% 1\n\
         r2 r4 r8 d8 | %% 2\n\
         g8 [ a16 bes16 ] a8 [ g8 ] c8 bes4 a8 | %% 3\n\
         d,8 [ g8 ] <c' e'>4 a8~ | %% 4\n\
         a2 r2 \\bar \"|.\"\n\
         }}\n\
         \\score {{ \\new Staff {{ \\piccolo }} }}\n"
    )
}

#[test]
fn transposition_ne_consomme_pas_une_note_de_la_musique() {
    // `\transposition` est une fonction musicale à un argument `ly:pitch?`
    // (`ly/music-functions-init.ly`) : sa hauteur n'est pas une note. MusicXML
    // garde la hauteur écrite, sans transposition de jeu. L'argument parasite
    // ajoutait une note et faussait tout l'état `\relative` d'une octave.
    let avec = read_lilypond(&source_transposition("\\transposition c'")).expect("lecture");
    let sans = read_lilypond(&source_transposition("")).expect("lecture");
    let attendu = vec![74, 79, 81, 82, 81, 79, 84, 82, 81, 74, 79, 96, 112, 93, 93];
    // Cas non concerné : sans `\transposition`, la musique reste identique.
    assert_eq!(hauteurs(&sans), attendu);
    assert_eq!(hauteurs(&avec), attendu);
    assert_eq!(avec.measures.len(), 4);
    assert_eq!(
        durees_ecrites(&avec),
        vec![
            Fraction::HALF,
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::HALF,
            Fraction::HALF,
        ]
    );
}

#[test]
fn transposition_de_staff_decale_la_concert() {
    // Portage de `instrumentTransposition` (`ly/engraver-init.ly`) : la valeur
    // est la hauteur que joue l'instrument quand il lit un do central écrit.
    // `\transposition a'` fait donc sonner une sixte majeure plus haut, sans
    // changer les hauteurs écrites : c'est l'instrument qui porte l'intervalle.
    let score = read_lilypond(
        "\\version \"2.24.0\"\n\
         \\score { \\new Staff { \\transposition a'\n\
         \\relative c' { c4 d e f | <c e g>2 g2 | } } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![69, 71, 73, 74, 69, 73, 76, 64]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        score.parts[0].instrument.transpose,
        Some(Transpose::new(5, 9))
    );
    // Cas non concerné : sans `\transposition`, la concert vaut l'écrite.
    let sans =
        read_lilypond("\\score { \\new Staff { \\relative c' { c4 d e f | <c e g>2 g2 | } } }")
            .expect("lecture");
    assert_eq!(hauteurs(&sans), vec![60, 62, 64, 65, 60, 64, 67, 55]);
    assert_eq!(sans.parts[0].instrument.transpose, None);

    // Cas réel : les orchestres réunissent plusieurs portées dans un même
    // StaffGroup, donc une seule partie MusicXML. On ne peut poser qu'un
    // `<transpose>` pour toutes les portées : décaler la concert de la portée
    // transpositrice ferait écrire des hauteurs fausses à l'export. On garde
    // donc les hauteurs ÉCRITES de chaque portée, comme sans `\transposition`.
    let orchestre = read_lilypond(
        "\\score { << \\new StaffGroup << \
         \\new Staff { \\transposition a' \\relative c' { c4 d e f } } \
         \\new Staff { \\relative c' { c4 d e f } } >> >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&orchestre), vec![60, 62, 64, 65, 60, 62, 64, 65]);
    assert_eq!(orchestre.measures.len(), 1);
    assert_eq!(orchestre.parts[0].instrument.transpose, None);
}

#[test]
fn transposition_dans_le_bloc_midi() {
    // `\midi { \transposition c }` (guitare écrite une octave au-dessus) ne
    // contient aucune musique, mais décale d'une octave la hauteur concert.
    let score = read_lilypond(
        "\\version \"2.24.0\"\n\
         \\score { \\new Staff { \\relative c' { c4 d e f | <c e g>2 g2 | } }\n\
         \\midi { \\transposition c } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![48, 50, 52, 53, 48, 52, 55, 43]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        score.parts[0].instrument.transpose,
        Some(Transpose::new(-7, -12))
    );
    // Cas réel : un duo de guitares. Le `\transposition` du bloc `\midi` vaut
    // pour toutes les portées : il décale la concert de chacune dès
    // l'interprétation, avant même que la structure des parties soit connue.
    let duo = read_lilypond(
        "\\score { << \\new Staff { \\relative c' { c4 d e f } } \
         \\new Staff { \\relative c' { c4 d e f } } >> \
         \\midi { \\transposition c } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&duo), vec![48, 50, 52, 53, 48, 50, 52, 53]);
    assert_eq!(duo.measures.len(), 1);
    assert_eq!(duo.parts.len(), 2);
    assert_eq!(
        duo.parts[0].instrument.transpose,
        Some(Transpose::new(-7, -12))
    );
    // Cas non concerné : le même bloc `\midi` sans transposition ne décale rien.
    let sans = read_lilypond(
        "\\score { \\new Staff { \\relative c' { c4 d e f } } \\midi { \\tempo 4=60 } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&sans), vec![60, 62, 64, 65]);
    assert_eq!(sans.parts[0].instrument.transpose, None);
}

#[test]
fn transposition_argument_absent_ou_different() {
    // Cas limites : un argument qui n'est pas `c'` est consommé lui aussi, et
    // un `\transposition` sans argument en fin de musique ne doit ni ajouter de
    // note ni bloquer l'analyse (toute boucle consomme un jeton par tour).
    let different =
        read_lilypond("\\relative c' { \\transposition bes, c4 d4 e4 f4 | }").expect("lecture");
    // `bes,` fait sonner un si bémol une octave sous le do central : les
    // hauteurs écrites restent c d e f, les hauteurs concert montent de 14
    // demi-tons.
    assert_eq!(hauteurs(&different), vec![46, 48, 50, 51]);
    assert_eq!(different.measures.len(), 1);

    let absent = read_lilypond("\\relative c' { c4 d4 \\transposition }").expect("lecture");
    assert_eq!(hauteurs(&absent), vec![60, 62]);
    assert_eq!(absent.measures.len(), 1);
}

/// Quintes des armures du score, dans l'ordre où elles apparaissent.
fn armures(score: &scoretk_core::model::Score) -> Vec<i8> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter(|segment| segment.kind == SegmentType::KeySig)
        .filter_map(|segment| match segment.element(Track::from_raw(0)) {
            Some(SegmentItem::KeySig(key)) => Some(key.transposed_key.fifths()),
            _ => None,
        })
        .collect()
}

#[test]
fn armure_transposee() {
    // `\key` est transposé lui aussi : do majeur sous `\transpose c d` devient
    // ré majeur (2 dièses), et des transpositions imbriquées se composent.
    let score = lire(
        "\\transpose c d { \\key c \\major c4 d4 e4 f4 | \\transpose c d { \\key c \\major g4 } }",
    );
    assert_eq!(armures(&score), vec![2, 4]);
    assert_eq!(hauteurs(&score), vec![62, 64, 66, 67, 71]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn notes_et_armure_sans_transposition() {
    // Sans `\transpose`, rien ne change : mêmes hauteurs, mêmes classes
    // tonales et armure de do majeur.
    let score = lire("\\key c \\major e4 b4 c4 d4");
    assert_eq!(hauteurs(&score), vec![64, 71, 60, 62]);
    let tpc: Vec<i8> = score.notes.iter().map(|note| note.tpc).collect();
    assert_eq!(
        tpc,
        vec![
            scoretk_core::model::tpc::E,
            scoretk_core::model::tpc::B,
            scoretk_core::model::tpc::C,
            scoretk_core::model::tpc::D
        ]
    );
    let tpc_transposed: Vec<i8> = score.notes.iter().map(|note| note.tpc_transposed).collect();
    assert_eq!(tpc_transposed, tpc);
    assert_eq!(armures(&score), vec![0]);
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
fn post_event_rest_transforme_la_note_en_silence() {
    // `c4\rest` est un silence nommé, pas une note de hauteur c : LilyPond ne
    // l'entend pas dans le MIDI. Le post-événement s'applique à la note
    // précédente et ne doit pas décaler les notes qui suivent.
    let source = "\\score { \\fixed c' { c4\\rest d e f | g\\rest a b c' } }";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![62, 64, 65, 69, 71, 72]);
    assert_eq!(score.measures.len(), 2);
    assert!(!warnings.iter().any(|w| w.message.contains("rest")));
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
fn context_staff_ouvre_une_portee() {
    // Deux `\context Staff = "…"` frères dans un `<< … >>` de partition sont
    // deux portées, pas deux voix d'une même portée (c'est l'idiome des
    // partitions d'orchestre Mutopia).
    let source = "\\score { << \
                  \\context Staff = \"a\" { \\fixed c' { c4 d e f } } \
                  \\context Staff = \"b\" { \\fixed c' { g4 a b c' } } >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 2);
    assert_eq!(score.staves.len(), 2);
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69, 71, 72]);
}

#[test]
fn context_staff_au_dela_de_quatre_voix() {
    // Cinq `\context Staff` ne doivent pas se faire écraser par la limite de
    // quatre voix par portée : chacun ouvre sa propre portée.
    let mut branches = String::new();
    for (i, note) in ['c', 'd', 'e', 'f', 'g'].iter().enumerate() {
        branches.push_str(&format!(
            "\\context Staff = \"s{i}\" {{ \\fixed c' {{ {note}1 }} }} "
        ));
    }
    let source = format!("\\score {{ << {branches} >> }}");
    let score = read_lilypond(&source).expect("lecture");
    assert_eq!(score.staves.len(), 5);
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
}

#[test]
fn context_staff_imbrique_avec_voix_nommees() {
    // `\context Staff` dans une portée déjà ouverte retrouve cette portée :
    // ses voix doivent être lues, pas ignorées comme un contexte imbriqué.
    let source = "\\score { \\new Staff { \\fixed c' { c1 \
                  \\context Staff << \
                    \\context Voice = \"v0\" { d1 } \
                    \\context Voice = \"v1\" { e1 } \
                  >> f1 } } }";
    let score = read_lilypond(source).expect("lecture");
    let mut notes = hauteurs(&score);
    notes.sort_unstable();
    assert_eq!(notes, vec![60, 62, 64, 65]);
}

#[test]
fn silence_avec_hauteur_avance_la_reference_relative() {
    // `event_chord` de `lily/parser.yy` : `\rest` change la note en
    // `RestEvent`, mais sa hauteur écrite est résolue et avance la référence
    // `\relative` comme une note. Sur le passage de bwv651.ly
    // `d'2\rest d16\rest g, c e g c, e g`, ne pas résoudre les silences
    // placerait `g,` une octave trop bas (43 au lieu de 55). Vérifié contre
    // le MIDI produit par LilyPond 2.27.
    let source = concat!(
        "motif = { g4 r4 r2 R1 d'2\\rest d16\\rest g, c e g c, e g | <c e g>1~ | c'1 } ",
        "\\score { \\relative f' { c8 b a4 g2~ | \\motif } }"
    );
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 6);
    assert_eq!(
        hauteurs(&score),
        vec![60, 59, 57, 55, 55, 55, 60, 64, 67, 60, 64, 67, 72, 76, 79, 84]
    );
}

#[test]
fn context_staff_nomme_avec_voix_surnumeraires() {
    // Un `\context Staff = "…"` nommé dont le contenu porte plus de quatre
    // voix nommées ne doit pas les écraser sur la limite du modèle : chaque
    // voix surnuméraire ouvre une portée sœur (pathetique-1 de Mutopia, où
    // chaque main enchaîne onze sections à plusieurs voix).
    let source = concat!(
        "\\score { \\context PianoStaff << ",
        "\\context Staff = \"rh\" { \\fixed c' { c1 ",
        "\\context Voice = \"a\" { e1 } \\context Voice = \"b\" { g1 } ",
        "\\context Voice = \"c\" { c'1 } \\context Voice = \"d\" { e'1 } } } ",
        "\\context Staff = \"lh\" { \\fixed c' { f1 ",
        "\\context Voice = \"e\" { a1 } \\context Voice = \"f\" { c'1 } ",
        "\\context Voice = \"g\" { f'1 } \\context Voice = \"h\" { a'1 } } } ",
        ">> }"
    );
    let score = read_lilypond(source).expect("lecture");
    assert!(score.staves.len() >= 2);
    let mut notes = hauteurs(&score);
    notes.sort_unstable();
    assert_eq!(notes, vec![60, 64, 65, 67, 69, 72, 72, 76, 77, 81]);
}

#[test]
fn context_staff_nomme_garde_les_voix_sur_la_portee_englobante() {
    // Un `\context Staff = "nom"` dont le contenu porte deux voix nommées
    // (chacune dans un `\context Staff \relative` de variable, comme les
    // `\links`/`\pedal` de w10-h-moll-cfi) reste une seule portée. Répartir
    // ces voix en portées sœurs réapplique les enveloppes `\relative` voix par
    // voix : les octaves et le nombre de portées changent sans raison.
    let source = concat!(
        "global = { s4 s2. } ",
        "rechts = \\context Staff \\relative c' { fis'4 ais2 | cis'4 d h | } ",
        "links = \\context Staff \\relative c { ",
        "\\context Voice = \"links\" r4 h <fis' h d> <fis h d> } ",
        "pedal = \\context Staff \\relative c, { ",
        "\\context Voice = \"pedal\" s4 s2. } ",
        "\\score { \\context PianoStaff << ",
        "\\context Staff = \"up\" << \\global \\rechts >> ",
        "\\context Staff = \"down\" << \\global \\links \\pedal >> ",
        ">> }"
    );
    let score = read_lilypond(source).expect("lecture");
    // Deux portées seulement : une par `\context Staff = "…"` du score.
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, Fraction::new(1, 1));
    assert_eq!(score.measures[1].len, Fraction::new(1, 1));
    let mut notes = hauteurs(&score);
    notes.sort_unstable();
    assert_eq!(notes, vec![62, 62, 66, 66, 78, 82, 97, 98]);

    // Cas non concerné : une seule voix nommée ne déclenche pas non plus la
    // répartition, et garde ses hauteurs et sa mesure.
    let seul = read_lilypond(concat!(
        "global = { s4 s2. } ",
        "links = \\context Staff \\relative c { ",
        "\\context Voice = \"links\" r4 h <fis' h d> <fis h d> } ",
        "\\score { \\context Staff = \"down\" << \\global \\links >> }"
    ))
    .expect("lecture");
    assert_eq!(seul.staves.len(), 1);
    assert_eq!(seul.measures.len(), 1);
    let mut notes = hauteurs(&seul);
    notes.sort_unstable();
    assert_eq!(notes, vec![62, 62, 66, 66]);
}

#[test]
fn context_staff_avec_relative_garde_la_reference_du_bloc() {
    // `\context Staff \relative << … \\ … >>` : la référence relative porte
    // sur tout le bloc polyphonique (collisions.ly, `\hairyChord`). Ouvrir une
    // portée fille la ferait résoudre voix par voix et décalerait `fis,` de
    // deux octaves (66 attendu, 42 obtenu avant correction).
    let source = concat!(
        "hairy = \\context Staff \\relative << e''1 \\\\ fis,1 >> ",
        "suite = \\fixed c' { c1 d1 } ",
        "\\score { \\hairy | \\suite }"
    );
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    let mut notes = hauteurs(&score);
    notes.sort_unstable();
    assert_eq!(notes, vec![60, 62, 66, 76]);

    // Cas limite : le bloc polyphonique en fin de source, sans musique après.
    let seul = read_lilypond(concat!(
        "hairy = \\context Staff \\relative << e''1 \\\\ fis,1 >> ",
        "\\score { \\hairy }"
    ))
    .expect("lecture");
    assert_eq!(seul.measures.len(), 1);
    let mut notes = hauteurs(&seul);
    notes.sort_unstable();
    assert_eq!(notes, vec![66, 76]);
}

#[test]
fn noms_de_contextes_insensibles_a_la_casse() {
    // Les partitions Mutopia anciennes écrivent `\new voice` (2.8.0) ou
    // `\context staff` en minuscules : LilyPond les accepte, le lecteur aussi.
    let source = "\\score { \
                  \\new staff { \
                    << \\new voice { \\fixed c' { c4 d e f } } \
                       \\new voice { \\fixed c' { g4 a b c' } } >> \
                  } \
                  \\context staff = \"bas\" { \\fixed c' { c,4 e, g, } } \
                  }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.staves.len(), 2);
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65, 67, 69, 71, 72, 48, 52, 55]
    );
}

#[test]
fn musique_de_haut_niveau_new_staff_relative() {
    let score = read_lilypond("\\new Staff \\relative { c'4 d e f }").expect("lecture");
    assert_eq!(score.staves.len(), 1);
    // Sans référence, LilyPond prend `f` sous le do central comme référence :
    // `c'` reste au do central (60).
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
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
    // Sans référence, LilyPond prend `f` sous le do central comme référence :
    // `c'` reste au do central (60).
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
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
    // Deux expressions musicales de haut niveau forment deux partitions, comme
    // LilyPond (`{ c'4 d' } { e'4 f' }` → deux fichiers MIDI) : seule la
    // première est retenue, la seconde est signalée.
    let (score, warnings) = read_lilypond_reporting("{ c'4 d' }\n{ e'4 f' }").expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
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
    // regression/trill-spanner-consecutive.ly : quatre appels de haut niveau,
    // donc quatre partitions ; seule la première est retenue, les autres sont
    // ignorées avec un avertissement.
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
    assert_eq!(score.notes.len(), 2);
    let ignorees = warnings
        .iter()
        .filter(|warning| warning.message == "partition supplémentaire ignorée")
        .count();
    assert_eq!(ignorees, 3, "avertissements : {warnings:?}");
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
fn fonction_signature_ancienne_nom_de_fonction() {
    // LilyPond 2.8 écrit la signature `(nom location …)` : `nom` et `location`
    // ne sont pas des paramètres. Les confondre faisait consommer la musique
    // qui suit l'appel comme arguments et rejouer la mesure suivante à la
    // place de l'argument musical.
    let score = read_lilypond(
        "\\version \"2.24.0\"\n\
         foo = #(define-music-function (foo location str music) (string? ly:music?) #{ $music #})\n\
         \\score { \\relative c' { <a d fis>8 d,16 d' fis, d' |\n\
         \\foo #\"x\" { a,16 cis' a' cis, } a,,4 |\n\
         <a' cis g'>4 } }",
    )
    .expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![57, 62, 66, 50, 62, 54, 62, 45, 61, 69, 61, 33, 45, 49, 55]
    );
    assert_eq!(score.measures.len(), 2);
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
    // La récursion est bornée ; l'appel fautif ne produit rien et la musique
    // qui suit est lue normalement, dans la même partition.
    let (score, warnings) = read_lilypond_reporting(
        "loop = #(define-music-function (m) (ly:music?) #{ \\loop $m #})\n\
         { \\loop { c'4 } d'4 }\n",
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

/// Lit un bloc `\score { … }` complet, langue anglaise, écrit à la main.
fn lire_score(corps: &str) -> scoretk_core::model::Score {
    let source = format!("\\version \"2.24.0\"\n\\language \"english\"\n{corps}\n");
    read_lilypond(&source).unwrap_or_else(|error| panic!("lecture : {error}"))
}

#[test]
fn transpose_autour_d_un_contexte() {
    // `\transpose` prend une musique quelconque, contextes compris : la
    // structure de portées est celle du contenu et la transposition
    // s'applique à toutes ses voix. En notation absolue, `c` vaut une octave
    // sous le do central (48) et `\transpose c d` donne donc `[50, 52]`.
    let score = lire_score("\\score { \\transpose c d \\new Staff { c4 d } }");
    assert_eq!(hauteurs(&score), vec![50, 52]);
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(durees_ecrites(&score), vec![Fraction::QUARTER; 2]);

    let score = lire_score("\\score { \\transpose c d << \\new Staff { c4 d } >> }");
    assert_eq!(hauteurs(&score), vec![50, 52]);
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);

    // Les voix d'une portée sont transposées elles aussi.
    let score = lire_score(
        "\\score { \\new Staff \\transpose c d << \\new Voice { c'4 } \\new Voice { e'4 } >> }",
    );
    assert_eq!(hauteurs(&score), vec![62, 66]);
    assert_eq!(score.staves.len(), 1);
}

#[test]
fn relative_fixed_et_variable_autour_d_un_contexte() {
    // Même descente pour les fonctions d'octave : `\relative` et `\fixed`
    // s'appliquent aux voix de la portée qu'ils enveloppent.
    let score = lire_score("\\score { \\relative c' \\new Staff { c4 d e f } }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(durees_ecrites(&score), vec![Fraction::QUARTER; 4]);

    let score = lire_score("\\score { \\fixed c' \\new Staff { c4 d } }");
    assert_eq!(hauteurs(&score), vec![60, 62]);

    // Une variable qui porte la fonction et son contexte se déplie de la
    // même façon dans `\score`.
    let score = lire_score("melodie = \\transpose c d \\new Staff { c4 d }\n\\score { \\melodie }");
    assert_eq!(hauteurs(&score), vec![50, 52]);
    assert_eq!(score.staves.len(), 1);
}

#[test]
fn unfold_repeats_transparent_devant_un_contexte() {
    // `\unfoldRepeats` ne change pas la musique lue : il est transparent et
    // laisse passer les contextes qu'il enveloppe.
    let score = lire_score("\\score { \\unfoldRepeats \\new Staff { c4 d } }");
    assert_eq!(hauteurs(&score), vec![48, 50]);
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);

    let score = lire_score("\\score { \\unfoldRepeats \\transpose c d \\new Staff { c4 d } }");
    assert_eq!(hauteurs(&score), vec![50, 52]);
    assert_eq!(score.staves.len(), 1);
}

#[test]
fn transpose_autour_d_un_piano_staff() {
    // Un groupe de portées garde ses deux portées, chacune transposée, et
    // forme une seule partie.
    let score = lire_score(
        "\\score { \\transpose c d \\new PianoStaff << \\new Staff { c'4 } \\new Staff { \\clef bass c4 } >> }",
    );
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.parts.len(), 1);
    assert_eq!(hauteurs(&score), vec![62, 50]);
    assert_eq!(score.measures.len(), 1);

    // Musique sans contexte : une seule portée anonyme, transposée, barre de
    // contrôle comprise ; rien ne change pour elle.
    let score = lire_score("\\score { \\transpose c d { c4 d | e4 f } }");
    assert_eq!(hauteurs(&score), vec![50, 52, 54, 55]);
    assert_eq!(score.staves.len(), 1);
}

#[test]
fn transpose_traversant_articulate_et_unfold_repeats() {
    // Partition MIDI écrite par `musicxml2ly` (`Angels_Carol.ly`) :
    // `\transpose d c \unfoldRepeats \articulate << … >>`. `\articulate` est
    // une fonction musicale de `articulate.ly` (`define-music-function (music)
    // (ly:music?)`) : elle prend la musique suivante en argument et ne change
    // pas les hauteurs. La transposition doit donc porter sur toutes les
    // portées ; perdue, elle décale toute la partition d'un ton.
    let source = r#"
      \version "2.24.0"
      \language "english"
      \include "articulate.ly"
      refrain = \relative c' { c4 d4 | e2 ~ e4 f4 }
      \score {
        \transpose c d
        \unfoldRepeats \articulate
        <<
          \new Staff { \refrain | <c e g>2 r2 }
          \new Staff { \fixed c' { c,2 g,2 | c2 r2 } }
        >>
      }
    "#;
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    // `\transpose c d` monte d'un ton : do4→ré4, mi4→fa♯4, do2→ré2.
    assert_eq!(
        hauteurs(&score),
        vec![62, 64, 66, 66, 67, 50, 54, 57, 50, 57, 62]
    );
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.measures.len(), 3);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::HALF,
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::HALF,
            Fraction::HALF
        ],
        "durées de la première portée"
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("commande ignorée")),
        "avertissements : {warnings:?}"
    );

    // Cas non concerné : sans transposition, `\articulate` ne change rien.
    let score =
        lire_score("\\score { \\unfoldRepeats \\articulate \\new Staff { \\fixed c' { c4 d } } }");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    // Cas limite : `\articulate` en fin de musique, sans argument musical.
    let score =
        lire_score("\\score { \\transpose c d \\new Staff { \\fixed c' { c4 \\articulate } } }");
    assert_eq!(hauteurs(&score), vec![62]);

    // Cas réel réduit : `musicxml2ly` écrit une partition imprimée (`\layout`,
    // pas de transposition) puis la partition MIDI (`\midi`, transposée).
    // Le MusicXML représente la partition GRAVÉE : c'est le premier `\score`
    // gravé qui fait foi, même si LilyPond ne joue que la seconde. Que la
    // référence MIDI diffère ici est une limite de l'outil de mesure.
    let source = r#"
      \version "2.24.0"
      \language "english"
      \include "articulate.ly"
      \book {
        \score { \transpose c c \layout {} \new Staff { \fixed c' { c4 d4 } } }
        \score { \transpose c d \unfoldRepeats \articulate
                 \new Staff { \fixed c' { c4 d4 } } \midi {} }
      }
    "#;
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::QUARTER, Fraction::QUARTER]
    );
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "partition supplémentaire ignorée")
            .count(),
        1
    );

    // Même arbitrage avec des fonctions musicales : la partition gravée
    // (`\transpose d d`, donc non transposée) l'emporte sur la partition MIDI
    // (`\transpose d c`, d'un ton plus bas) : hauteurs non transposées.
    let source = r#"
      \version "2.24.0"
      \language "english"
      m = \fixed c' { c4 d4 }
      \book {
        \score { \transpose d d \m \layout {} }
        \score { \transpose d c \m \midi {} }
      }
    "#;
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "partition supplémentaire ignorée")
            .count(),
        1
    );
}

/// Accords d'une voie (piste brute) : (tick absolu, hauteurs, durée écrite).
fn accords_de_voix(
    score: &scoretk_core::model::Score,
    voix: u32,
) -> Vec<(Fraction, Vec<u8>, Fraction)> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            let chord_rest = segment.chord_rest(Track::from_raw(voix))?;
            matches!(&chord_rest.kind, ChordRestKind::Chord(_))
                .then_some((segment.tick, chord_rest))
        })
        .map(|(tick, chord_rest)| {
            let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                unreachable!("accord filtré ci-dessus");
            };
            (
                tick,
                chord.notes.iter().map(|&id| score.note(id).pitch).collect(),
                chord_rest.duration.ticks(),
            )
        })
        .collect()
}

/// Nombre de voix (pistes brutes) portant au moins un accord.
fn nombre_de_voix(score: &scoretk_core::model::Score) -> usize {
    (0..4)
        .filter(|&voix| !accords_de_voix(score, voix).is_empty())
        .count()
}

#[test]
fn voix_ouverte_dans_une_musique_sequentielle() {
    // `\new Voice` dans une séquence ouvre une voix de la portée à la position
    // courante (`context_prefix` / NEWCONTEXT, `lily/parser.yy` l. 1685-1690).
    let score = lire("\\new Voice { c4 d }");
    assert_eq!(nombre_de_voix(&score), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(score.measures.len(), 1);

    // La musique du contexte ouvert fait avancer la séquence : `e` vient après
    // le `d` de la voix nouvelle, à 2/4, comme LilyPond.
    let score = lire("c4 \\new Voice { d4 } e4");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 2), vec![64], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![(Fraction::new(1, 4), vec![62], Fraction::new(1, 4))]
    );
    assert_eq!(score.measures.len(), 1);

    // `\context Voice` retrouve ou ouvre un contexte existant : même effet.
    let score = lire("\\context Voice = \"x\" { c4 d }");
    assert_eq!(nombre_de_voix(&score), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
}

#[test]
fn voix_nommees_dans_une_simultanee() {
    // Les deux branches d'une simultanée démarrent au même tick, même quand
    // elles ouvrent des contextes nommés (`<< { \new Voice … } … >>`).
    let score = lire("<< { \\new Voice = \"s\" { c4 d } } \\new Voice = \"a\" { e4 f } >>");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::ZERO, vec![64], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![65], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(score.measures.len(), 1);

    // Forme de niveau portée, déjà lue avant ce correctif : inchangée.
    let score = read_lilypond(
        "\\score { \\new Staff << \\new Voice = \"x\" { c4 d } \\new Voice = \"y\" { e4 f } >> }",
    )
    .expect("lecture");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::ZERO, vec![52], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![53], Fraction::new(1, 4)),
        ]
    );
}

#[test]
fn polyphonie_temporaire_avec_double_barre() {
    // `<< … \\ … >>` : passage polyphonique temporaire, une voix par branche,
    // la musique reprend ensuite dans la voix d'origine.
    let score = lire("<< { c4 d } \\\\ { e4 f } >>");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::ZERO, vec![64], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![65], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn simultane_sans_contexte_une_seule_portee() {
    // Un `<< … >>` de `\score` sans aucun `\new` est une musique simultanée
    // d'une seule portée anonyme, comme un `{ … }` : les deux éléments y sont
    // deux voix, pas deux portées.
    let score = lire_score("\\score { << { c'4 } { e'4 } >> }");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 64]);
}

#[test]
fn contexte_et_musique_nue_dans_le_meme_simultane() {
    // Dès qu'un `\new` structure le `<< … >>`, les éléments sans contexte
    // restants sont réunis en une seule portée anonyme.
    let score = lire_score("a = \\new Staff { d'1 }\n\\score { << \\a { c'1 } >> }");
    assert_eq!(score.staves.len(), 2);
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    assert_eq!(hauteurs, vec![60, 62]);
}

#[test]
fn variable_portant_un_contexte_dans_un_simultane() {
    // L'appel d'une variable rend la musique de contexte (`Music::New`) :
    // `<< \p >>` doit produire une portée, avec ou sans `\with`.
    let score = lire_score(
        "fl = { c'4 d' }\np = \\new Staff \\with { midiInstrument = \"flute\" } \\fl\n\\score { << \\p >> }",
    );
    assert_eq!(score.staves.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 62]);

    let score = lire_score("fl = { c'4 d' }\np = \\new Staff \\fl\n\\score { << \\p >> }");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 62]);
}

#[test]
fn deux_variables_de_contexte_donnent_deux_portees() {
    let score =
        lire_score("a = \\new Staff { c'1 }\nb = \\new Staff { d'1 }\n\\score { << \\a \\b >> }");
    assert_eq!(score.staves.len(), 2);
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    assert_eq!(hauteurs, vec![60, 62]);
}

#[test]
fn variable_sans_musique_utilisable_avertit() {
    // Une commande inconnue stockée dans une variable ne doit pas disparaître
    // en silence au point d'appel ; une musique vide, elle, est normale.
    let source = "\\version \"2.24.0\"\n\\language \"english\"\n\
                  a = \\nimportequoi\n\\score { \\a }";
    let (_, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("variable \\a")),
        "avertissements : {warnings:?}"
    );

    let source = "\\version \"2.24.0\"\n\\language \"english\"\n\
                  a = \\fermata\n\\score { \\a }";
    let (_, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("variable \\a")),
        "avertissements : {warnings:?}"
    );
}

#[test]
fn lyricsto_voix_nommee_dans_score() {
    // `\new Lyrics \lyricsto "s"` frère de la voix dans un `<< … >>` de
    // `\score` : le bloc de paroles n'a pas de portée, il doit pourtant
    // s'attacher à la voix nommée. `--` donne begin/end.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f } } \
         \\new Lyrics \\lyricsto \"s\" { la li -- lo lu } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "la".to_owned(), false),
            (62, 0, LyricsSyllabic::Begin, "li".to_owned(), false),
            (64, 0, LyricsSyllabic::End, "lo".to_owned(), false),
            (65, 0, LyricsSyllabic::Single, "lu".to_owned(), false),
        ]
    );
}

#[test]
fn lyricsto_musique_dans_variable() {
    let score = read_lilypond(
        "m = { \\fixed c' { c4 d e f } } \
         \\score { << \\new Voice = \"s\" \\m \
         \\new Lyrics \\lyricsto \"s\" { do re mi fa } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
    let textes: Vec<String> = paroles(&score)
        .into_iter()
        .map(|(_, _, _, text, _)| text)
        .collect();
    assert_eq!(textes, vec!["do", "re", "mi", "fa"]);
}

#[test]
fn lyricsto_paroles_dans_variable() {
    let score = read_lilypond(
        "v = \\lyricmode { la li lo lu } \
         \\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f } } \
         \\new Lyrics \\lyricsto \"s\" \\v >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
    let textes: Vec<String> = paroles(&score)
        .into_iter()
        .map(|(_, _, _, text, _)| text)
        .collect();
    assert_eq!(textes, vec!["la", "li", "lo", "lu"]);
}

#[test]
fn lyricsto_couplets_avec_stanza_et_variables() {
    // Deux couplets sur la même voix, numérotés par `\set stanza`, avec des
    // paroles définies dans des variables `\lyricmode` (motif courant des
    // fichiers réels). Les deux couplets partagent les mêmes hauteurs.
    let score = read_lilypond(
        "vA = \\lyricmode { la li lo lu } \
         vB = \\lyricmode { do re mi fa } \
         \\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f } } \
         \\new Lyrics \\lyricsto \"s\" { \\set stanza = \"1.\" \\vA } \
         \\new Lyrics \\lyricsto \"s\" { \\set stanza = \"2.\" \\vB } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "la".to_owned(), false),
            (60, 1, LyricsSyllabic::Single, "do".to_owned(), false),
            (62, 0, LyricsSyllabic::Single, "li".to_owned(), false),
            (62, 1, LyricsSyllabic::Single, "re".to_owned(), false),
            (64, 0, LyricsSyllabic::Single, "lo".to_owned(), false),
            (64, 1, LyricsSyllabic::Single, "mi".to_owned(), false),
            (65, 0, LyricsSyllabic::Single, "lu".to_owned(), false),
            (65, 1, LyricsSyllabic::Single, "fa".to_owned(), false),
        ]
    );
}

#[test]
fn lyricsto_couplets_sans_stanza_sont_numérotés() {
    // Sans `\set stanza`, les blocs successifs sur la même voix reçoivent les
    // numéros 1, 2, 3 (attribut `number` de `<lyric>`), comme dans LilyPond.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f } } \
         \\new Lyrics \\lyricsto \"s\" { la li lo lu } \
         \\new Lyrics \\lyricsto \"s\" { do re mi fa } >> }",
    )
    .expect("lecture");
    let vers: Vec<usize> = paroles(&score)
        .into_iter()
        .map(|(_, verse, _, _, _)| verse)
        .collect();
    // Les paroles sont rangées par accord : chaque accord porte les deux
    // couplets, donc les vers s'alternent.
    assert_eq!(vers, vec![0, 1, 0, 1, 0, 1, 0, 1]);
}

#[test]
fn lyricsto_saute_liaisons_et_prolonge_melisme() {
    // `~` (fin de liaison) et `__` (mélisme) ne consomment pas de syllabe ;
    // `--` donne begin/end. Les notes liées ou sous `__` n'en reçoivent pas.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4~ d4 e4 f4 g4 } } \
         \\new Lyrics \\lyricsto \"s\" { la li -- lo __ mi } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "la".to_owned(), false),
            (64, 0, LyricsSyllabic::Begin, "li".to_owned(), false),
            (65, 0, LyricsSyllabic::End, "lo".to_owned(), true),
        ]
    );
}

#[test]
fn lyricsto_chaines_markup_et_elision() {
    // Une chaîne et un `\markup` simple servent de syllabes ; `~` d'élision
    // est conservé dans le texte, comme le fait l'export MusicXML.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f } } \
         \\new Lyrics \\lyricsto \"s\" { \"chaîne\" \\markup { \"suite\" } at~taque } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    let textes: Vec<String> = paroles(&score)
        .into_iter()
        .map(|(_, _, _, text, _)| text)
        .collect();
    assert_eq!(textes, vec!["chaîne", "suite", "at~", "taque"]);
}

#[test]
fn lyrics_sans_lyricsto_alignment_par_tick() {
    // `\new Lyrics { la2 li2 … }` sans `\lyricsto` : les durées explicites
    // alignent les syllabes sur les ticks de la voix de la portée précédente.
    let score = read_lilypond(
        "\\score { << \\new Staff { \\fixed c' { \\time 4/4 c2 d2 | e4 f4 g4 a4 } } \
         \\new Lyrics { la2 li2 do4 re4 mi4 fa4 } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "la".to_owned(), false),
            (62, 0, LyricsSyllabic::Single, "li".to_owned(), false),
            (64, 0, LyricsSyllabic::Single, "do".to_owned(), false),
            (65, 0, LyricsSyllabic::Single, "re".to_owned(), false),
            (67, 0, LyricsSyllabic::Single, "mi".to_owned(), false),
            (69, 0, LyricsSyllabic::Single, "fa".to_owned(), false),
        ]
    );
}

#[test]
fn lyricsto_saute_les_notes_sous_liaison() {
    // `melismaBusyProperties` contient `slurMelismaBusy` par défaut : la note
    // qui ouvre la liaison reçoit une syllabe, celles qu'elle couvre jusqu'à
    // la note de fermeture comprise n'en reçoivent pas.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4( d e) f } } \
         \\new Lyrics \\lyricsto \"s\" { la li } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    let places: Vec<(u8, String)> = paroles(&score)
        .into_iter()
        .map(|(pitch, _, _, text, _)| (pitch, text))
        .collect();
    assert_eq!(places, vec![(60, "la".to_owned()), (65, "li".to_owned())]);
}

#[test]
fn lyricsto_avec_melisma_busy_properties_vide() {
    // `\set melismaBusyProperties = #'()` : plus aucune note n'est sautée,
    // même sous une liaison d'expression ou après une liaison `~`.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\set melismaBusyProperties = #'() \
         \\fixed c' { c4( d e) f } } \
         \\new Lyrics \\lyricsto \"s\" { la li lo lu } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    let places: Vec<(u8, String)> = paroles(&score)
        .into_iter()
        .map(|(pitch, _, _, text, _)| (pitch, text))
        .collect();
    assert_eq!(
        places,
        vec![
            (60, "la".to_owned()),
            (62, "li".to_owned()),
            (64, "lo".to_owned()),
            (65, "lu".to_owned()),
        ]
    );

    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\set melismaBusyProperties = #'() \
         \\fixed c' { c4~ d4 e4 } } \
         \\new Lyrics \\lyricsto \"s\" { la li lo } >> }",
    )
    .expect("lecture");
    let places: Vec<(u8, String)> = paroles(&score)
        .into_iter()
        .map(|(pitch, _, _, text, _)| (pitch, text))
        .collect();
    assert_eq!(
        places,
        vec![
            (60, "la".to_owned()),
            (62, "li".to_owned()),
            (64, "lo".to_owned()),
        ]
    );
}

#[test]
fn lyricsto_voix_ouverte_dans_une_sequence() {
    // `\new Voice = "s"` ouvert au milieu d'une séquence : la voix est créée
    // à la position courante, et `\lyricsto "s"` doit la retrouver comme une
    // voix déclarée dans un `<< … >>` de `\score`.
    let score = read_lilypond(
        "\\score { << \\new Staff { \\fixed c' { c4 \\new Voice = \"s\" { d4 e4 f4 } g4 } } \
         \\new Lyrics \\lyricsto \"s\" { do re mi } >> }",
    )
    .expect("lecture");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::ONE, vec![67], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
            (Fraction::new(1, 2), vec![64], Fraction::new(1, 4)),
            (Fraction::new(3, 4), vec![65], Fraction::new(1, 4)),
        ]
    );
    let textes: Vec<(u8, String)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(1)))
        .flat_map(|chord_rest| {
            let pitch = chord_rest
                .chord_ref()
                .and_then(|chord| chord.notes.first().copied())
                .map(|id| score.note(id).pitch)
                .unwrap_or(0);
            chord_rest
                .lyrics
                .iter()
                .map(move |lyrics| (pitch, lyrics.text.clone()))
        })
        .collect();
    assert_eq!(
        textes,
        vec![
            (62, "do".to_owned()),
            (64, "re".to_owned()),
            (65, "mi".to_owned()),
        ]
    );
}

#[test]
fn polyphonie_temporaire_dans_une_sequence() {
    let score = lire("c4 << { d e } \\\\ { f g } >> a");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
            (Fraction::new(1, 2), vec![64], Fraction::new(1, 4)),
            (Fraction::new(3, 4), vec![69], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 4), vec![65], Fraction::new(1, 4)),
            (Fraction::new(1, 2), vec![67], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::ONE);

    // Cas limite : contextualisation vide, sans note, ne doit ni paniquer ni
    // décaler la suite.
    let score = lire("c4 \\new Voice { } d4");
    assert_eq!(nombre_de_voix(&score), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
}

#[test]
fn octave_relative_apres_une_polyphonie_temporaire() {
    // Source vérifiée contre LilyPond 2.27 : la musique qui suit un
    // `<< … \\ … >>` part de la dernière note de la **dernière** branche.
    // `Music_sequence::simultaneous_relative_callback` parcourt les branches
    // dans l'ordre (`lily/music-sequence.cc`), chacune relativement à la
    // précédente ; partir de la première décalait tout d'une octave.
    let score = lire("\\relative c' { c4 << { c8 } { e,8 } >> f4 }");
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![60], Fraction::new(1, 8)),
            (Fraction::new(3, 8), vec![53], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![(Fraction::new(1, 4), vec![52], Fraction::new(1, 8))]
    );

    // Les branches s'enchaînent : la seconde part de la dernière note de la
    // première, et la continuation de la dernière note de la seconde.
    let score = lire("\\relative c' { c4 << { c'8 d } \\\\ { c8 c } >> f4 }");
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![72], Fraction::new(1, 8)),
            (Fraction::new(3, 8), vec![74], Fraction::new(1, 8)),
            (Fraction::new(1, 2), vec![77], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 4), vec![72], Fraction::new(1, 8)),
            (Fraction::new(3, 8), vec![72], Fraction::new(1, 8)),
        ]
    );
}

#[test]
fn octave_relative_apres_un_contexte_ouvert() {
    // Un `\new Voice` de la séquence est un élément comme un autre :
    // la musique qui suit part de sa dernière note
    // (`generic_to_relative_octave`, `lily/music.cc`), d'où le `f` à l'octave 5.
    let score = lire("\\relative c' { c4 \\new Voice { c'8 } f4 }");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(3, 8), vec![77], Fraction::new(1, 4)),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![(Fraction::new(1, 4), vec![72], Fraction::new(1, 8))]
    );
}

#[test]
fn passages_polyphoniques_successifs_partagent_une_voix() {
    // Deux passages `<< … \\ … >>` successifs ne doivent pas ouvrir deux
    // voix temporaires de plus : MuseScore n'en connaît que quatre et les
    // passages, disjoints dans le temps, tiennent dans la même piste.
    let score = lire("c4 << { d8 e } \\\\ { f8 g } >> a4 << { b8 c' } \\\\ { d8 e } >> f4");
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(1, 4), vec![62], Fraction::new(1, 8)),
            (Fraction::new(3, 8), vec![64], Fraction::new(1, 8)),
            (Fraction::new(1, 2), vec![69], Fraction::new(1, 4)),
            (Fraction::new(3, 4), vec![71], Fraction::new(1, 8)),
            (Fraction::new(7, 8), vec![72], Fraction::new(1, 8)),
            (Fraction::ONE, vec![65], Fraction::new(1, 4)),
        ]
    );
    // Les deux passages tiennent dans la même voix temporaire.
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 4), vec![65], Fraction::new(1, 8)),
            (Fraction::new(3, 8), vec![67], Fraction::new(1, 8)),
            (Fraction::new(3, 4), vec![62], Fraction::new(1, 8)),
            (Fraction::new(7, 8), vec![64], Fraction::new(1, 8)),
        ]
    );
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn sequence_dans_un_groupe_de_score() {
    // Un `{ … }` peut contenir des contextes : LilyPond les crée à la
    // position courante, comme dans `<< … >>`. Le `\new ChoirStaff` de
    // `\score` reçoit ici une séquence qui n'enveloppe qu'un simultané de
    // deux portées : elle ne doit plus passer pour une portée anonyme vide.
    let score = lire_score(
        "\\score { \\new ChoirStaff { << \\new Staff { c4 d } \\new Staff { e4 f } >> } }",
    );
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![48], Fraction::QUARTER),
            (Fraction::QUARTER, vec![50], Fraction::QUARTER),
        ]
    );
    // Portée 2, voix 0 : `Track::new(1, 0)`.
    assert_eq!(
        accords_de_voix(&score, Track::new(StaffId::from_index(1), 0).raw()),
        vec![
            (Fraction::ZERO, vec![52], Fraction::QUARTER),
            (Fraction::QUARTER, vec![53], Fraction::QUARTER),
        ]
    );
}

#[test]
fn musique_de_haut_niveau_dans_une_sequence() {
    // En musique de haut niveau, `\score { { \new Staff … } }` ne fait pas
    // une portée anonyme : le contexte du `{ … }` donne la structure. Même
    // chose quand une variable porte le contexte.
    let score = lire_score("\\score { { \\new Staff { c4 d } } }");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![48], Fraction::QUARTER),
            (Fraction::QUARTER, vec![50], Fraction::QUARTER),
        ]
    );

    // Une variable peut porter l'enveloppe et le contexte : `\relative`
    // s'applique alors à la voix de la portée, accord et silence compris.
    let score = lire_score("v = \\relative c' \\new Staff { <c e g>4 r d }\n\\score { { \\v } }");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 64, 67, 62]);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60, 64, 67], Fraction::QUARTER),
            (Fraction::new(1, 2), vec![62], Fraction::QUARTER),
        ]
    );
}

#[test]
fn sequence_de_groupe_dans_une_variable() {
    // Le groupe est enfoui dans une variable, sous un `{ … }` puis sous un
    // `<< … >>` : la descente traverse les deux sans perdre les portées.
    let score = lire_score(
        "p = { << \\new StaffGroup << \\new Staff { c4 d } \\new Staff { e f } >> >> }\n\\
         \\score { \\p }",
    );
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![48], Fraction::QUARTER),
            (Fraction::QUARTER, vec![50], Fraction::QUARTER),
        ]
    );
    // Portée 2, voix 0 : `Track::new(1, 0)`.
    assert_eq!(
        accords_de_voix(&score, Track::new(StaffId::from_index(1), 0).raw()),
        vec![
            (Fraction::ZERO, vec![52], Fraction::QUARTER),
            (Fraction::QUARTER, vec![53], Fraction::QUARTER),
        ]
    );
}

#[test]
fn contextes_multiples_dans_une_sequence() {
    // Plusieurs contextes dans un même `{ … }` sont rares et sans ordre
    // séquentiel naturel : ils deviennent des portées parallèles, avec un
    // avertissement. La musique nue qui les accompagne reste une portée
    // anonyme, et un `{ … }` sans aucun contexte ne change pas.
    let (score, warnings) = read_lilypond_reporting(
        "\\version \"2.24.0\"\n\\language \"english\"\n\
         v = { \\new Staff { c4 d } \\new Staff { e f } }\n\
         \\score { { c'8 \\v } }",
    )
    .expect("lecture");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("plusieurs contextes")),
        "avertissements : {warnings:?}"
    );
    assert_eq!(score.staves.len(), 3);
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    assert_eq!(hauteurs, vec![48, 50, 52, 53, 60]);

    // Un `{ … }` sans contexte ne change pas : une portée anonyme, la barre
    // de contrôle est consommée comme avant.
    let score = lire_score("\\score { { c4 d | e4 f } }");
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![48], Fraction::QUARTER),
            (Fraction::QUARTER, vec![50], Fraction::QUARTER),
            (Fraction::new(1, 2), vec![52], Fraction::QUARTER),
            (Fraction::new(3, 4), vec![53], Fraction::QUARTER),
        ]
    );
}

#[test]
fn paroles_mot_decoupe_a_la_blanche() {
    // La règle `<lyrics>` de `lily/lexer.ll` découpe les paroles aux blancs :
    // `Ah_!` est **une** syllabe dont le `_` devient une espace
    // (`lyric_fudge`), comme l'apostrophe de `qu'il` et la ponctuation collée
    // de `bon,` / `gne.`. `--` entre blancs reste la césure de mot.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"v\" { \\fixed c' { c4 d e f g a b c' d' } } \
         \\new Lyrics \\lyricsto \"v\" { Ah_! qu'il fait bon, sur la mon -- ta -- gne. } >> }",
    )
    .expect("lecture");
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "Ah !".to_owned(), false),
            (62, 0, LyricsSyllabic::Single, "qu'il".to_owned(), false),
            (64, 0, LyricsSyllabic::Single, "fait".to_owned(), false),
            (65, 0, LyricsSyllabic::Single, "bon,".to_owned(), false),
            (67, 0, LyricsSyllabic::Single, "sur".to_owned(), false),
            (69, 0, LyricsSyllabic::Single, "la".to_owned(), false),
            (71, 0, LyricsSyllabic::Begin, "mon".to_owned(), false),
            (72, 0, LyricsSyllabic::Middle, "ta".to_owned(), false),
            (74, 0, LyricsSyllabic::End, "gne.".to_owned(), false),
        ]
    );
}

/// Hauteurs de chaque accord d'une piste, dans l'ordre.
fn hauteurs_par_accord(score: &scoretk_core::model::Score, piste: u32) -> Vec<Vec<u8>> {
    accords_de_voix(score, piste)
        .into_iter()
        .map(|(_, hauteurs, _)| hauteurs)
        .collect()
}

/// Durées écrites de chaque accord d'une piste, dans l'ordre.
fn durees_par_accord(score: &scoretk_core::model::Score, piste: u32) -> Vec<Fraction> {
    accords_de_voix(score, piste)
        .into_iter()
        .map(|(_, _, duree)| duree)
        .collect()
}

#[test]
fn parallel_music_repartit_les_mesures_entre_les_voix() {
    // `\parallelMusic` (`ly/music-functions-init.ly`) découpe la musique aux
    // contrôles de mesure `|` et répartit les tranches circulairement entre
    // les variables nommées : ici une barre par ligne, quatre voix, le motif
    // d'un piano à deux portées et deux voix chacune.
    let score = read_lilypond(
        r#"\version "2.16"
\language "français"
MetriqueArmure = {
  \time 2/4
  \key sol \major
}
\parallelMusic #'(PianoI PianoII PianoIII PianoIV) {
  %Bar1
  re8\mf  |
  s8      |
  s8      |
  re8     |

  %Bar2
  sol8.  sol16  sol8  si  |
  si4           re8   re  |
  re4           re8   sol |
  sol,4         si8   sol |

  %Bar3
  la8.   sol16  la8   si   |
  re4.                fad8 |
  fad8.  mi16   fad8  fad  |
  re'4.               si8  |
}
\score {
  \new PianoStaff <<
    \new Staff <<
      \MetriqueArmure
      \new Voice { \voiceOne \relative do' { \PianoI } }
      \new Voice { \voiceTwo \relative do' { \PianoII } }
    >>
    \new Staff <<
      \MetriqueArmure
      \clef bass
      \new Voice { \voiceOne \relative do { \PianoIII } }
      \new Voice { \voiceTwo \relative do { \PianoIV } }
    >>
  >>
}"#,
    )
    .expect("lecture");
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.measures.len(), 3);
    assert_eq!(
        hauteurs_par_accord(&score, 0),
        vec![
            vec![62],
            vec![67],
            vec![67],
            vec![67],
            vec![71],
            vec![69],
            vec![67],
            vec![69],
            vec![71],
        ]
    );
    assert_eq!(
        durees_par_accord(&score, 0),
        vec![
            Fraction::new(1, 8),
            Fraction::new(3, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(3, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
        ]
    );
    assert_eq!(
        hauteurs_par_accord(&score, 1),
        vec![vec![59], vec![62], vec![62], vec![62], vec![66]]
    );
    assert_eq!(
        hauteurs_par_accord(&score, 4),
        vec![
            vec![50],
            vec![50],
            vec![55],
            vec![54],
            vec![52],
            vec![54],
            vec![54]
        ]
    );
    assert_eq!(
        hauteurs_par_accord(&score, 5),
        vec![vec![50], vec![43], vec![47], vec![43], vec![50], vec![47]]
    );
    assert_eq!(
        durees_par_accord(&score, 5),
        vec![
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(3, 8),
            Fraction::new(1, 8),
        ]
    );
}

#[test]
fn paroles_souligne_isole_saute_la_note() {
    // Seul un `_` **isolé** entre blancs saute une note (mélisme) ; `__` fait
    // de même. Collé à un mot, `_` est une espace (`fin_` → « fin ») et
    // n'allonge plus la syllabe.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"s\" { \\fixed c' { c4 d e f g a } } \
         \\new Lyrics \\lyricsto \"s\" { la _ mon __ bre fin_ } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        paroles(&score),
        vec![
            (60, 0, LyricsSyllabic::Single, "la".to_owned(), true),
            (64, 0, LyricsSyllabic::Single, "mon".to_owned(), true),
            (67, 0, LyricsSyllabic::Single, "bre".to_owned(), false),
            (69, 0, LyricsSyllabic::Single, "fin".to_owned(), false),
        ]
    );
}

#[test]
fn paroles_guillemets_typographiques_et_apostrophe() {
    // Guillemets et apostrophe typographiques font partie du mot, sans le
    // couper : `«Ah»` et `l’ami` sont une syllabe chacun.
    let score = read_lilypond(
        "\\score { << \\new Voice = \"v\" { \\fixed c' { c4 d e } } \
         \\new Lyrics \\lyricsto \"v\" { «Ah» l’ami c’est } >> }",
    )
    .expect("lecture");
    assert_eq!(score.measures.len(), 1);
    let textes: Vec<String> = paroles(&score)
        .into_iter()
        .map(|(_, _, syllabic, text, _)| {
            assert_eq!(syllabic, LyricsSyllabic::Single);
            text
        })
        .collect();
    assert_eq!(textes, vec!["«Ah»", "l’ami", "c’est"]);
}

#[test]
fn parallel_music_forme_a_virgules_et_sans_barre() {
    // Syntaxe à virgules de LilyPond 2.24 (`\parallelMusic haut,bas`), deux
    // voix d'une même portée : quatre barres donnent deux mesures par voix.
    let score = lire_score(
        "\\parallelMusic haut,bas {\n\
           c'4 d' |\n\
           e'8 f' |\n\
           g'4 a' |\n\
           b'8 c'' |\n\
         }\n\
         \\score { \\new Staff << \\time 2/4 \\haut \\\\ \\bas >> }",
    );
    assert_eq!(score.staves.len(), 1);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        hauteurs_par_accord(&score, 0),
        vec![vec![60], vec![62], vec![67], vec![69]]
    );
    assert_eq!(
        hauteurs_par_accord(&score, 1),
        vec![vec![64], vec![65], vec![71], vec![72]]
    );

    // Sans aucun contrôle de mesure, LilyPond ignore la musique avec un
    // avertissement (« ignoring parallel music without bar checks ») ; la
    // portée ordinaire du fichier, elle, reste inchangée.
    let (score, warnings) = read_lilypond_reporting(
        "\\parallelMusic a,b { c'4 d' }\n\\score { \\new Staff { c'4 d' e' f' } }",
    )
    .expect("lecture");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("sans contrôle de mesure")),
        "avertissements : {warnings:?}"
    );
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn durees_nues_repetent_la_note_ou_l_accord_precedent() {
    // Une durée seule (`8`, `4`) après une note ou un accord vaut une
    // répétition de l'élément précédent, comme `q` (lily/parser.yy).
    let score = lire("\\time 4/4 c8 8 d4 4 r4 | <e g>8 8 <f a>4 4 | c2 2");
    assert_eq!(
        hauteurs(&score),
        vec![60, 60, 62, 62, 64, 67, 64, 67, 65, 69, 65, 69, 60, 60]
    );
    assert_eq!(score.measures.len(), 3);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::HALF,
            Fraction::HALF,
        ]
    );

    // Cas non concerné : `q` et les durées données restent inchangés.
    let score = lire("\\time 4/4 c4 q8 q8 d4 e4");
    assert_eq!(hauteurs(&score), vec![60, 60, 60, 62, 64]);
}

#[test]
fn repeat_unfold_entre_guillemets_est_deplie() {
    // `\repeat "unfold" 2 { … }` (forme chaîne des partitions anciennes) se
    // déplie comme `\repeat unfold 2` ; `\repeat volta`, lui, reste écrit une
    // seule fois dans le MusicXML.
    let score = lire(
        "\\time 4/4 \\repeat \"unfold\" 2 { c4 d e f } | g2 a2 | \
         \\repeat volta 2 { b4 c' d' e' } | f'2 g'2",
    );
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65, 60, 62, 64, 65, 67, 69, 71, 72, 74, 76, 77, 79]
    );
    assert_eq!(score.measures.len(), 5);
    assert_eq!(score.measures[3].start_bar_line, BarLineType::StartRepeat);
    assert_eq!(score.measures[3].repeat_count, 2);
}

#[test]
fn paroles_avec_lyrics_imbrique_gardent_les_portees() {
    // Un `\new Lyrics` à l'intérieur d'un bloc de paroles (second couplet
    // aligné) ne doit pas avaler la portée suivante ni ses notes.
    let score = lire_score(
        "\\score { << \
           \\new Staff { \\clef treble \\new Voice = \"s\" { c'4 d' e' f' } } \
           \\new Lyrics = \"l\" \\lyricsto \"s\" { \
             do re << mi \\new Lyrics \\with { alignBelowContext = \"l\" } \
             { fa sol } >> la } \
           \\new Staff { \\clef bass \\new Voice = \"b\" { c4 d e f } } \
         >> }",
    );
    assert_eq!(score.staves.len(), 2);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 48, 50, 52, 53]);
    let couplets = paroles(&score);
    let premiers: Vec<&str> = couplets
        .iter()
        .filter(|(_, verse, ..)| *verse == 0)
        .map(|(_, _, _, text, _)| text.as_str())
        .collect();
    assert_eq!(premiers, vec!["do", "re", "mi", "la"]);
    let seconds: Vec<&str> = couplets
        .iter()
        .filter(|(_, verse, ..)| *verse == 1)
        .map(|(_, _, _, text, _)| text.as_str())
        .collect();
    assert_eq!(seconds, vec!["fa", "sol"]);
}

#[test]
fn tempo_pointe_ne_fait_pas_perdre_le_time_suivant() {
    // `\tempo 4.=50` laisse le point et le nombre `50` dans le flux musical.
    // Un entier qui n'est pas une durée doit consommer son seul jeton :
    // l'avancer une seconde fois avalerait `\time` et sa mesure.
    let score = lire("\\tempo 4.=50 \\time 6/8 c4 d4 e4 f4");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(score.measures.len(), 2);
}

#[test]
fn partcombine_deux_voix_simultanees() {
    // `\partcombine` (ancien nom de `\partCombine`, `ly/music-functions-init.ly`)
    // place ses deux arguments dans deux voix simultanées de la portée, et non
    // l'un à la suite de l'autre. Les deux voix gardent leurs accords, leur
    // liaison et leur silence, dans la mesure ordinaire qui suit des barres.
    let score = lire_score(
        "\\score { \\new Staff { \\fixed c' { \
           c4 d e f | g2 a2 | \
           \\partcombine { b4 <c' e'>2~ d'4 } { g4 a2 b4 } | \
           d'2 c'2 | \
         } } }",
    );
    assert_eq!(score.measures.len(), 4);
    assert_eq!(nombre_de_voix(&score), 2);
    let quart = Fraction::QUARTER;
    let blanche = Fraction::HALF;
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::new(0, 1), vec![60], quart),
            (Fraction::new(1, 4), vec![62], quart),
            (Fraction::new(2, 4), vec![64], quart),
            (Fraction::new(3, 4), vec![65], quart),
            (Fraction::new(1, 1), vec![67], blanche),
            (Fraction::new(3, 2), vec![69], blanche),
            (Fraction::new(2, 1), vec![71], quart),
            (Fraction::new(9, 4), vec![72, 76], blanche),
            (Fraction::new(11, 4), vec![74], quart),
            (Fraction::new(3, 1), vec![74], blanche),
            (Fraction::new(7, 2), vec![72], blanche),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(2, 1), vec![67], quart),
            (Fraction::new(9, 4), vec![69], blanche),
            (Fraction::new(11, 4), vec![71], quart),
        ]
    );
}

#[test]
fn partcombine_chord_range_et_simultanee_ordinaire() {
    // Cas limite : l'argument optionnel `#'(0 . 8)` (portée des accords) est
    // consommé, et `\partCombine` en fin de flux garde ses deux voix.
    let score = lire_score(
        "\\score { \\new Staff { \\fixed c' { \
           c4 d e f | \\partCombine #'(0 . 8) { g4 a } { b4 c' } \
         } } }",
    );
    assert_eq!(score.measures.len(), 2);
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::new(0, 1), vec![60], Fraction::QUARTER),
            (Fraction::new(1, 4), vec![62], Fraction::QUARTER),
            (Fraction::new(2, 4), vec![64], Fraction::QUARTER),
            (Fraction::new(3, 4), vec![65], Fraction::QUARTER),
            (Fraction::new(1, 1), vec![67], Fraction::QUARTER),
            (Fraction::new(5, 4), vec![69], Fraction::QUARTER),
        ]
    );
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 1), vec![71], Fraction::QUARTER),
            (Fraction::new(5, 4), vec![72], Fraction::QUARTER),
        ]
    );
    // Cas non concerné : une simultanée ordinaire reste inchangée.
    let score = lire_score(
        "\\score { \\new Staff { \\fixed c' { \
           c4 d | << { e4 f } \\\\ { g4 a } >> | b4 c' \
         } } }",
    );
    assert_eq!(score.measures.len(), 2);
    assert_eq!(nombre_de_voix(&score), 2);
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(1, 2), vec![67], Fraction::QUARTER),
            (Fraction::new(3, 4), vec![69], Fraction::QUARTER),
        ]
    );
}

#[test]
fn unset_ne_mange_pas_la_note_qui_suit() {
    // Un chemin de propriété s'arrête avant un nom de note : `f1` après
    // `\unset melismaBusyProperties` est une note relative, pas un morceau du
    // chemin. L'avaler décalait l'octave relative de toute la suite.
    let score = lire("\\relative c' { \\unset melismaBusyProperties f1 | bf2 a }");
    assert_eq!(hauteurs(&score), vec![65, 70, 69]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::ONE, Fraction::HALF, Fraction::HALF]
    );

    // Cas non concerné : `\set` porte bien son `=`, et un `\unset` dont le
    // chemin contient des mots non musicaux laisse la note suivante intacte.
    let score = lire("\\relative c' { \\set melismaBusyProperties = #'() f1 | bf2 a }");
    assert_eq!(hauteurs(&score), vec![65, 70, 69]);
    let score = lire("\\relative c' { \\unset Staff.clef f1 | bf2 a }");
    assert_eq!(hauteurs(&score), vec![65, 70, 69]);
}

#[test]
fn partcombine_sous_relative_garde_les_hauteurs_absolues() {
    // Régression `part-combine-relative.ly` : `make-non-relative-music`
    // (`ly/music-functions-init.ly`) protège les deux parties de
    // `\partCombine`. Un `\relative` placé *autour* de la commande est donc
    // ignoré, et les trois mesures du test sont identiques : E4/C4 puis F4/D4.
    let score = lire_score(
        "\\score { \\new Staff { \
           \\partCombine \\absolute { e'2 f' } { c'2 d' } \
           \\relative \\partCombine { e'2 f' } { c'2 d' } \
           \\partCombine \\relative { e'2 f } \\relative { c'2 d } \
         } } }",
    );
    assert_eq!(score.measures.len(), 3);
    assert_eq!(nombre_de_voix(&score), 2);
    let blanche = Fraction::HALF;
    // Voix « one » : E4 F4, trois fois de suite.
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::new(0, 1), vec![64], blanche),
            (Fraction::new(1, 2), vec![65], blanche),
            (Fraction::new(1, 1), vec![64], blanche),
            (Fraction::new(3, 2), vec![65], blanche),
            (Fraction::new(2, 1), vec![64], blanche),
            (Fraction::new(5, 2), vec![65], blanche),
        ]
    );
    // Voix « two » : C4 D4, trois fois de suite.
    assert_eq!(
        accords_de_voix(&score, 1),
        vec![
            (Fraction::new(0, 1), vec![60], blanche),
            (Fraction::new(1, 2), vec![62], blanche),
            (Fraction::new(1, 1), vec![60], blanche),
            (Fraction::new(3, 2), vec![62], blanche),
            (Fraction::new(2, 1), vec![60], blanche),
            (Fraction::new(5, 2), vec![62], blanche),
        ]
    );

    // Cas limite : même avec une hauteur explicite, `\relative c''` autour de
    // `\partCombine` ne doit pas décaler les hauteurs écrites des deux parties.
    let score = lire_score(
        "\\score { \\new Staff { \\relative c'' { \
           \\partCombine { e'2 f' } { c'2 d' } \
         } } }",
    );
    assert_eq!(score.measures.len(), 1);
    assert_eq!(accords_de_voix(&score, 0)[0].1, vec![64]);
    assert_eq!(accords_de_voix(&score, 0)[1].1, vec![65]);
    assert_eq!(accords_de_voix(&score, 1)[0].1, vec![60]);
    assert_eq!(accords_de_voix(&score, 1)[1].1, vec![62]);

    // Cas non concerné : `\relative` à l'intérieur de chaque partie reste
    // applicable (motif réel des fichiers qui définissent les voix en
    // variables `\relative do' { … }` avant `\partcombine`).
    let score = lire_score(
        "\\score { \\new Staff { \
           \\partCombine \\relative c' { e2 f } \\relative c' { c2 d } \
         } } }",
    );
    assert_eq!(score.measures.len(), 1);
    assert_eq!(accords_de_voix(&score, 0)[0].1, vec![64]);
    assert_eq!(accords_de_voix(&score, 0)[1].1, vec![65]);
    assert_eq!(accords_de_voix(&score, 1)[0].1, vec![60]);
    assert_eq!(accords_de_voix(&score, 1)[1].1, vec![62]);
}

#[test]
fn markup_sans_accolades_ne_mange_pas_la_musique() {
    // `\sectionLabel \markup \tiny "…"` n'a pas d'accolade : le texte du
    // markup s'arrête à la musique suivante, qui doit rester lisible
    // (régressions `caesura-style-*`, `bar-line-stuff-override-*`).
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         staffMusic = \\fixed c' {\n\
           \\sectionLabel \\markup \\tiny \"No Art.\"\n\
           c4 d e f\n\
         }\n\
         \\score { << \\new Staff \\staffMusic >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65]);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4); 4],
        "durées"
    );
    assert_eq!(score.measures.len(), 1, "mesures");
}

#[test]
fn grille_daccords_a_cote_dune_melodie() {
    // `\chords` décrit une grille d'accords (contexte ChordNames) : LilyPond
    // ne la joue pas dans le MIDI. Seule la mélodie doit produire des notes,
    // même si la grille vient en premier (régressions `AuClairDeLaLune`,
    // `HavaNagila` : notes d'accompagnement en trop).
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score { <<\n\
           \\chords { c:7 d:m }\n\
           \\new Staff { c'4 d'4 }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62], "notes de la mélodie seule");
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4); 2],
        "durées de la mélodie"
    );
    assert_eq!(score.measures.len(), 1, "mesures");

    // `\simultaneous` n'est pas une grille : il reste de la musique.
    let source = "\\version \"2.24.0\"\n\\simultaneous { c'4 d'4 e'4 }";
    let score = read_lilypond(source).expect("lecture partition implicite");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert_eq!(score.measures.len(), 1, "mesures implicites");
}

#[test]
fn grilles_non_jouees_seules_et_contextes() {
    // Une grille comme seule musique ne doit pas faire échouer le fichier :
    // partition vide avec avertissement (cas limite, fin de fichier sans
    // argument après `\chordmode`).
    let (score, warnings) = read_lilypond_reporting("\\version \"2.24.0\"\n\\chords { c:7 d:m }")
        .expect("grille seule");
    assert!(hauteurs(&score).is_empty(), "aucune note de grille");
    assert!(
        warnings.iter().any(|w| w.message.contains("grille")),
        "avertissement sur la grille ignorée : {warnings:?}"
    );
    let score = read_lilypond("\\version \"2.24.0\"\n\\chordmode").expect("chordmode seul en fin");
    assert!(hauteurs(&score).is_empty(), "aucune note en fin de fichier");

    // `\figures`, `\new ChordNames` et `\new FiguredBass` ne jouent pas
    // davantage à côté d'une mélodie avec barre de contrôle.
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score { <<\n\
           \\new ChordNames { c1 d1 }\n\
           \\figures { e1 f1 }\n\
           \\new FiguredBass { g1 a1 }\n\
           \\new Staff { c'4 d'4 | e'4 f'4 }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65], "mélodie seule");
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4); 4],
        "durées de la mélodie"
    );
    assert_eq!(score.measures.len(), 1, "mesures");
}

#[test]
fn raccourcis_sans_musique_ne_mangent_pas_la_suite() {
    // `\stemNeutral` est un alias de `\revert Stem.direction`
    // (`ly/property-init.ly`) et `\etc` marque une fonction musicale
    // partielle (`REPEAT … partial_function`, `lily/parser.yy`) : ni l'un ni
    // l'autre ne doit avaler la musique qui suit (régressions
    // `grace-music-override`, `repeat-etc`).
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         stopGraceMusic = \\stemNeutral\n\
         { \\grace a32 f4 \\grace a32 f4 }";
    let score = read_lilypond(source).expect("lecture stemNeutral");
    // Les appoggiatures sont stockées avant la note qu'elles ornent ;
    // l'octave par défaut est la troisième (`a` = 57, `f` = 53).
    assert_eq!(hauteurs(&score), vec![53, 57, 53, 57]);
    assert_eq!(score.measures.len(), 1, "mesures");

    let source = "\\version \"2.24.0\"\n\
         testPercent = \\repeat percent \\etc\n\
         { \\testPercent c'4 d'4 }";
    let score = read_lilypond(source).expect("lecture etc");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4), Fraction::new(1, 4)],
        "durées"
    );
    assert_eq!(score.measures.len(), 1, "mesures");
}

#[test]
fn markup_sans_accolades_s_arrete_devant_une_fonction_musicale() {
    // `\markup "0.50:" \example 0.50` : la chaîne appartient au markup, mais
    // `\example` est une fonction musicale appelée juste après. Le markup ne
    // doit pas l'avaler avec la suite (régression `magnifyStaff-space-alist`).
    let source = "\\version \"2.24.0\"\n\
         example = #(define-music-function (mag) (positive?) #{ c'4 d'4 #})\n\
         \\markup \"0.50:\" \\example 0.50";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62]);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4), Fraction::new(1, 4)],
        "durées"
    );
    assert_eq!(score.measures.len(), 1, "mesures");
}

#[test]
fn parser_include_string_injecte_la_musique() {
    // `#(ly:parser-include-string "…")` inclut son argument comme un fichier
    // (`lily/lexer.ll`) : la musique injectée doit être lue, pas ignorée
    // (régression `include-string`).
    let source = "\\version \"2.19.22\"\n\
         #(ly:parser-include-string \"\\relative { a4 b c d }\")";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![57, 59, 60, 62]);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4); 4],
        "durées"
    );
    assert_eq!(score.measures.len(), 1, "mesures");
}

#[test]
fn include_ignore_sans_partition_ne_fait_pas_echouer() {
    // Les gabarits `satb.*`/`ssaattbb.*` mettent leur `\score` dans le
    // fichier livré avec LilyPond (`satb.ly`, `ssaattbb.ly`), que le
    // sous-ensemble n'inclut pas. Ce n'est pas une erreur de syntaxe : la
    // lecture renvoie une partition vide avec un avertissement.
    let source = "\\version \"2.19.19\"\n\
         SopranoMusic = \\relative { c''4 c c4 c }\n\
         \\include \"satb.ly\"";
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert!(score.measures.is_empty(), "partition vide");
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("aucune partition lisible")),
        "avertissements : {warnings:?}"
    );

    // Une source réellement sans musique reste une erreur.
    let error = read_lilypond("\\version \"2.24.0\"\n\\header { title = \"x\" }")
        .expect_err("aucune musique");
    assert!(error.to_string().contains("aucune musique"));
}

#[test]
fn markup_avec_argument_entre_accolades_laisse_la_musique_suivante() {
    // `\markup \column { … }` (valeur de variable) et `\markup \bold { … }`
    // (dans une voix) ont un argument entre accolades : le bloc doit être
    // sauté d'un seul tenant. Sinon il devient une musique parasite, qui
    // décale les octaves de la suite ou se fait passer pour la partition
    // implicite (régressions `duration-line-start-stop`,
    // `satb-template-with-keyboard-reduction`, `franck_ave_maria`).
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         etiquette = \\markup \\column { \"haut\" { \"bas\" } }\n\
         musique = \\relative c' {\n\
           \\markup \\bold {a Tempo}\n\
           c4 ^\\markup \\tiny \"Dolce\" d | e2 r |\n\
           <c e g>2 c,4 | c1\n\
         }\n\
         \\score { <<\n\
           \\new Staff \\musique\n\
           \\new Staff { \\fixed c' { c'4 d' | e'2 r | } }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    // Voix 0 : do4 ré4 | mi2 silence | accord do-mi-sol2 do3 | do3,
    // puis les trois notes de la seconde portée (do5 ré5 mi5).
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 60, 64, 67, 48, 48, 72, 74, 76]
    );
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 4),
            Fraction::new(1, 1),
        ],
        "durées de la première portée"
    );
    assert_eq!(score.measures.len(), 4, "mesures");

    // Un markup dont l'argument est une chaîne, sans accolade, garde son
    // comportement : la musique qui suit n'est pas avalée.
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score { \\new Staff { c'4 ^\\markup \\tiny \"No Art.\" d'4 e'2 } }";
    let score = read_lilypond(source).expect("lecture chaîne");
    assert_eq!(hauteurs(&score), vec![60, 62, 64], "musique après chaîne");
    assert_eq!(score.measures.len(), 1, "mesures après chaîne");
}

#[test]
fn commande_sans_argument_ne_veure_pas_le_bloc_musique_suivant() {
    // Régressions Mutopia (Greis, Deutschlandlied, ...) : chaque voix écrit
    // `<< \autoBeamOff \dynamicUp { \global \melodie } >>`. Ces commandes n'ont
    // pas d'argument : le bloc `{ … }` qui les suit est de la musique ordinaire
    // et doit être lu, pas consommé comme un argument de commande inconnue.
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         global = { \\key c \\major \\time 4/4 }\n\
         dessus = \\fixed c' { c'4 d' e' f' | g'2 r | <c' e' g'>2~ q2 | }\n\
         bas = \\fixed c { c4 d e f | g2 c' | c'2~ c' | }\n\
         \\score { <<\n\
           \\new Staff <<\n\
             \\context Voice = haut { \\voiceOne <<\n\
               \\autoBeamOff \\dynamicUp { \\global \\dessus }\n\
             >> }\n\
             \\context Voice = basse { \\voiceTwo <<\n\
               \\autoBeamOff \\dynamicDown { \\global \\bas }\n\
             >> }\n\
           >>\n\
           \\new Staff { \\fixed c' { \\clef bass c4 d e f | g2 r | c'2~ c' | } }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![
            72, 74, 76, 77, 79, 72, 76, 79, 72, 76, 79, // dessus
            48, 50, 52, 53, 55, 60, 60, 60, // bas
            60, 62, 64, 65, 67, 72, 72, // seconde portée
        ],
        "hauteurs des trois voix"
    );
    assert_eq!(score.measures.len(), 3, "mesures");
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
        ],
        "durées de la première voix"
    );

    // Une commande inconnue en fin de flux ne consomme rien, et une musique
    // sans commande inconnue reste inchangée (cas non concerné).
    let score =
        read_lilypond("\\score { \\new Staff { \\fixed c' { c'4 d' | e'2 r | } \\autoBeamOff } }")
            .expect("lecture fin de flux");
    assert_eq!(
        hauteurs(&score),
        vec![72, 74, 76],
        "musique avant la commande"
    );
    assert_eq!(score.measures.len(), 2, "mesures avant la commande");
}
