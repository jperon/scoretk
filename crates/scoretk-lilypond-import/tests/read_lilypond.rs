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

/// Durées des attaques seules (les prolongations d'un accord coupé à une
/// barre portent `tie_back`).
fn durees_attaques(score: &scoretk_core::model::Score) -> Vec<Fraction> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter(|chord_rest| match &chord_rest.kind {
            ChordRestKind::Chord(chord) => chord
                .notes
                .iter()
                .all(|&id| score.note(id).tie_back.is_none()),
            _ => true,
        })
        .map(|chord_rest| {
            chord_rest
                .ticks
                .unwrap_or_else(|| chord_rest.duration.ticks())
        })
        .collect()
}

/// Hauteurs des attaques seules : les prolongations issues d'un accord
/// coupé à une barre portent `tie_back` et ne sont pas des attaques MIDI.
/// Décalage de lecture du premier ornement après la note.
fn offset_grace_apres(score: &scoretk_core::model::Score) -> Option<Fraction> {
    score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            let SegmentItem::ChordRest(chord_rest) = segment.element(Track::from_raw(0))? else {
                return None;
            };
            let ChordRestKind::Chord(chord) = &chord_rest.kind else {
                return None;
            };
            chord
                .grace_chords
                .iter()
                .find_map(|grace| grace.played_offset)
        })
        .next()
}

fn hauteurs_attaques(score: &scoretk_core::model::Score) -> Vec<u8> {
    score
        .notes
        .iter()
        .filter(|note| note.tie_back.is_none())
        .map(|note| note.pitch)
        .collect()
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

/// La durée par défaut d'une note nue est fixée à l'analyse
/// (`parser->default_duration_`, `lily/parser.yy` l. 4644), pas au parcours :
/// le second passage d'une reprise ne doit pas hériter de la dernière durée du
/// premier passage. Réduction de `traditioner_af_swenska_folk_dansar.3.13` :
/// `g,` valait 2 (venu de `a2`) puis 3 (venu de `e2.` qui termine le passage).
#[test]
fn unfold_ne_reprend_pas_la_duree_par_defaut_du_passage() {
    let score = lire("\\unfoldRepeats { a2 \\repeat volta 2 { g, g4 e2. } }");
    let attaques: Vec<(i64, u8)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            let chord_rest = segment.chord_rest(Track::from_raw(0))?;
            let pitch = chord_rest
                .chord_ref()
                .and_then(|chord| chord.notes.first().copied())?;
            // Les prolongations d'une note coupée par une barre ne sont pas
            // des attaques.
            if score.note(pitch).tie_back.is_some() {
                return None;
            }
            Some((segment.tick.ticks(480), score.note(pitch).pitch))
        })
        .collect();
    // Passages en noires (1 noire = 120 ticks pour 480 par ronde) :
    // `a2` (0), `g,` (2), `g4` (4), `e2.` (5), puis la reprise : `g,` (8),
    // `g4` (10), `e2.` (11). Avant correctif, `g,` de la reprise durait 3
    // (durée de `e2.`) et décalait le passage à 11/13/14.
    assert_eq!(
        attaques,
        vec![
            (0, 69),
            (240, 55),
            (480, 67),
            (600, 64),
            (960, 55),
            (1200, 67),
            (1320, 64)
        ]
    );
}

/// La lecture des arguments d'une fonction musicale sert seulement à trouver
/// leur fin ; le corps est ensuite relu après substitution (`REPARSE`,
/// `lily/parser.yy` l. 1949, annulé par `MYREPARSE`). Cette première lecture
/// ne doit pas laisser sa durée par défaut derrière elle : dans
/// `faure-sicilienne-guitare-doigtee`, `\barre 3 { … g8 }` donnait la durée 8
/// aux notes nues qui précédaient le `g8` de l'argument.
#[test]
fn arguments_de_fonction_ne_laissent_pas_leur_duree_par_defaut() {
    let source = r#"\version "2.24.0"
\language "english"
barre = #(define-music-function (parser location strg music)(number? ly:music?)
  #{ $music #})
\score { \new Staff { \fixed c' { c16 d e f \barre3 { a b c' d' g8 } } } }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 8),
        ]
    );
}

/// `script_abbreviation` (`lily/parser.yy` l. 3469) : après `^` ou `_`, le
/// signe de ponctuation est une articulation. `d''16^.^4` laissait le `.`
/// dans le flux, et le `4` du doigté était lu comme une noire qui rejouait
/// l'accord : le triolet de `Czerny_Op_821_No_016` gagnait des notes.
#[test]
fn signe_apres_direction_est_une_articulation() {
    let score = lire("\\time 2/4 \\times 2/3 { d''16^.^4 [ d''^.^3 d''^.^2 ] } d''8^.^1");
    assert_eq!(hauteurs(&score), vec![86, 86, 86, 86]);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 8),
        ]
    );
}

/// `bes/+d` : le `+` marque une basse explicite (`scm/chord-entry.scm`
/// l. 46-53, `interpret-bass`). Le lecteur s'arretait sur le `+`, laissait
/// `d` dans le flux et jouait un accord de re au temps suivant : la grille
/// d'accords de WinchesterNew gagnait des notes et se decalait d'un temps.
#[test]
fn accord_basse_explicite_avec_plus() {
    let score = read_lilypond(
        "\\version \"2.18.2\"\n\\score { \\new Staff { \\chordmode { bes4 bes/+d } } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![70, 74, 77, 70, 74, 77, 62]);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4), Fraction::new(1, 4)]
    );
}

/// `nom = …` est une affectation, pas un argument : `\tieUp` (commande sans
/// argument ignoree par le lecteur) avalait le `td` de `td = ...`, puis toute
/// la chaine de definitions, dont `su` (`\change Staff = up`) dans bwv798.
#[test]
fn commande_inconnue_ne_mange_pas_une_affectation() {
    let source = "\\version \"2.18.2\"\ntu = \\tieUp\ntd = { c'4 }\n                  \\score { \\new Staff { \\td } }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![60]);
}

/// `/d` est un renversement (`scm/chord-entry.scm`, `process-inversion` et
/// `make-chord-elements`) : la note de basse entre dans l'accord et les
/// autres notes sont ramenees dans l'octave au-dessus, au lieu de garder
/// l'accord fondamental et d'ajouter une basse (`bes/d` = re4 fa4 sib4).
#[test]
fn accord_renverse_place_les_notes_au_dessus_de_la_basse() {
    let score =
        read_lilypond("\\version \"2.18.2\"\n\\score { \\new Staff { \\chordmode { bes/d } } }")
            .expect("lecture");
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    assert_eq!(hauteurs, vec![62, 65, 70]);
}

/// `\simultaneous { … }` est le jumeau de `<< … >>` (`SIMULTANEOUS
/// braced_music_list`, `lily/parser.yy` l. 1581-1588) : les elements du bloc
/// sont simultanes. Lire le bloc comme une sequence decalait la seconde voix
/// apres la premiere (`bach_air_bmv_1068`, guitare).
#[test]
fn simultaneous_est_simultane() {
    let source =
        "\\version \"2.18.2\"\n\\score { \\new Staff { \\simultaneous { { c'4 } { e'4 } } } }";
    let score = read_lilypond(source).expect("lecture");
    let mut attaques: Vec<(u8, bool)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => {
                        let chord = chord_rest.chord_ref()?;
                        let pitch = score.note(*chord.notes.first()?).pitch;
                        Some((pitch, segment.tick.ticks(1920) == 0))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    assert_eq!(attaques, vec![(60, true), (64, true)]);
}

/// `q` rejoue les hauteurs du dernier accord, pas ses prolongations : la
/// note repetee ferme la liaison mais n'en commence pas une autre
/// (`repetition-chord`, `scm/ly-syntax-constructors.scm` l. 228-232). Sans
/// cela, la liaison du `q` s'etendait jusqu'a l'accord suivant et son A3
/// etait ecrit en `tie stop`, donc perdu comme attaque (`catacombae`).
#[test]
fn q_ne_prolonge_pas_la_liaison_du_accord_precedent() {
    let source = "\\version \"2.18.2\"\n\\score { \\new Staff \\relative c' {\n                  <e, _~ a ~ e' ^~>2. | q2. |\n                  <a ^~ f ^~ a, _~>2. | q2.\n} }";
    let score = read_lilypond(source).expect("lecture");
    let mut attaques: Vec<(u8, i64)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => Some(
                        chord_rest
                            .chord_ref()
                            .into_iter()
                            .flat_map(|chord| chord.notes.iter().copied())
                            .filter(|id| score.note(*id).tie_back.is_none())
                            .map(|id| (score.note(id).pitch, segment.tick.ticks(1920)))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    assert_eq!(
        attaques,
        vec![
            (45, 2880),
            (52, 0),
            (53, 2880),
            (57, 0),
            (57, 2880),
            (64, 0),
        ]
    );
}

/// Un script de `ly/script-init.ly` est un post-evenement : `\downprall`
/// ne doit pas faire sortir la commande du post-evenement, sinon le `~` qui
/// suit est avale et la liaison est perdue (`bwv-988-v22`, g1 \downprall ~).
#[test]
fn script_d_ornement_garde_la_liaison_suivante() {
    let score = lire("g'1 \\downprall ~ g'1");
    assert_eq!(score.notes.len(), 2, "les deux notes sont liees");
    assert_eq!(
        hauteurs_attaques(&score),
        vec![79],
        "une seule attaque : la liaison tient"
    );
}

/// Un script sans equivalent dans le modele est consomme seul : passer par
/// parse_direction_event avalait la commande suivante (`\downprall
/// \times 2/3 { … }` perdait le n-olet, BWV-830-4).
#[test]
fn script_sans_equivalent_ne_mange_pas_le_nolet_suivant() {
    let score = lire("c'8 \\downprall \\times 2/3 { e'16 f' g' } d'4");
    assert_eq!(hauteurs(&score), vec![72, 76, 77, 79, 74],);
    let mut attaques: Vec<(u8, i64)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => {
                        let chord = chord_rest.chord_ref()?;
                        let pitch = score.note(*chord.notes.first()?).pitch;
                        Some((pitch, segment.tick.ticks(1920)))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    // 1/8, puis trois 1/24 (nolet 3:2), puis 1/4 ; sans le nolet les
    // attaques seraient à 240, 360, 480 et 600.
    assert_eq!(
        attaques,
        vec![(72, 0), (74, 480), (76, 240), (77, 320), (79, 400)]
    );
}

/// `\compressMusic` est l'ancien nom de `\scaleDurations`
/// (`python/convertrules.py` l. 2765) : meme argument et memes durees
/// multipliees par n/d (bwv631a et trois autres chorals).
#[test]
fn compress_music_est_scale_durations() {
    let score = lire("\\compressMusic #'(3 . 2) { c'4 d' }");
    assert_eq!(hauteurs(&score), vec![72, 74]);
    assert_eq!(
        durees_attaques(&score),
        vec![Fraction::new(3, 8), Fraction::new(3, 8)]
    );
}

/// `\\set tieWaitForNote = ##t` : les liaisons pendantes ne sont plus jetees a
/// la fin de leur note (`lily/tie-performer.cc` l. 166-180), la note de meme
/// hauteur qui arrive plus tard les ferme (`acknowledge_audio_element`,
/// l. 116-150). `Chop-28-1` : `\\tuplet 3/2 {c,16~ g,~ c~} s8 | <c, g, c>4`
/// n'a que trois attaques avec le reglage, six sans.
#[test]
fn tie_wait_for_note_garde_les_liaisons_pendantes() {
    let avec = lire("\\set tieWaitForNote = ##t \\tuplet 3/2 { c'16~ g'~ c''~ } s8 <c' g' c''>4");
    assert_eq!(hauteurs_attaques(&avec), vec![72, 79, 84]);

    let sans = lire("\\tuplet 3/2 { c'16~ g'~ c''~ } s8 <c' g' c''>4");
    assert_eq!(hauteurs_attaques(&sans), vec![72, 79, 84, 72, 79, 84]);
}

/// `-"texte"` reste une direction textuelle (`lily/parser.yy` `post_event:
/// '-' fingering`, `direction_reqd_event: gen_text_def` qui accepte STRING) :
/// la consommer laisse le `~` qui suit sans note et casse la liaison
/// (`Lobt.ly`, `re2-"4--5" \fermata ~re4`).
#[test]
fn texte_apres_tiret_garde_la_liaison_suivante() {
    let score = lire("c'2-\"4--5\" ~c'4 c'2");
    assert_eq!(hauteurs_attaques(&score), vec![72, 72]);
}

/// `\\(` et `\\)` sont des post-événements (`lily/parser.yy`
/// `direction_less_event`) : les omettre coupait la lecture après elles et
/// le `~` suivant était perdu (`Chop-28-4`, `gs4->\(~ gs4`).
#[test]
fn commande_de_liaison_d_expression_garde_le_tie_suivant() {
    let score = lire("b'4\\(~ b'4");
    assert_eq!(hauteurs_attaques(&score), vec![83]);
}

/// Une liste entre accolades qui suit une commande de style de markup est son
/// argument (`lily/parser.yy` `markup_braced_list`) : `\markup \bold {a
/// Tempo}` ne doit pas laisser `a` devenir une note.
#[test]
fn markup_de_style_consomme_sa_liste_entre_accolades() {
    let score = lire("c'2 ^\\markup \\bold {a Tempo} d'4");
    assert_eq!(hauteurs(&score), vec![72, 74]);
}

/// Une redefinition Scheme d'une commande integree (`#(define ottava
/// (let … (define-music-function …)))`, Chop-28-19) enveloppe la commande
/// d'origine : le lecteur ne sait pas evaluer la fonction, la commande
/// integree doit rester prioritaire et non le corps `#{ … #}` stocke.
#[test]
fn une_redefinition_scheme_ne_masque_pas_une_commande_integree() {
    let src = "\\version \"2.24.0\"\n#(define ottava #{ { d''4 } #})\n\\relative c' { c4 \\ottava #1 e4 f \\ottava #0 g4 }\n";
    let score = read_lilypond(src).expect("lecture");
    // Le corps Scheme stocké (`d''4`) ne doit pas devenir une note ; `\ottava`
    // reste la convention d'écriture de LilyPond (pas de transposition du son).
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 64, 65, 67]
    );
    assert!(score
        .spanners
        .iter()
        .any(|spanner| matches!(spanner.kind, scoretk_core::model::SpannerType::Ottava(_))));
}

/// Ancienne syntaxe `\override Context #'prop = valeur` (avant 2.18) :
/// le nom quoté doit être consommé, sinon `= 1` reste dans la musique et
/// le `1` devient une durée de blanche ronde qui décale la suite
/// (Symphony5_1/2).
#[test]
fn override_ancienne_syntaxe_ne_consomme_pas_la_duree_suivante() {
    let src = "\\version \"2.24.0\"\n\\score { \\new Staff { \\time 2/4 \\override MultiMeasureRest #'expand-limit = 1 \\repeat volta 2 { r8 c'8 c' c' } } }\n";
    let score = read_lilypond(src).expect("lecture");
    let mut attaques: Vec<i64> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => {
                        chord_rest.chord_ref()?;
                        Some(segment.tick.ticks(1920))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    // r8 (0,25 en blanches), puis trois croches à 0,5/1/1,5 noires.
    assert_eq!(attaques, vec![240, 480, 720]);
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
    assert_eq!(
        hauteurs_attaques(&score),
        vec![65, 74, 62, 71, 60, 60, 64, 67]
    );
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::QUARTER,
            Fraction::new(1, 8),
            Fraction::QUARTER,
            Fraction::new(3, 8),
            Fraction::new(1, 8),
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

#[test]
fn transpose_dans_relative_reste_absolu() {
    // Notation Reference §1.2.2 : « Music inside a \transpose block is
    // absolute unless a \relative is included in the block. » Côté Scheme,
    // `TransposedMusic` porte le `to-relative-callback`
    // `ly:relative-octave-music::no-relative-callback`
    // (`scm/define-music-types.scm`) : la conversion `\relative` ne descend
    // pas dans le bloc transposé. Les notes y gardent leur octave absolue
    // (`f` = F3 = 53, transposé c→d = G3 = 55), et la référence relative qui
    // suit le bloc reste la dernière note AVANT lui (D7, d'où `c` = C7 = 96),
    // et non la dernière note du bloc. Vérifié contre le MIDI de LilyPond 2.27
    // sur cette même source réduite.
    let source = "\
        \\score { << \
          \\new Staff \\relative c' { \
            c'''4 d4 \\transpose c d { f4 <g b>4 } | c4~ c4 r2 | \
          } \
          \\new Staff \\relative c { c2 d2 | e1 } \
        >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![96, 98, 55, 57, 61, 96, 96, 48, 50, 52]
    );
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::QUARTER; 6]
            .into_iter()
            .chain([Fraction::HALF])
            .collect::<Vec<_>>()
    );
    assert_eq!(score.measures.len(), 2);

    // Cas non concerné : sous `\fixed`, la référence absolue est conservée et
    // la transposition s'applique normalement (F4 transposé c→d = G4 = 67).
    let fixe = lire("\\fixed c' { \\transpose c d { f4 } }");
    assert_eq!(hauteurs(&fixe), vec![67]);
    assert_eq!(fixe.measures.len(), 1);
}

#[test]
fn make_relative_dans_transpose_garde_le_contexte_relative() {
    // `scm/music-functions.scm` : `make-relative` relit ses arguments dans le
    // `\relative` environnant AVANT de construire le `TransposedMusic`. Le
    // `\transpose` n'est donc pas un corps littéral absolu (§1.2.2) : ses
    // hauteurs doivent rester celles du contexte relatif. Source réduite de
    // `input/regression/make-relative-copies.ly`, vérifiée contre le MIDI de
    // LilyPond 2.27 (55, 59, 62, 67 par portée).
    let source = "\
        reltranspose = #(define-music-function (from to music) \
          (ly:pitch? ly:pitch? ly:music?) \
          (make-relative (music) music #{ \\transpose #from #to $music #})) \
        mus = \\reltranspose c g { \\partial 4. c8 e g | c2 r8 c, e g c1 } \
        \\score { \\new Staff \\relative \\mus }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![55, 59, 62, 67, 55, 59, 62, 67]);
    // Le `\partial 4.` décale la numérotation : huit noires tiennent dans
    // trois mesures (levée + deux mesures pleines).
    assert_eq!(score.measures.len(), 3);
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
    let attendu = vec![74, 79, 81, 82, 81, 79, 84, 82, 81, 74, 79, 96, 112, 93];
    // Cas non concerné : sans `\transposition`, la musique reste identique.
    assert_eq!(hauteurs_attaques(&sans), attendu);
    assert_eq!(hauteurs_attaques(&avec), attendu);
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
            Fraction::new(3, 8),
            Fraction::new(1, 8),
            Fraction::HALF,
        ]
    );
}

/// `\transposition` est une propriété du Staff (`lily/staff-performer.cc`,
/// `instrumentTransposition`) : quand une portée source est dépliée en deux
/// portées par le modèle (voix nommées), le décalage vaut pour les deux.
#[test]
fn transposition_sur_portee_depliee_decale_toutes_les_voix() {
    let score = read_lilypond(
        "\\score { \\context Staff = \"g\" << \\context Staff = \"g\" { \\transposition c } { c''4 } { e''4 } >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 64]);
    assert_eq!(
        score.parts[0].instrument.transpose,
        Some(Transpose::new(-7, -12))
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
    // StaffGroup, donc une seule partie MusicXML sans `<transpose>` global.
    // LilyPond 2.26 joue cependant la portée transpositrice une sixte plus
    // haut : la concert de cette portée seule est décalée dans le modèle.
    let orchestre = read_lilypond(
        "\\score { << \\new StaffGroup << \
         \\new Staff { \\transposition a' \\relative c' { c4 d e f } } \
         \\new Staff { \\relative c' { c4 d e f } } >> >> }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&orchestre), vec![69, 71, 73, 74, 60, 62, 64, 65]);
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
fn partial_avec_multiplicateur_donne_la_longueur_de_mesure() {
    // Notation Reference §2.3.3 « Upbeats » : `\partial` prend une durée et
    // les multiplicateurs sont admis, donc `\partial 8*5` vaut cinq croches.
    // Source réduite d'une levée en fin de partition, avec barres de contrôle,
    // accord, silence et mesures pleines qui encadrent la levée.
    let score = lire(concat!(
        "\\time 3/4 \\partial 8*5 ",
        "c'8 c'8 c'8 c'8 c'8 | ",
        "d'4. e'4. | ",
        "<g' b' d''>2. \\bar \"|.\"",
    ));
    let lens: Vec<_> = score.measures.iter().map(|measure| measure.len).collect();
    assert_eq!(
        lens,
        vec![
            Fraction::new(5, 8),
            Fraction::new(3, 4),
            Fraction::new(3, 4),
        ],
        "longueurs des mesures"
    );
    assert!(score.measures[0].is_irregular());
    assert_eq!(
        durees_ecrites(&score),
        [
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(3, 8),
            Fraction::new(3, 8),
            Fraction::new(3, 4),
        ],
        "durées écrites de la voix"
    );
}

#[test]
fn partial_multiplicateur_en_cours_de_partition() {
    // Réduction de `FauconDArmor.ly` : `\partial 8` en ouverture (cas non
    // concerné, doit rester 1/8) puis `\partial 8*5` en fin de partition, en
    // 6/8. La deuxième levée vient juste après une barre implicite.
    let score = lire(concat!(
        "\\time 6/8 \\partial 8 c'8 | ",
        "d'4. e'8 f'8 g'8 | ",
        "\\partial 8*5 <c' e' g'>4. r4 \\bar \"|.\"",
    ));
    let lens: Vec<_> = score.measures.iter().map(|measure| measure.len).collect();
    assert_eq!(
        lens,
        vec![
            Fraction::new(1, 8),
            Fraction::new(6, 8),
            Fraction::new(5, 8)
        ],
        "longueurs des mesures"
    );
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 8),
            Fraction::new(3, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(3, 8),
            Fraction::new(2, 8),
        ],
        "durées écrites de la voix"
    );
}

#[test]
fn contexte_staff_simple_garde_trame_et_grace() {
    // Extrait minimal de `grace-staff-length.ly` : après un silence d'une
    // mesure, un `\context Staff` (contexte EXISTANT) contient un ornement et
    // sa note. LilyPond poursuit la trame de la portée : la note est jouée
    // dans la deuxième mesure, avec son ornement, sur la même piste.
    let source = concat!(
        "\\version \"2.16.0\"\n\\language \"english\"\n",
        "\\score { \\new PianoStaff << ",
        "\\new Staff { R1 R1 \\bar \"|.\" } ",
        "\\new Staff \\fixed c' { R1 \\context Staff { \\grace { c16 } c1 } } ",
        ">> }"
    );
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, Fraction::ONE);
    // La note principale est dans la mesure 2 (tick 4), pas au début de la
    // portée : le `\context Staff` ne rouvre pas une voix à zéro.
    let notes_du_context: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| {
            let raw = segment.element(Track::from_raw(4))?;
            let scoretk_core::model::SegmentItem::ChordRest(chord_rest) = raw else {
                return None;
            };
            let scoretk_core::model::ChordRestKind::Chord(chord) = &chord_rest.kind else {
                return None;
            };
            Some((
                segment.tick,
                chord_rest.duration.kind,
                chord.notes.len(),
                chord.grace_chords.len(),
            ))
        })
        .collect();
    assert_eq!(
        notes_du_context,
        vec![(Fraction::ONE, DurationType::Whole, 1, 1)]
    );
    assert_eq!(hauteurs(&score), vec![60, 60]);
}

#[test]
fn new_staff_imbrique_garde_sa_propre_voix() {
    // Extrait de `incipit-modern.ly` : `\incipit \new Staff { … }` crée une
    // vraie portée, pas un contexte existant. Ses notes gardent leur piste et
    // leur trame ; les fondre dans la voix principale ferait chevaucher les
    // attaques des deux groupes (N2 4/4 → 0/4 dans l'oracle).
    let source = concat!(
        "\\score { \\new Staff { \\fixed c' { ",
        "\\incipit \\new Staff \\with { firstClef = ##f } { c4 e g c' } ",
        "d4 f a d' } } }"
    );
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 2);
    let incipit = accords_de_voix(&score, 1);
    assert_eq!(
        incipit
            .iter()
            .map(|(_, pitches, _)| pitches.clone())
            .collect::<Vec<_>>(),
        vec![vec![60], vec![64], vec![67], vec![72]]
    );
    assert_eq!(incipit[0].0, Fraction::ZERO);
    let principal = accords_de_voix(&score, 0);
    assert_eq!(
        principal
            .iter()
            .map(|(_, pitches, _)| pitches.clone())
            .collect::<Vec<_>>(),
        vec![vec![62], vec![65], vec![69], vec![74]]
    );
    assert_eq!(principal[0].0, Fraction::ONE);
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

/// Deux `\context Staff = "x"` du même `\score` retrouvent la **même**
/// portée (`lily/context.cc`, `Context::find_create_context` →
/// `find (FIND_CREATE)` : un contexte nommé est retrouvé, pas dupliqué).
/// La seconde musique devient une voix simultanée de la portée, sur le même
/// `Audio_staff` MIDI (`lily/staff-performer.cc`, `get_audio_staff`).
#[test]
fn context_staff_de_meme_nom_est_retrouve() {
    let source = "\\score { << \
                  \\context Staff = \"x\" { \\fixed c' { c4 d e f } } \
                  \\context Staff = \"x\" { \\fixed c' { g4 a b c' } } >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1, "une seule partie");
    assert_eq!(score.staves.len(), 1, "une seule portee nommee x");
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    assert_eq!(hauteurs, vec![60, 62, 64, 65, 67, 69, 71, 72]);
}

/// Dans une séquence `{ … }`, un `\context Staff` retrouvé plus tard
/// continue **dans le temps** (`lily/parser.yy` `sequential_music`) : la
/// seconde section commence après la première, elle ne repart pas du tick 0
/// (alas.ly, deux `<< … >>` successifs sur les mêmes portées nommées).
#[test]
fn context_staff_retrouve_continue_dans_le_temps() {
    let source = "\\score { { \
        << \\context Staff = \"x\" { \\fixed c' { c4 d e f } } >> \
        << \\context Staff = \"x\" { \\fixed c' { g4 a b c' } } >> } }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.staves.len(), 1);
    let mut attaques: Vec<(u8, i64)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => {
                        let chord = chord_rest.chord_ref()?;
                        let pitch = score.note(*chord.notes.first()?).pitch;
                        Some((pitch, segment.tick.ticks(1920)))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    assert_eq!(
        attaques,
        vec![
            (60, 0),
            (62, 480),
            (64, 960),
            (65, 1440),
            (67, 1920),
            (69, 2400),
            (71, 2880),
            (72, 3360),
        ]
    );
}

/// `\context Voice = "x"` qui reparaît dans une section ultérieure continue
/// la même voix (`lily/context.cc` `find_create_context`) : la musique de la
/// section se place à la date de son élément dans la séquence
/// (`lily/parser.yy` `sequential_music`), pas au tick 0 (carcassi-op60-09).
#[test]
fn context_voice_de_meme_nom_continue_dans_le_temps() {
    let source = "\\score { { \
        \\context Staff = \"g\" { \\set Staff.midiInstrument = \"acoustic guitar (nylon)\" } \
        << \\context Voice = \"a\" { \\fixed c' { c4 } } >> \
        << \\context Voice = \"b\" { \\fixed c' { d4 } } >> \
        << \\context Voice = \"a\" { \\fixed c' { e4 } } >> } }";
    let score = read_lilypond(source).expect("lecture");
    let mut attaques: Vec<(u8, i64)> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .flat_map(|segment| {
            segment
                .iter_elements()
                .filter_map(|(_, element)| match element {
                    SegmentItem::ChordRest(chord_rest) => {
                        let chord = chord_rest.chord_ref()?;
                        let pitch = score.note(*chord.notes.first()?).pitch;
                        Some((pitch, segment.tick.ticks(1920)))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    attaques.sort_unstable();
    assert_eq!(attaques, vec![(60, 0), (62, 480), (64, 960)]);
}

/// `\transposition` est une propriété de Staff : posée dans un
/// `\context Staff` sans musique (`midiStuff`), elle vaut pour **toutes** les
/// voix qui suivent sur cette portée, pas seulement la première
/// (`lily/staff-performer.cc`, `instrumentTransposition`).
#[test]
fn transposition_d_un_contexte_vide_vaut_pour_les_voix_suivantes() {
    let source = "\\score { { \
        \\context Staff = \"g\" { \\transposition c } \
        << \\context Voice = \"a\" { \\relative c'' { c4 } } \
           \\context Voice = \"b\" { \\relative c' { a2 } } >> } }";
    let score = read_lilypond(source).expect("lecture");
    let mut hauteurs = hauteurs(&score);
    hauteurs.sort_unstable();
    // `\transposition c` : un do' écrit sonne un do (une octave plus bas).
    assert_eq!(hauteurs, vec![45, 60]);
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
        hauteurs_attaques(&score),
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
fn language_sans_guillemets() {
    // LilyPond accepte `\language deutsch` sans guillemets (le fichier de
    // l'Alléluia de Lassus l'écrit ainsi) : `h` est si naturel, `b` si bémol.
    let score = lire_sans_avertissement_d_include(
        "\\version \"2.18.2\"\n\\language deutsch\n\\score { \\new Staff { h'4 b'4 } }\n",
    );
    assert_eq!(hauteurs(&score), vec![71, 70]);
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
    assert_eq!(hauteurs_attaques(&score), vec![60, 62, 64, 65, 67, 69]);
    assert_eq!(
        durees_attaques(&score),
        vec![
            Fraction::new(1, 6),
            Fraction::new(1, 6),
            Fraction::new(1, 6),
            Fraction::new(1, 6),
            Fraction::new(1, 16),
            Fraction::new(1, 6),
        ]
    );
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

/// Les noms de contexte peuvent etre ecrits sans guillemets : LilyPond
/// accepte un symbole nu (`CHANGE STRING '=' simple_string`, `lily/parser.yy`).
/// `\new Staff = one` perdait son nom, et `\change Staff = one` ne trouvait
/// plus la portee (avertissement « portee inconnue »).
#[test]
fn noms_de_contexte_sans_guillemets() {
    let source = r#"\version "2.18.2"
\score { \context PianoStaff <<
  \new Staff = one { c'4 }
  \new Staff = two { \change Staff = one e'4 }
>> }
"#;
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("portée inconnue")),
        "avertissements : {warnings:?}"
    );
    assert_eq!(hauteurs(&score), vec![60, 64]);
}

#[test]
fn change_dans_beam_cross_staff() {
    // Reduced inline copy of LilyPond's `input/regression/beam-cross-staff.ly`
    // (SPDX-License-Identifier: GPL-3.0-or-later). The previous version read
    // an absolute home path and silently skipped when the file was absent.
    let source = r#"\version "2.16.0"
\context PianoStaff <<
  \new Staff = "one" \relative c'{
    \stemUp  c8[ c \change Staff=two \stemUp c c]
    c[ c c c]
    \change Staff=one
    \stemDown  f8[ f \change Staff=two \stemUp c c]
    r2
    \stemDown  c8[ c \change Staff=one \stemDown c c]
    r2
    \change Staff=two
    \stemUp  c8[ c \change Staff=one \stemDown f f]
    r2
  }
  \new Staff = "two" \relative c'{
    \clef bass
    s1 s1 s1 s1
  }
>>
"#;
    assert!(source.contains("\\change"), "source de régression attendue");
    let (score, warnings) = read_lilypond_reporting(source).expect("lecture");
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

/// `\set Timing.measurePosition = #(ly:make-moment -1 4)` : LilyPond replace
/// la position courante dans la mesure (`lily/timing-translator.cc` l. 470-505).
/// Une position negative fait tomber la barre de retour a zero a `t + 1/4` :
/// l'anacrouse de deux croches forme sa propre mesure, puis les 2/4 suivent.
#[test]
fn measure_position_negative_ferme_la_mesure_sur_l_anacrouse() {
    let score = lire(
        "\\time 2/2 c1 d1 \\set Timing.measurePosition = #(ly:make-moment -1 4)          \\time 2/4 e8 f g4 a4",
    );
    assert_eq!(
        longueurs(&score),
        vec![
            Fraction::new(1, 1),
            Fraction::new(1, 1),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
        ]
    );
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67, 69]);
}

#[test]
fn timing_translator_dans_staff_rend_le_temps_local() {
    // Extrait de `skiptypesetting-show-first-and-last.ly` : le bloc `\layout`
    // retire `Timing_translator` de `Score` et l'ajoute à `Staff`. Les deux
    // portées ont alors leur propre mesure (3/4 et 4/4), au lieu d'une trame
    // unique imposée par la première portée.
    let source = concat!(
        "\\version \"2.23.8\"\n",
        "\\layout { \\context { \\Score \\remove \"Timing_translator\" } ",
        "\\context { \\Staff \\consists \"Timing_translator\" } }\n",
        "\\score { << ",
        "\\new Staff { \\fixed c' { \\time 3/4 c4 c4 c4 | d4 d4 d4 } } ",
        "\\new Staff { \\fixed c { \\clef bass \\time 4/4 c4 c4 c4 c4 } } ",
        ">> }"
    );
    let score = read_lilypond(source).expect("lecture");
    // Frontières 0, 3, 4, 6 quarts : les deux tempos sont conservés.
    assert_eq!(
        longueurs(&score),
        vec![
            Fraction::new(3, 4),
            Fraction::new(1, 4),
            Fraction::new(2, 4),
        ]
    );
    assert_eq!(score.measures.len(), 3);
    assert_eq!(
        hauteurs(&score),
        vec![60, 60, 60, 62, 62, 62, 48, 48, 48, 48]
    );

    // Cas non concerné : sans déplacement du traducteur, un seul `\time`
    // vaut pour toute la partition ; la mesure reste celle de la portée 0.
    let sans_deplacement = source.replace("\\remove \"Timing_translator\" ", "");
    let sans_deplacement = sans_deplacement.replace("\\consists \"Timing_translator\" ", "");
    let score = read_lilypond(&sans_deplacement).expect("lecture");
    assert_eq!(
        longueurs(&score),
        vec![Fraction::new(3, 4), Fraction::new(3, 4)]
    );
    assert_eq!(score.measures.len(), 2);
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

    // Cas réel réduit : `musicxml2ly` écrit une partition imprimée (`\layout`)
    // puis la partition MIDI (`\midi` sans `\layout`, transposée). LilyPond
    // n'écrit qu'un MIDI, celui de la seconde (NR §24.3) : c'est elle qui
    // fournit le modèle, à hauteur de concert de la référence.
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
    assert_eq!(hauteurs(&score), vec![62, 64]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::QUARTER, Fraction::QUARTER]
    );
    // Le modèle est déjà la partition MIDI transposée : aucune transposition
    // de lecture supplémentaire ne doit être appliquée par l'écrivain MIDI.
    assert_eq!(score.playback_transposition, None);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.message == "partition supplémentaire ignorée")
            .count(),
        1
    );

    // Même arbitrage avec des fonctions musicales : la partition MIDI
    // (`\transpose d c`, d'un ton plus bas) est celle que LilyPond joue,
    // donc le modèle suit ses hauteurs (58/60).
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
    assert_eq!(hauteurs(&score), vec![58, 60]);
    assert_eq!(score.measures.len(), 1);
    // Le modèle est la partition MIDI : pas de transposition de lecture.
    assert_eq!(score.playback_transposition, None);
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
fn simultane_sans_contexte_une_portee_par_element() {
    // Un `<< … >>` de `\score` sans aucun `\new` ouvre un contexte par
    // élément : LilyPond grave deux portées et joue deux pistes MIDI
    // (`lily/parser.yy` `context_prefix`, `lily/context.cc`). C'est l'idiome
    // des partitions chorales Mutopia. Dans une portée, les mêmes éléments
    // restent des voix (`collect_staves`).
    let score = lire_score("\\score { << { c'4 } { e'4 } >> }");
    assert_eq!(score.staves.len(), 2);
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
    // Plusieurs contextes dans un même `{ … }` sont rares : LilyPond les
    // crée successivement (`sequential_music`), la seconde portée commençant
    // après la musique de la première ; un avertissement le signale. La
    // musique nue qui les accompagne reste la première portée anonyme, et un
    // `{ … }` sans aucun contexte ne change pas.
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
    // `f` (3,5) franchit la barre de mesure à 4 et devient deux notes liées.
    assert_eq!(hauteurs, vec![48, 50, 52, 53, 53, 60]);

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
        hauteurs_attaques(&score),
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
            Fraction::QUARTER,
            Fraction::QUARTER,
            Fraction::HALF,
        ]
    );

    // Cas non concerné : `q` et les durées données restent inchangés.
    let score = lire("\\time 4/4 c4 q8 q8 d4 e4");
    assert_eq!(hauteurs_attaques(&score), vec![60, 60, 60, 62, 64]);
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
    // `\chords` décrit une grille d'accords que LilyPond joue dans son MIDI
    // (track dédié, vérifié en 2.26). La grille précède la mélodie dans la
    // source : ses notes doivent donc venir en premier.
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score { <<\n\
           \\chords { c:7 d:m }\n\
           \\new Staff { c'4 d'4 }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![60, 64, 67, 70, 62, 65, 69, 60, 62],
        "accords joués puis mélodie"
    );
    let durees_melodie: Vec<Fraction> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(4)))
        .map(|chord_rest| chord_rest.duration.ticks())
        .collect();
    assert_eq!(
        durees_melodie,
        vec![Fraction::new(1, 4); 2],
        "durées de la mélodie (piste 4)"
    );
    assert_eq!(score.measures.len(), 1, "mesures");

    // `\simultaneous` n'est pas une grille : il reste de la musique.
    let source = "\\version \"2.24.0\"\n\\simultaneous { c'4 d'4 e'4 }";
    let score = read_lilypond(source).expect("lecture partition implicite");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert_eq!(score.measures.len(), 1, "mesures implicites");
}

#[test]
fn grille_daccords_jouee_dans_le_midi() {
    // Vérifié en compilant ces sources avec LilyPond 2.26 : `\chords` et les
    // contextes ChordNames produisent une piste MIDI jouée, avec les notes des
    // accords. Seules les grilles de chiffres (FiguredBass) restent muettes.
    let source = "\\version \"2.26.0\"\n\
         \\language \"english\"\n\
         \\score { <<\n\
           \\new Staff { \\time 2/4 c'4 d'4 | e'4 f'4 }\n\
           \\chords { c:7 d:m }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture de la grille jouée");
    // C7 = 60/64/67/70, Dm = 62/65/69, puis la mélodie en do central.
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65, 60, 64, 67, 70, 62, 65, 69],
        "mélodie puis accords joués"
    );
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 4); 4],
        "durées de la mélodie (piste 0)"
    );
    assert_eq!(
        score.measures.len(),
        2,
        "deux mesures avec barre de contrôle"
    );

    // `\new ChordNames` et `\context ChordNames` jouent aussi, y compris avec
    // `\chordmode` explicite dans une variable et un `\with` sur le contexte.
    let new_context = read_lilypond(
        "grille = \\chordmode { c2 c:m7 }\n\
         \\score { << \\new ChordNames \\with { chordChanges = ##t } \\grille \
         \\new Staff { c'2 d'2 } >> }",
    )
    .expect("grille \\new ChordNames");
    assert_eq!(
        hauteurs(&new_context),
        vec![60, 64, 67, 60, 63, 67, 70, 60, 62],
        "accords de la grille puis mélodie"
    );
    assert_eq!(new_context.measures.len(), 1, "une mesure");
    let existing = read_lilypond(
        "grille = \\chordmode { c1 c:m }\n\
         \\score { << \\context ChordNames \\grille \
         \\new Staff { c'1 d'1 } >> }",
    )
    .expect("grille \\context ChordNames");
    assert_eq!(
        hauteurs(&existing),
        vec![60, 64, 67, 60, 63, 67, 60, 62],
        "accords du contexte puis mélodie"
    );

    // Cas « non concerné » : FiguredBass reste une grille muette.
    let figures = read_lilypond(
        "\\score { << \\new FiguredBass \\figures { <6>1 <4>1 } \
         \\new Staff { c'2 d' e' f' } >> }",
    )
    .expect("grille de chiffres");
    assert_eq!(hauteurs(&figures), vec![60, 62, 64, 65], "mélodie seule");
}

#[test]
fn grilles_de_chiffres_non_jouees_et_fin_de_flux() {
    // Une grille d'accords comme seule musique est jouée, sans avertissement.
    let (score, warnings) = read_lilypond_reporting("\\version \"2.24.0\"\n\\chords { c:7 d:m }")
        .expect("grille seule");
    assert_eq!(hauteurs(&score), vec![60, 64, 67, 70, 62, 65, 69]);
    assert!(
        !warnings.iter().any(|w| w.message.contains("grille")),
        "aucun avertissement sur une grille jouée : {warnings:?}"
    );
    // Cas limite : fin de fichier sans argument après `\chordmode`.
    let score = read_lilypond("\\version \"2.24.0\"\n\\chordmode").expect("chordmode seul en fin");
    assert!(hauteurs(&score).is_empty(), "aucune note en fin de fichier");

    // `\figures` et `\new FiguredBass` restent des grilles muettes ; la grille
    // `\new ChordNames` joue, avec des notes seules (pas de triade sans
    // `\chordmode`, comme dans le MIDI de LilyPond 2.26).
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         \\score { <<\n\
           \\new ChordNames { c1 d1 }\n\
           \\figures { e1 f1 }\n\
           \\new FiguredBass { g1 a1 }\n\
           \\new Staff { c'4 d'4 | e'4 f'4 }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![48, 50, 60, 62, 64, 65],
        "grille jouée puis mélodie (chiffres ignorés)"
    );
    let durees_melodie: Vec<Fraction> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(4)))
        .map(|chord_rest| chord_rest.duration.ticks())
        .collect();
    assert_eq!(
        durees_melodie,
        vec![Fraction::new(1, 4); 4],
        "durées de la mélodie (piste 4)"
    );
    // La grille en blanches couvre deux mesures, la mélodie une seule.
    assert_eq!(score.measures.len(), 2, "mesures");
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
         #(ly:parser-include-string \"\\\\relative { a4 b c d }\")";
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
        hauteurs_attaques(&score),
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
            Fraction::new(3, 4),
            Fraction::new(1, 4),
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
    assert_eq!(
        hauteurs_attaques(&score),
        vec![60, 62, 64],
        "musique après chaîne"
    );
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

#[test]
fn mark_markup_entre_accolades_ne_devient_pas_une_note() {
    // Régression Angels_Carol (musicxml2ly, Rutter) : les voix portent
    // `\mark \markup { \box { A } }`. Le gestionnaire de `\mark` laissait la
    // commande `\markup` en place : son bloc entre accolades restait dans le
    // flux et, lu comme de la musique, le « A » de `\box { A }` devenait une
    // note parasite qui décalait toutes les mesures suivantes.
    let source = "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         global = { \\key c \\major \\time 4/4 }\n\
         dessus = \\fixed c' {\n\
           \\global c4 d | e2 r |\n\
           \\mark \\markup { \\box { A } } <c e g>2~ q2 | c1\n\
         }\n\
         accomp = \\relative c' { c4 d | e2 r | c2~ c | }\n\
         \\score { <<\n\
           \\new Staff <<\n\
             \\context Voice = haut { \\voiceOne \\dessus }\n\
             \\context Voice = basse { \\voiceTwo \\accomp }\n\
           >>\n\
           \\new Staff { \\fixed c' { c'4 d' | e'2 r' | } }\n\
         >> }";
    let score = read_lilypond(source).expect("lecture");
    // Première voix : do4 ré4 | mi4 silence | accord do-mi-sol lié à lui-même
    // puis do4. La seconde portée ajoute do5 ré5 | mi5 silence.
    assert_eq!(
        hauteurs_attaques(&score),
        vec![
            60, 62, 64, 60, 64, 67, 60, 60, // dessus
            62, 64, 60, // accompagnement
            72, 74, 76, // seconde portée
        ],
        "hauteurs : le « A » du markup ne doit pas apparaître"
    );
    assert_eq!(score.measures.len(), 4, "mesures");
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
        ],
        "durées de la première portée"
    );

    // Cas limite : accolades imbriquées dans le markup, en fin de flux. Le
    // bloc entier doit être consommé, sans laisser échapper la dernière note.
    let source = "\\score { \\new Staff { \\fixed c' { c'4 d' | e'2 r' | \
         \\mark \\markup { \\box { \\bold { Z } } } c''1 } } }";
    let score = read_lilypond(source).expect("lecture imbriquée");
    assert_eq!(
        hauteurs_attaques(&score),
        vec![72, 74, 76, 84],
        "musique après markup imbriqué"
    );
    assert_eq!(score.measures.len(), 3, "mesures après markup imbriqué");

    // Cas non concerné : un `\mark` dont l'argument est une chaîne garde son
    // comportement, la musique qui suit restant intacte.
    let score = read_lilypond(
        "\\score { \\new Staff { \\fixed c' { c'4 \\mark \\markup \"A\" d'4 e'2 } } }",
    )
    .expect("lecture chaîne");
    assert_eq!(
        hauteurs_attaques(&score),
        vec![72, 74, 76],
        "musique après chaîne"
    );
    assert_eq!(score.measures.len(), 1, "mesures après chaîne");
}

#[test]
fn transposition_du_global_atteint_les_voix_de_la_portee() {
    // Cas réel (Carcassi, guitar-skole…) : `\transposition c` est posé dans
    // `\global`, frère des `\context Voice` d'un même `\context Staff`. Le
    // parseur en fait trois groupes : le global (sans note) porte la propriété,
    // les voix musicales doivent la recevoir. Sans cela, la concert reste écrite
    // d'une octave trop haut et l'export MusicXML ne porte aucun `<transpose>`.
    let source = r#"
      \version "2.24.0"
      \language "english"
      global = { \transposition c \clef treble \time 4/4 }
      haut = { c'4 d' e' f' | <g' c''>2 g'2 | r2 a'2 }
      bas = { c4 d4 e4 f4 | g2~ g2 | r2 c'2 }
      \score {
        \context Staff = "guitare" <<
          \global
          \context Voice = "haut" { \haut }
          \context Voice = "bas" { \bas }
        >>
      }
    "#;
    let score = read_lilypond(source).expect("lecture");
    // `\transposition c` fait sonner une octave plus bas : do4->do3 (48).
    assert_eq!(
        hauteurs(&score),
        vec![48, 50, 52, 53, 55, 60, 55, 57, 36, 38, 40, 41, 43, 43, 48]
    );
    assert_eq!(score.measures.len(), 3);
    let transposes: Vec<_> = score
        .parts
        .iter()
        .map(|part| part.instrument.transpose)
        .collect();
    // Une seule portée LilyPond = une seule partie MusicXML : la
    // transposition portée par le `\global` frère des voix s'applique à
    // cette partie, qui contient toutes les voix de la portée.
    assert_eq!(
        transposes,
        vec![Some(Transpose::new(-7, -12))],
        "la propriété doit suivre les voix musicales, pas le global vide"
    );

    // Cas non concerné : sans `\transposition`, hauteurs et parties inchangées.
    let sans = read_lilypond(&source.replace("\\transposition c ", "")).expect("lecture");
    assert_eq!(
        hauteurs(&sans),
        vec![60, 62, 64, 65, 67, 72, 67, 69, 48, 50, 52, 53, 55, 55, 60]
    );
    assert!(sans
        .parts
        .iter()
        .all(|part| part.instrument.transpose.is_none()));

    // Autre idiom réel (guitar-skole) : chaque voix de la portée porte
    // `\transposition c`. Toutes portant le même intervalle, la propriété est
    // posée sur l'unique partie et la concert est décalée d'une octave.
    let voix = read_lilypond(
        "\\score { \\new Staff { << \
         \\new Voice { \\transposition c \\fixed c' { c4 d | e4 f } } \
         \\new Voice { \\transposition c \\fixed c' { g4 a | b4 c' } } >> } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&voix), vec![48, 50, 52, 53, 55, 57, 59, 60]);
    assert_eq!(voix.measures.len(), 1, "les deux voix couvrent une mesure");
    assert_eq!(voix.parts.len(), 1);
    assert_eq!(
        voix.parts[0].instrument.transpose,
        Some(Transpose::new(-7, -12))
    );

    // Cas limite : une portée suivante, sans `\transposition`, ne doit pas
    // hériter de celle du groupe précédent.
    let deux = read_lilypond(&source.replace(
        "      }\n    ",
        "        \\new Staff { \\fixed c' { g4 a4 } }\n      }\n    ",
    ))
    .expect("lecture");
    assert_eq!(
        deux.parts.last().and_then(|part| part.instrument.transpose),
        None,
        "la transposition ne franchit pas un nouveau Staff"
    );
}

#[test]
fn drummode_et_drumstaff_jouent_des_percussions() {
    // Cas réel (hara-kiri-drumstaff.ly, midi-drums.ly) : les noms `bd`, `sn`,
    // `hh` ne sont pas des hauteurs de la langue mais des événements que
    // LilyPond joue dans son MIDI (tables de `ly/drumpitch-init.ly`).
    let source = r#"
      \version "2.24.0"
      \score {
        <<
          \new DrumStaff \drummode {
            bd4 sn8 hh16 hh sn2 | r4 bd4 bd2
          }
          \new Staff { c'4 d' e' f' | g'2 r2 }
        >>
      }
    "#;
    let score = read_lilypond(source).expect("lecture des percussions");
    assert_eq!(
        hauteurs(&score),
        vec![36, 38, 42, 42, 38, 36, 36, 60, 62, 64, 65, 67],
        "hauteurs MIDI des fûts puis de la portée ordinaire"
    );
    // `durees_ecrites` ne regarde que la piste 0 : la batterie, y compris le
    // silence `r4` de la seconde mesure.
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 8),
            Fraction::new(1, 16),
            Fraction::new(1, 16),
            Fraction::new(1, 2),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
        ],
        "durées de la piste de batterie (silence compris)"
    );
    assert_eq!(score.measures.len(), 2, "deux mesures communes");

    // Cas limite : un nom inconnu est consommé sans bloquer la suite.
    let inconnu = read_lilypond("\\score { \\new DrumStaff \\drummode { bd4 inconnu4 sn4 } }")
        .expect("nom inconnu");
    assert_eq!(hauteurs(&inconnu), vec![36, 38]);

    // Cas « non concerné » : sans \drummode, la portée ordinaire est
    // exactement la même (les mots de batterie n'y sont pas des notes).
    let ordinaire = read_lilypond("\\score { \\new Staff { c'4 d' e' f' | g'2 r2 } }")
        .expect("portée ordinaire");
    assert_eq!(hauteurs(&ordinaire), vec![60, 62, 64, 65, 67]);
    assert_eq!(hauteurs(&ordinaire), hauteurs(&score)[7..]);
}

/// `a2:m` : la durée est écrite avant la qualité (`chord_body` de
/// `lily/parser.yy`) ; elle ne doit pas faire perdre le `:m`.
#[test]
fn chordmode_qualite_apres_duree() {
    let score = lire("\\chordmode { a2:m a2 }");
    // `lire()` enveloppe la musique dans `\fixed c'` : la table des accords
    // est elle aussi décalée d'une octave (vérifié sur le MIDI de LilyPond
    // 2.24 pour `\fixed c' { \chordmode { a2:m a2 } }`).
    assert_eq!(hauteurs(&score), vec![81, 84, 88, 81, 85, 88]);
}

#[test]
fn chordmode_dans_une_portee_joue_les_accords() {
    // `\chordmode` est joué par LilyPond dans un Staff (chord-changes.ly),
    // contrairement à `\chords`/ChordNames qui reste une grille d'affichage.
    let source = r#"
      \version "2.24.0"
      \language "nederlands"
      grille = \chordmode { c2 c:m7 c/g }
      \score {
        <<
          \new Staff { \grille | }
          \new Staff { \relative c' { c4 d e f | g2 r2 } }
        >>
      }
    "#;
    let score = read_lilypond(source).expect("lecture de la grille jouée");
    // c = 60/64/67, c:m7 = 60/63/67/70, c/g = 60/64 + basse 55 (renversement).
    // La seconde portée est en do central (60, 62, 64, 65, 67).
    assert_eq!(
        hauteurs(&score),
        vec![60, 64, 67, 60, 63, 67, 70, 60, 64, 55, 60, 62, 64, 65, 67,],
        "notes des accords joués puis de la portée relative"
    );
    assert_eq!(score.measures.len(), 2, "deux mesures communes");
    // Une durée par accord sur la piste 0 (chaque accord joué dure une
    // blanche), y compris le silence de la seconde portée.
    assert_eq!(
        durees_ecrites(&score),
        vec![Fraction::new(1, 2); 3],
        "durées des accords de la piste 0"
    );

    // Cas limite : fin de flux, un accord seul sans durée explicite garde la
    // durée par défaut (noire).
    let seul = read_lilypond("\\score { \\new Staff \\chordmode { c:m } }")
        .expect("accord en fin de flux");
    assert_eq!(hauteurs(&seul), vec![60, 63, 67]);
    assert_eq!(durees_ecrites(&seul), vec![Fraction::new(1, 4)]);

    // Les deux idiomatiques `\new` et `\context` sous ChordNames jouent la
    // grille (vérifié contre le MIDI de LilyPond 2.26) : c = 60/64/67,
    // c:m = 60/63/67, puis la portée ordinaire.
    let jouee = read_lilypond(
        "\\score { << \\new ChordNames \\chords { c1 c:m } \
         \\new Staff { c'2 d' e' f' } >> }",
    )
    .expect("grille \\new ChordNames");
    assert_eq!(
        hauteurs(&jouee),
        vec![60, 64, 67, 60, 63, 67, 60, 62, 64, 65]
    );
    let contexte = read_lilypond(
        "grille = \\chordmode { c1 c:m }\n\
         \\score { << \\context ChordNames \\with { chordChanges = ##t } \\grille \
         \\new Staff { c'2 d' e' f' } >> }",
    )
    .expect("grille en \\context");
    assert_eq!(
        hauteurs(&contexte),
        vec![60, 64, 67, 60, 63, 67, 60, 62, 64, 65]
    );
}

/// `\ottava` ne transpose pas le MIDI de LilyPond (Notation Reference §1.3.3 :
/// « leaving the sounding pitch unchanged »). Le modèle garde la portée pour
/// l'écriture, mais `play` est faux : l'export MIDI ne doit pas décaler ces
/// notes. Le spanner couvre exactement les notes entre `\ottava #1` et
/// `\ottava #0` (la mesure de 4/4 commence au tick 0, chaque noire vaut 480).
#[test]
fn ottava_de_lilypond_est_une_convention_d_ecriture() {
    use scoretk_core::model::SpannerType;

    let score = read_lilypond(
        "\\version \"2.24.0\"\n\\language \"english\"\n\
         \\score { \\new Staff { \\fixed c' { c4 \\ottava #1 d4 e4 \\ottava #0 f4 } } }",
    )
    .expect("lecture \\ottava");
    let ottavas: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Ottava(_)))
        .collect();
    assert_eq!(ottavas.len(), 1, "une seule portée d'ottava");
    let ottava = ottavas[0];
    assert!(
        !ottava.play,
        "le MIDI LilyPond ne transpose pas sous \\ottava"
    );
    assert_eq!(ottava.start.tick.ticks(1920), 480, "début sur d4");
    assert_eq!(ottava.end.tick.ticks(1920), 1440, "fin après e4");
}

/// Notation Reference §22.2.2 : `\tag` marque une expression, `\keepWithTag`
/// ne garde que les expressions marquées d'un des tags nommés (les expressions
/// sans tag sont toujours gardées) et `\removeWithTag` retire celles qui portent
/// un des tags nommés. Le test porte sur une partition à quatre portées, avec
/// variables, `\relative`, accords, silences, liaisons et barres de contrôle.
#[test]
fn tags_filtrent_les_alternatives_en_musique_ordinaire() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\language "english"
motif = \fixed c' {
  \tag #'a { c4 d e f }
  \tag #'b { g4 a b c' }
  c'2 ~ c'2 |
}
rel = \relative c' { \tag #'a { c4 d } \tag #'b { e4 f } | }
\score {
  <<
    \new Staff { \keepWithTag #'a \motif }
    \new Staff { \removeWithTag #'a \motif }
    \new Staff { \keepWithTag #'a \rel }
    \new Staff { \motif }
    \new Staff { \keepWithTag #'(a b) \motif }
  >>
}
"#,
    )
    .expect("lecture des tags");
    // Une portée occupe une piste brute tous les quatre indices (quatre voix
    // par portée dans le modèle, `ids.rs`).
    let piste = |staff: u32| Track::new(StaffId::from_raw(staff), 0).raw();
    // Portée 1 : seul le tag `a`, plus la musique sans tag (la liaison).
    assert_eq!(
        hauteurs_par_accord(&score, piste(0)),
        vec![vec![60], vec![62], vec![64], vec![65], vec![72], vec![72]]
    );
    assert_eq!(
        durees_par_accord(&score, piste(0)),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
        ]
    );
    // Portée 2 : `\removeWithTag a` garde le tag `b` et la musique sans tag.
    assert_eq!(
        hauteurs_par_accord(&score, piste(1)),
        vec![vec![67], vec![69], vec![71], vec![72], vec![72], vec![72]]
    );
    // Portée 3 : le filtre s'applique aussi à une variable écrite en `\relative`.
    assert_eq!(
        hauteurs_par_accord(&score, piste(2)),
        vec![vec![60], vec![62]]
    );
    // Cas non concerné : sans filtre, les deux alternatives sont lues.
    assert_eq!(
        hauteurs_par_accord(&score, piste(3)),
        vec![
            vec![60],
            vec![62],
            vec![64],
            vec![65],
            vec![67],
            vec![69],
            vec![71],
            vec![72],
            vec![72],
            vec![72]
        ]
    );
    // Portée 5 : une liste de tags garde les deux alternatives (forme `#'(…)`).
    assert_eq!(
        hauteurs_par_accord(&score, piste(4)),
        vec![
            vec![60],
            vec![62],
            vec![64],
            vec![65],
            vec![67],
            vec![69],
            vec![71],
            vec![72],
            vec![72],
            vec![72]
        ]
    );
    assert_eq!(score.measures.len(), 3);
}

/// Notation Reference §22.2.2, « Using tag groups » : avec `\tagGroup`, un
/// `\keepWithTag` ne retire que les tags du même groupe que ceux qu'il garde.
/// Un tag sans groupe (ici `other`) n'est pas touché ; réciproquement garder un
/// tag sans groupe ne retire aucun des tags appartenant à un groupe.
#[test]
fn tag_group_filtre_selon_le_groupe_et_non_selon_le_nom() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\language "english"
music = \fixed c' {
  \tag violinI { c4 d }
  \tag violinII { e4 f }
  \tag other { g4 a }
  b4 c' |
}
\tagGroup violinI,violinII
\score {
  <<
    \new Staff { \keepWithTag violinI \music }
    \new Staff { \keepWithTag other \music }
  >>
}
"#,
    )
    .expect("lecture des groupes de tags");
    let piste = |staff: u32| Track::new(StaffId::from_raw(staff), 0).raw();
    assert_eq!(
        hauteurs_par_accord(&score, piste(0)),
        vec![vec![60], vec![62], vec![67], vec![69], vec![71], vec![72]]
    );
    assert_eq!(
        hauteurs_par_accord(&score, piste(1)),
        vec![
            vec![60],
            vec![62],
            vec![64],
            vec![65],
            vec![67],
            vec![69],
            vec![71],
            vec![72]
        ]
    );
    assert_eq!(score.measures.len(), 2);
}

/// Extending §1 : un `#(define x #{ … #})` est une variable musicale, insérée
/// par `$x`. `#(set-global-staff-size n)` est un réglage sans effet musical.
#[test]
fn variable_musicale_definie_en_scheme() {
    let (score, warnings) = read_lilypond_reporting(
        "\\version \"2.24.0\"\n\
         \\language \"english\"\n\
         #(set-global-staff-size 16)\n\
         #(define motif #{ c'4 d' e' f' #})\n\
         \\score { \\new Staff { \\fixed c' { $motif g'2 ~ g'2 | } } }\n",
    )
    .expect("lecture de la variable Scheme");
    // `#{ … #}` est absolu : c' y vaut 72 (do central + une octave), et le
    // `\fixed c'` environnant ne change pas ces hauteurs écrites.
    assert_eq!(hauteurs(&score), vec![72, 74, 76, 77, 79, 79]);
    assert_eq!(score.measures.len(), 2);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(1, 2),
        ]
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.message.contains("commande ignorée")),
        "avertissements : {warnings:?}"
    );
}

/// `\set Timing.measureLength = #(ly:make-moment n d)` : la forme à deux
/// entiers est une fraction, comme la forme `#(ly:make-moment n/d)` déjà lue.
#[test]
fn measurelength_moment_scheme_a_deux_entiers() {
    let score = lire("\\set Timing.measureLength = #(ly:make-moment 2 4) c4 d");
    assert_eq!(score.measures.len(), 1);
    assert_eq!(score.measures[0].len, Fraction::new(1, 2));
    assert_eq!(hauteurs(&score), vec![60, 62]);
}

/// `lily/parser.yy` `post_event_nofinger: direction_less_event` : `\fermata` ou
/// `\p` est un post-événement isolé, sans argument. Seule la règle
/// `script_dir music_function_call` consomme la commande qui suit, après
/// `^`, `_` ou `-`. Un `\times`/`\tuplet` suivant un post-événement reste donc
/// de la musique et doit produire son n-olet.
#[test]
fn post_event_ne_consomme_pas_le_times_suivant() {
    let source = r#"\version "2.24.0"
\language "english"
mus = \fixed c' {
  \time 2/4
  c4 d4 | e4\fermata \times 2/3 { f8 g8 a8 } |
  b4\p \tuplet 3/2 { c'8 d'8 e'8 } | <f' a' c''>4 ~ <f' a' c''>4
}
bas = \fixed c {
  \time 2/4
  c4\fermata \times 2/3 { d8 e8 f8 } | g2 r2
}
\score { << \new Staff \mus \new Staff \bas >> }"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 4, "mesures");
    assert_eq!(
        score.tuplets.len(),
        3,
        "les deux \\times et le \\tuplet après post-événement"
    );
    assert!(score
        .tuplets
        .iter()
        .all(|tuplet| tuplet.ratio == Fraction::new(3, 2)));
    let tuplets: Vec<_> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter(|chord_rest| chord_rest.tuplet.is_some())
        .map(|chord_rest| {
            (
                chord_rest
                    .chord_ref()
                    .map(|chord| chord.notes.clone())
                    .unwrap_or_default(),
                chord_rest.duration.ticks(),
            )
        })
        .collect();
    let notes_en_n_olet: Vec<(u8, Fraction)> = tuplets
        .into_iter()
        .flat_map(|(notes, duree)| notes.into_iter().map(move |id| (id, duree)))
        .map(|(id, duree)| (score.note(id).pitch, duree))
        .collect();
    assert_eq!(
        notes_en_n_olet,
        vec![
            (65, Fraction::new(1, 8)),
            (67, Fraction::new(1, 8)),
            (69, Fraction::new(1, 8)),
            (72, Fraction::new(1, 8)),
            (74, Fraction::new(1, 8)),
            (76, Fraction::new(1, 8)),
        ],
        "hauteurs et durées écrites des n-olets de la première portée"
    );

    // Cas limite : post-événement en fin de musique, sans jeton à suivre.
    let fin = lire("c4 \\fermata");
    assert_eq!(hauteurs(&fin), vec![60]);
    assert_eq!(fin.measures.len(), 1);
    assert!(fin.tuplets.is_empty());

    // Cas non concerné : `\times` sans post-événement devant est inchangé.
    let seul = lire("\\times 2/3 { c8 d8 e8 }");
    assert_eq!(hauteurs(&seul), vec![60, 62, 64]);
    assert_eq!(seul.tuplets.len(), 1);
}

/// `lily/parser.yy` : une commande sans argument (`command_element`) ne prend
/// aucun jeton ; seuls les `music_function_call` en ont. `r`, `R` et `s` sont
/// des noms de silence (de la musique), jamais des arguments. L'heuristique
/// `skip_command_arguments` ne doit donc pas les avaler après `\autoBeamOff`,
/// `\oneVoice`, `\dynamicUp`…, sinon le silence disparaît et tout le reste de
/// la portée est décalé d'une durée.
#[test]
fn commande_sans_argument_ne_consomme_pas_un_silence() {
    let source = r#"\version "2.24.0"
\language "english"
haut = \fixed c' {
  \time 4/4 \partial 4
  \autoBeamOff r8 f8 | \oneVoice R1 | g2~ g2
}
bas = \fixed c {
  \time 4/4 \partial 4
  \dynamicUp s4 c4 | \oneVoice r2 d2
}
\score { << \new Staff \haut \new Staff \bas >> }"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 3, "mesures");
    assert_eq!(
        score.measures[0].len,
        Fraction::new(1, 4),
        "levée d'un quart"
    );
    assert_eq!(hauteurs_attaques(&score), vec![65, 67, 48, 50]);
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 8), // r8
            Fraction::new(1, 8), // f8
            Fraction::ZERO,      // R1 : silence de mesure entière
            Fraction::new(1, 2),
            Fraction::new(1, 2),
        ],
        "durées de la première portée"
    );

    // Cas limite : commande sans argument en fin de musique.
    let fin = lire("c4 \\autoBeamOff");
    assert_eq!(hauteurs_attaques(&fin), vec![60]);
    assert_eq!(fin.measures.len(), 1);

    // Cas non concerné : la note suivant la commande reste lue.
    let note = lire("\\autoBeamOff c4 d4");
    assert_eq!(hauteurs_attaques(&note), vec![60, 62]);
}

/// Notation Reference §2.2.2 « Invisible rests » : avec un silence invisible
/// `s4`, la durée explicite devient celle des notes suivantes ; avec
/// `\skip 4` (durée **numérique**), elle est ignorée pour la durée par défaut,
/// « to preserve backward compatibility with the legacy implementation ».
#[test]
fn skip_numerique_garde_la_duree_precedente() {
    let source = r#"\version "2.24.0"
\language "english"
haut = \fixed c' {
  \time 4/4
  c32 d32 e32 f32 \skip 4 g32 a32 b32 c'32 | d'2
}
bas = \fixed c {
  \time 4/4
  c32 d32 s4 e32 f32 | g2
}
\score { << \new Staff \haut \new Staff \bas >> }"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.measures.len(), 1, "mesures");
    assert_eq!(
        hauteurs(&score),
        vec![
            60, 62, 64, 65, 67, 69, 71, 72, 74, // haut
            48, 50, 52, 53, 55, // bas
        ]
    );
    assert_eq!(
        durees_ecrites(&score),
        vec![
            Fraction::new(1, 32), // c32
            Fraction::new(1, 32), // d32
            Fraction::new(1, 32), // e32
            Fraction::new(1, 32), // f32
            Fraction::new(1, 32), // g32 : `\skip 4` n'a rien changé
            Fraction::new(1, 32), // a32
            Fraction::new(1, 32), // b32
            Fraction::new(1, 32), // c'32
            Fraction::new(1, 2),  // d'2
        ],
        "durées de la première portée"
    );
    // Contraste : sur une portée seule, `s4` change bien la durée par défaut.
    let spacer = lire("c32 d32 s4 e f");
    assert_eq!(hauteurs(&spacer), vec![60, 62, 64, 65]);
    assert_eq!(
        durees_ecrites(&spacer),
        vec![
            Fraction::new(1, 32),
            Fraction::new(1, 32),
            Fraction::new(1, 4), // e32 -> quart
            Fraction::new(1, 4), // f32
        ]
    );

    // Cas limite : multiplicateur nul sur `\skip`, sans panique ni note.
    let nul = lire("c32 d32 \\skip 0 e32");
    assert_eq!(hauteurs(&nul), vec![60, 62, 64]);
    assert_eq!(durees_ecrites(&nul), vec![Fraction::new(1, 32); 3]);
}

/// Notation Reference §4.2.2 « Tremolo repeats » : la durée totale d'un
/// `\repeat tremolo n { a b }` vaut n fois celle du corps. Le MIDI de LilyPond
/// 2.26 joue deux notes tenues, chacune de durée `n × écrite`, et non les notes
/// du corps répétées : vérifié pour n = 2, 4, 6, 8 sur `c'16 d'16`.
#[test]
fn repeat_tremolo_etire_les_durees() {
    let score = lire("\\time 4/4 \\repeat tremolo 4 { c16 d16 } e2");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert_eq!(score.measures.len(), 1, "mesures");
    let reelles: Vec<Fraction> = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .map(|chord_rest| chord_rest.ticks.unwrap_or(chord_rest.duration.ticks()))
        .collect();
    assert_eq!(
        reelles,
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 4),
            Fraction::new(1, 2)
        ],
        "chaque note du trémolo dure 4 × une double croche"
    );

    // Cas limite : facteur 1, sans changement de durée.
    let un = lire("\\repeat tremolo 1 { c8 d8 } e4");
    assert_eq!(hauteurs(&un), vec![60, 62, 64]);
    assert_eq!(
        durees_ecrites(&un),
        vec![
            Fraction::new(1, 8),
            Fraction::new(1, 8),
            Fraction::new(1, 4)
        ]
    );
}

/// `ly/music-functions-init.ly:1274` : `\octaveCheck` est une fonction
/// musicale qui consomme une **hauteur** (`ly:pitch?`) et ne produit aucune
/// note ni durée (`RelativeOctaveCheck`). Sans consommer l'argument, la
/// hauteur était relue comme une note et décalait la référence `\relative`.
#[test]
fn octave_check_reussi_consomme_sa_hauteur() {
    let score = lire("\\relative c' { c4 \\octaveCheck c' d e }");
    assert_eq!(hauteurs(&score), vec![60, 62, 64]);
    assert_eq!(score.measures.len(), 1);
}

#[test]
fn octave_check_echoue_decale_la_reference() {
    // `lily/relative-octave-check.cc:33-60` : si la hauteur de contrôle n'est
    // pas à l'octave obtenue, la référence est décalée de delta_oct ; le MIDI
    // de LilyPond joue d/e une octave plus haut.
    let score = lire("\\relative c' { c4 \\octaveCheck c'' d e }");
    assert_eq!(hauteurs(&score), vec![60, 74, 76]);
    assert_eq!(score.measures.len(), 1);
}

/// `lily/percent-repeat-iterator.cc` : `\repeat percent n { … }` joue le
/// corps une fois, puis émet n−1 `PercentEvent` de la longueur du corps : le
/// temps avance de n × la durée du corps, mais les répétitions ne sont pas
/// rejouées. Le lecteur les ignorait entièrement et plaçait la suite trop tôt.
#[test]
fn repeat_percent_avance_les_repetitions() {
    let score = lire("\\time 4/4 \\repeat percent 3 { c4 d e f } g1");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
    assert_eq!(
        score.measures.len(),
        4,
        "les deux répétitions de pourcent occupent deux mesures"
    );
}

#[test]
fn repeat_percent_sous_unfold_repeats_est_joue() {
    // `\unfoldRepeats` transforme `PercentRepeatedMusic` en
    // `UnfoldedRepeatedMusic` (`scm/music-functions.scm`, `unfold-repeats`).
    let score = lire("\\unfoldRepeats \\repeat percent 3 { c4 d e f } g1");
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65, 60, 62, 64, 65, 60, 62, 64, 65, 67]
    );
    assert_eq!(score.measures.len(), 4);
}

#[test]
fn repeat_tremolo_sous_unfold_repeats_est_joue() {
    // `\unfoldRepeats` transforme `TremoloRepeatedMusic` en
    // `UnfoldedRepeatedMusic` : le corps est rejoué n fois avec ses durées.
    let score = lire("\\unfoldRepeats \\repeat tremolo 4 { c16 d16 } e2");
    assert_eq!(hauteurs(&score), vec![60, 62, 60, 62, 60, 62, 60, 62, 64]);
    assert_eq!(score.measures.len(), 1);
}

/// `lily/parser.yy` `context_prefix` (l. 1686-1690) : `\new Voice` crée une
/// voix **dans le Staff courant**, pas une portée sœur. Les contextes Voice
/// meurent à la fin de leur musique : quatre `\new Voice` successifs peuvent
/// réutiliser le même emplacement de voix. Aucune note ne doit être perdue.
/// `\afterGrace` : le modèle porte l'instant de lecture de l'ornement après
/// la note (`afterGraceFraction` = 3/4 par défaut,
/// `ly/music-functions-init.ly:86-115`). Le MusicXML l'écrit en `<offset>`.
/// `make-volta-set` (`scm/define-music-callbacks.scm`) renvoie le corps puis
/// un `SequentialAlternativeMusic` : la deuxième alternative est relative à la
/// fin de la première, pas à la fin du corps.
#[test]
fn alternative_relative_a_la_fin_de_la_precedente() {
    let score = read_lilypond(
        "\\relative c' { \\repeat volta 2 { c'4 } \\alternative { { c,,4 } { c4 } } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![72, 48, 48]);
}

#[test]
fn alternative_relative_sequentielle_sous_unfold() {
    let score = read_lilypond(
        "\\score { \\new Staff { \\unfoldRepeats \\relative c' { \\repeat volta 2 { c'4 } \\alternative { { c,,4 } { c4 } } } } }",
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![72, 48, 72, 48]);
}

#[test]
fn after_grace_porte_son_instant_de_lecture() {
    let score = lire("c4 \\afterGrace d4 { e16 }");
    assert_eq!(offset_grace_apres(&score), Some(Fraction::new(3, 16)));
}

#[test]
fn after_grace_fraction_optionnelle() {
    // `\\afterGrace 7/8 c1 { d16 }` : l'argument optionnel est une échelle
    // (ly/music-functions-init.ly:86-115).
    let score = lire("c4 \\afterGrace 7/8 d4 { e16 }");
    assert_eq!(offset_grace_apres(&score), Some(Fraction::new(7, 32)));
}

#[test]
fn after_grace_fraction_par_defaut_assignee() {
    let score = read_lilypond(
        "afterGraceFraction = 15/16 \\score { \\new Staff { c4 \\afterGrace d4 { e16 } } }",
    )
    .expect("lecture");
    assert_eq!(offset_grace_apres(&score), Some(Fraction::new(15, 64)));
}

#[test]
fn after_grace_fraction_par_defaut_scheme() {
    let score = read_lilypond(
        "#(define afterGraceFraction (cons 15 16)) \\score { \\new Staff { c4 \\afterGrace d4 { e16 } } }",
    )
    .expect("lecture");
    assert_eq!(offset_grace_apres(&score), Some(Fraction::new(15, 64)));
}

#[test]
fn new_voice_reuse_un_emplacement_de_voix() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \context Staff = "up" {
  \new Voice { c4 d4 }
  \new Voice { e4 f4 }
  \new Voice { g4 a4 }
  \new Voice { b4 c'4 }
} }"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(score.staves.len(), 1, "une seule portée");
    assert_eq!(
        hauteurs(&score),
        vec![48, 50, 52, 53, 55, 57, 59, 60],
        "toutes les notes sont conservées"
    );
    let pistes: Vec<u32> = (0..16)
        .filter(|track| {
            score.measures.iter().any(|measure| {
                measure.segments.iter().any(|segment| {
                    segment
                        .chord_rest(Track::from_raw(*track))
                        .is_some_and(|chord_rest| chord_rest.chord_ref().is_some())
                })
            })
        })
        .collect();
    // réutilisation des emplacements : une seule piste suffit.
    assert_eq!(pistes.len(), 1, "pistes occupées : {pistes:?}");

    // Cas non concerné : deux `\new Staff` restent deux portées distinctes.
    let deux = read_lilypond("\\score { << \\new Staff { c'4 } \\new Staff { e'4 } >> }")
        .expect("lecture");
    assert_eq!(deux.staves.len(), 2);
    assert_eq!(hauteurs(&deux), vec![60, 64]);
}

/// `Timing` est un alias de `Score` (`ly/engraver-init.ly`) : il n'y a qu'une
/// grille de mesures, et `\partial` (`ly/music-functions-init.ly`, `PartialSet`)
/// décale la position dans cette grille pour toutes les portées. Un `R4.` qui
/// franchit la barre de la levée continue simplement après celle-ci : le MIDI
/// de LilyPond joue les notes du haut à 0,5/1/1,5/2 noires et celle du bas à
/// 6 noires. Les éléments qui franchissent une barre doivent être découpés.
#[test]
fn elements_a_travers_les_barres_sont_decoupes() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \new PianoStaff <<
  \new Staff { \time 3/8 \partial 8 r8 c'8 d'8 e'8 | f'4. }
  \new Staff { \time 3/8 R4. R4. R4. R4. c4. }
>> }"#;
    let score = read_lilypond(source).expect("lecture");
    let mut notes: Vec<(u8, Fraction)> = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            for track in 0..8 {
                if let Some(chord_rest) = segment.chord_rest(Track::from_raw(track)) {
                    if let Some(chord) = chord_rest.chord_ref() {
                        for id in &chord.notes {
                            if score.note(*id).tie_back.is_none() {
                                notes.push((score.note(*id).pitch, segment.tick));
                            }
                        }
                    }
                }
            }
        }
    }
    notes.sort();
    notes.dedup();
    assert_eq!(
        notes,
        vec![
            (48, Fraction::new(3, 2)), // bas : 6 noires
            (60, Fraction::new(1, 8)), // haut : 0,5 noire
            (62, Fraction::new(1, 4)),
            (64, Fraction::new(3, 8)),
            (65, Fraction::new(1, 2)),
        ],
        "attaques absolues des cinq notes"
    );

    // Le `R4.` du bas traverse la barre de la levée : il doit être découpé en
    // silences explicites, sinon l'écrivain MusicXML le tronque à 0,5 noire.
    let bas: Vec<_> = (0..8)
        .flat_map(|track| {
            score.measures.iter().flat_map(move |measure| {
                measure.segments.iter().filter_map(move |segment| {
                    segment
                        .chord_rest(Track::from_raw(track))
                        .filter(|rest| rest.chord_ref().is_none())
                        .map(|rest| (track, rest))
                })
            })
        })
        .filter(|(track, _)| *track == 4)
        .map(|(_, rest)| (rest.duration.ticks(), rest.is_full_measure_rest()))
        .collect();
    assert!(
        bas.len() >= 4,
        "silences du bas découpés à chaque barre : {bas:?}"
    );
    assert!(
        bas.iter()
            .all(|(ticks, full)| *ticks > Fraction::ZERO && !*full),
        "chaque morceau porte sa durée explicite : {bas:?}"
    );
}

/// Les morceaux découpés à la barre portent une figure exacte, décomposée
/// comme `toDurationList` (`durationtype.cpp`, porté dans
/// `scoretk-midi-import::meter::to_duration_list`) : 5/16 = noire + double
/// croche, 3/8 = noire pointée. Aucune figure approximative, aucun plancher.
#[test]
fn silences_decoupes_en_figures_exactes() {
    let durees = |src: &str| -> Vec<Fraction> {
        let score = lire(src);
        score
            .measures
            .iter()
            .flat_map(|measure| measure.segments.iter())
            .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
            .filter(|rest| rest.chord_ref().is_none())
            .map(|rest| rest.duration.ticks())
            .collect()
    };
    // Cinq croches (2,5 noires) puis R2. (noire pointée = 3 noires) : deux
    // morceaux de 3/8 de ronde, chacun une noire pointée.
    assert_eq!(
        durees("\\time 4/4 c8 c8 c8 c8 c8 R2. c4"),
        vec![Fraction::new(3, 8), Fraction::new(3, 8)]
    );
    // Début à 2,75 noires : 5/16 = noire + double croche, puis 7/16 =
    // noire + noire pointée.
    assert_eq!(
        durees("\\time 4/4 c8 c8 c8 c8 c8 c16 R2. c4"),
        vec![
            Fraction::new(1, 4),
            Fraction::new(1, 16),
            Fraction::new(7, 16), // double croche pointée deux fois
        ]
    );
}

/// `ly/music-functions-init.ly:2470` : `\tuplet ratio [span] music` prend une
/// durée optionnelle de regroupement ; même sans elle, la musique est
/// compressée par `ratio`. Le lecteur doit consommer `8` avant le corps,
/// sinon la durée nue devient une note parasite et le n-olet disparaît.
#[test]
fn tuplet_avec_duree_de_regroupement() {
    let score = lire("\\tuplet 3/2 8 { c8 d8 e8 f8 g8 a8 } b2");
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65, 67, 69, 71],
        "aucune note parasite issue de la durée 8"
    );
    assert_eq!(score.tuplets.len(), 1, "un n-olet 3/2");
    assert_eq!(score.tuplets[0].ratio, Fraction::new(3, 2));
    let notes_avec_n_olet = score
        .measures
        .iter()
        .flat_map(|measure| measure.segments.iter())
        .filter_map(|segment| segment.chord_rest(Track::from_raw(0)))
        .filter(|chord_rest| chord_rest.tuplet.is_some())
        .count();
    assert_eq!(notes_avec_n_olet, 6);
    assert_eq!(score.measures.len(), 1, "mesures");

    // Cas non concerné : sans durée de regroupement, même comportement.
    let simple = lire("\\tuplet 3/2 { c8 d8 e8 } f2");
    assert_eq!(hauteurs(&simple), vec![60, 62, 64, 65]);
    assert_eq!(simple.tuplets.len(), 1);
}

/// LilyPond n'écrit qu'un MIDI : celui du `\score` qui porte `\midi` sans
/// `\layout` (NR §24.3). Quand un tel score accompagne le score gravé et joue
/// la même musique sans `\transpose`, c'est lui qui fait le modèle : la
/// référence MIDI est à hauteur de concert, pas transposée.
#[test]
fn score_midi_seul_choisi_pour_le_modele() {
    let source = r#"\version "2.24.0"
\language "english"
mus = { c'4 d' e' f' }
\score { \transpose c d { \mus } \layout { ragged-right = ##t } }
\score { { \mus } \midi { \tempo 4 = 100 } }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![60, 62, 64, 65],
        "hauteurs de concert du score MIDI (pas transposées)"
    );
    assert_eq!(score.measures.len(), 1);
    assert_eq!(durees_ecrites(&score), vec![Fraction::new(1, 4); 4]);

    // Cas non concerné : sans second score MIDI, le score gravé fait foi.
    let seul =
        read_lilypond("\\score { \\transpose c d { c'4 d' } \\layout {} }").expect("lecture");
    assert_eq!(hauteurs(&seul), vec![62, 64]);
}

/// Quand LilyPond rencontre plusieurs `\score` et qu'un seul porte `\midi`
/// sans `\layout`, c'est ce score qu'il joue (NR §24.3) : même si sa musique
/// diffère du score gravé, c'est elle que décrit la référence MIDI.
#[test]
fn score_midi_seul_choisi_meme_si_musique_differente() {
    let source = r#"\version "2.24.0"
\language "english"
\score { { c'4 d' } \layout {} }
\score { { e'4 f' g' a' } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![64, 65, 67, 69]);
    assert_eq!(score.measures.len(), 1);
    assert_eq!(durees_ecrites(&score), vec![Fraction::new(1, 4); 4]);
    assert_eq!(score.playback_transposition, None);
}

/// NR §1.1.3/§1.3.3 : `\clef "treble_8"` / `"G_8"` ne change que
/// l'affichage, jamais la hauteur MIDI. Le décalage d'octave observé sur
/// `AStMichel`/`22_StMichel` venait du choix du score gravé transposé, pas
/// de la clé.
#[test]
fn clef_treble_8_ne_change_pas_la_hauteur() {
    for clef in ["treble_8", "G_8", "treble"] {
        let src = format!("\\clef \"{clef}\" c'4 d'4");
        let score = lire(&src);
        assert_eq!(hauteurs(&score), vec![72, 74], "clé {clef}");
    }
}

/// Quand la partition gravée porte elle-même `\\midi`, LilyPond écrit son MIDI
/// en premier : la référence est donc transposée, et le modèle doit suivre la
/// partition gravée, pas un `\\score` MIDI ultérieur.
#[test]
fn score_grave_avec_midi_preval_sur_les_scores_midi_suivants() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \transpose c d { c'4 d' } \layout {} \midi {} }
\score { { e'4 f' } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(hauteurs(&score), vec![62, 64], "score gravé transposé");
    assert_eq!(score.measures.len(), 1);

    // Cas non concerné : sans `\\midi` sur le score gravé, le score MIDI seul
    // fait le modèle (règle NR §24.3).
    let seul = read_lilypond(
        r#"\version "2.24.0"
\language "english"
\score { \transpose c d { c'4 d' } \layout {} }
\score { { e'4 f' } \midi {} }
"#,
    )
    .expect("lecture");
    assert_eq!(hauteurs(&seul), vec![64, 65]);
}

/// `\\keepWithTag` filtre le résultat d'un `\\relative` : la conversion
/// relative parcourt **toutes** les branches, y compris celles que le filtre
/// retire ensuite. Le lecteur filtrait d'abord, changeant l'octave de la
/// branche retenue (Chop-28-16, NR §22.2.2).
#[test]
fn keepwithtag_apres_relative_pour_la_reference_octave() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \keepWithTag #'midi \relative {
  \tag #'layout { <ef' a gf'>4 }
  \tag #'midi { <ef a gf'>4 }
} }"#;
    // Valeurs attestées par une compilation LilyPond 2.26 isolée :
    // 63/69/78 (mi bémol 4, la 4, fa dièse 5).
    let score = read_lilypond(source).expect("lecture");
    let notes: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    assert_eq!(notes, vec![63, 69, 78], "octave issue de la branche layout");
}

/// Les branches `<< … \\ \new Voice … >>` d'un `\context Staff` qui sont
/// enchâssées dans un enveloppeur (`\fixed`, `\relative`) sont des passages
/// polyphoniques temporaires : LilyPond les joue sur la même portée en
/// réutilisant les emplacements de voix, et leur musique succède dans le
/// temps. Ouvrir une portée par branche déplace toutes les branches au tick 0.
#[test]
fn passages_temporaires_dans_un_contexte_staff_restent_sur_une_portee() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score {
  \context Staff = "up" <<
    \key c \major
    \time 4/4
    \fixed c' {
      << { c4 d4 e4 f4 } \\ \new Voice { g4 a4 b4 c'4 } >>
      << { c4 d4 e4 f4 } \\ \new Voice { g4 a4 b4 c'4 } >>
      << { c4 d4 e4 f4 } \\ \new Voice { g4 a4 b4 c'4 } >>
      << { c4 d4 e4 f4 } \\ \new Voice { g4 a4 b4 c'4 } >>
    }
  >>
}
"#,
    )
    .expect("lecture");
    assert_eq!(
        score.staves.len(),
        1,
        "quatre passages temporaires successifs tiennent sur une portée"
    );
    assert_eq!(score.measures.len(), 4);
    assert_eq!(score.notes.len(), 32);
}

/// Un `s` étiqueté retiré par `\keepWithTag` ne réserve aucun temps :
/// LilyPond filtre l'arbre avant de le faire jouer, et le `s` conservé
/// dans l'autre branche fixe seul l'attaque suivante. Notation Reference
/// §2.2.2 et §22.2.2 ; vérifié sur une source réduite compilée par
/// LilyPond 2.26 (c'4, `s2` étiqueté layout retiré, `s2` midi gardé, d'4 :
/// attaques 0 et 3).
#[test]
fn skip_etiquete_filtre_ne_consomme_pas_de_temps() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score {
  \keepWithTag #'midi
  \new Staff {
    \time 4/4
    c'4
    \tag #'layout { s2 }
    \tag #'midi { s2 }
    d'4
  }
}
"#,
    )
    .expect("lecture");
    assert_eq!(
        accords_de_voix(&score, 0),
        vec![
            (Fraction::ZERO, vec![60], Fraction::new(1, 4)),
            (Fraction::new(3, 4), vec![62], Fraction::new(1, 4)),
        ]
    );
}

/// Garde-fou du index tick → segment utilisé par `link_ties` : une liaison
/// doit être retrouvée quel que soit le nombre de mesures qui la précèdent.
#[test]
fn liaisons_indexees_sur_beaucoup_de_mesures() {
    let music = "c4~ c4 d4 e4 | ".repeat(60) + "c4~ c4 d4 e4";
    let score = lire(&music);
    assert_eq!(score.measures.len(), 61);
    let ties = liaisons(&score);
    assert_eq!(ties.len(), 61);
    assert!(ties.iter().all(|(from, to)| from == to));
    assert_eq!(
        score
            .notes
            .iter()
            .filter(|note| note.tie_back.is_some())
            .count(),
        61
    );
}

/// Même cause que les passages temporaires enchâssés, mais avec un contenu
/// séquentiel (`\clef`, `\time`, puis plusieurs `<< … \new Voice … >>`) :
/// LilyPond 2.26 joue ces trois passages sur la portée unique, aux ticks 0, 4
/// et 8 ; les voix enchâssées ne doivent pas ouvrir de portées sœurs.
#[test]
fn passages_temporaires_sequenciels_dans_un_contexte_staff() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score {
  \context PianoStaff <<
    \context Staff = "up" {
      \clef treble
      \time 4/4
      << { R1 } \\ \new Voice { c'4 d'4 e'4 f'4 } >>
      << { R1 } \\ \new Voice { g'4 a'4 b'4 c''4 } >>
      << { R1 } \\ \new Voice { d''4 e''4 f''4 g''4 } >>
      << { R1 } \\ \new Voice { a''4 b''4 c'''4 d'''4 } >>
    }
    \context Staff = "down" { \clef bass c4 c4 c4 c4 }
  >>
}
"#,
    )
    .expect("lecture");
    assert_eq!(score.staves.len(), 2, "PianoStaff à deux portées");
    assert_eq!(score.measures.len(), 4);
    assert_eq!(score.notes.len(), 20);
}

/// Règle LilyPond : un `\new Staff` est **une** portée ; les voix ouvertes par
/// `\\` ou `\new Voice` sont réparties sur ses quatre pistes à leur tick
/// d'ouverture, et une voix terminée libère sa piste. Deux passages
/// polyphoniques successifs doivent donc donner deux voix de quatre notes
/// chacune ( ticks 0–3 ), et non quatre voix de deux notes toutes au tick 0.
#[test]
fn une_portee_par_contexte_voix_ouvertes_a_leur_tick() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score { \new Staff {
  << { c'4 d'4 } \\ { e'4 f'4} >>
  << { g'4 a'4 } \\ { b'4 c''4} >>
}
"#,
    )
    .expect("lecture");
    assert_eq!(score.staves.len(), 1);
    let voix0 = accords_de_voix(&score, 0);
    let voix1 = accords_de_voix(&score, 1);
    assert_eq!(voix0.len(), 4, "voix principale : {:?}", voix0);
    assert_eq!(voix1.len(), 4, "voix sœur : {:?}", voix1);
    let ticks: Vec<_> = voix0.iter().map(|(tick, _, _)| *tick).collect();
    assert_eq!(
        ticks,
        vec![
            Fraction::ZERO,
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(3, 4)
        ]
    );
}

/// Un accord qui franchit une barre doit être placé dans la mesure où il
/// commence, sans allonger cette mesure : le lecteur ignore les `|` mais la
/// trame de mesures reste celle du `\time`. Le quatrième accord de
/// `c'2 b4 g'2 ees2` attaque donc au tick 5/4, pas 3/2.
#[test]
fn accord_a_cheval_sur_une_barne_pas_de_calage_supplementaire() {
    let score = read_lilypond("\\score { \\new Staff { c'2 b4 g'2 ees2 } }").expect("lecture");
    assert_eq!(score.measures.len(), 2);
    assert_eq!(score.measures[0].len, Fraction::ONE);
    let notes = accords_de_voix(&score, 0);
    assert_eq!(
        notes.iter().map(|(tick, _, _)| *tick).collect::<Vec<_>>(),
        vec![
            Fraction::ZERO,
            Fraction::new(1, 2),
            Fraction::new(3, 4),
            Fraction::new(1, 1),
            Fraction::new(5, 4),
        ]
    );
}

/// `R1*12/8*4` porte deux multiplicateurs : LilyPond 2.26 place la note
/// suivante au tick 24 (en noires), soit 6 mesures de 12/8. Le lecteur
/// n'en lisait qu'un et plaçait la note au tick 6.
#[test]
fn multiplicateurs_de_duree_enchaines() {
    let score = read_lilypond("\\score { \\new Staff { \\time 12/8 R1*12/8*4 c'4 d4 e4 } }")
        .expect("lecture");
    let notes = accords_de_voix(&score, 0);
    assert_eq!(
        notes.iter().map(|(tick, _, _)| *tick).collect::<Vec<_>>(),
        vec![
            Fraction::new(6, 1),
            Fraction::new(25, 4),
            Fraction::new(13, 2),
        ]
    );
}

/// `\transposition` est une propriété de Staff : dans un groupe de plusieurs
/// portées elle ne doit s'appliquer qu'à la portée qui la porte. LilyPond 2.26
/// joue ici do4/ré4/mi4/fa4 sur la première portée et la2/si2/do3/ré3 sur la
/// seconde (`\transposition a`, `lily/audio-item.cc` instrumentTransposition).
#[test]
fn transposition_sur_une_seule_portee_dun_groupe() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score {
  \new StaffGroup <<
    \new Staff { \fixed c' { c4 d4 e4 f4 } }
    \new Staff { \transposition a \fixed c' { c4 d4 e4 f4 } }
  >>
}
"#,
    )
    .expect("lecture");
    assert_eq!(score.staves.len(), 2);
    let mut notes = hauteurs(&score);
    notes.sort_unstable();
    assert_eq!(notes, vec![57, 59, 60, 61, 62, 62, 64, 65]);
}

/// `\ottava #1` ne transpose pas les hauteurs MIDI : LilyPond 2.26 joue
/// do4/ré4/mi4/fa4/sol4, l'ottava ne change que la notation (`make-ottava-set`,
/// `scm/music-functions.scm`, `lily/ottava-bracket.cc`).
#[test]
fn ottava_ne_change_pas_les_hauteurs_midi() {
    let score =
        read_lilypond("\\score { \\new Staff { c'4 \\ottava #1 d'4 e'4 f'4 \\ottava #0 g'4 } }")
            .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64, 65, 67]);
}
/// La musique qui suit `<< … \\ … >>` continue avec la durée par défaut
/// posée par la **dernière** branche (`lily/music-sequence.cc`,
/// `simultaneous_relative_callback`). Ici `a'2` la fixe à la blanche : la
/// dernière note attaque au tick 6 (en noires), pas 5.
#[test]
fn duree_par_defaut_apres_passage_polyphonique_suit_derniere_branche() {
    let score = read_lilypond(
        "\\score { \\new Staff { \\time 4/4 << { c'4 d'4 e'4 f'4 } \\\\ { g'2 a'2 } >> b' c'' } }",
    )
    .expect("lecture");
    let notes = accords_de_voix(&score, 0);
    assert_eq!(
        notes.iter().map(|(tick, _, _)| *tick).collect::<Vec<_>>(),
        vec![
            Fraction::ZERO,
            Fraction::new(1, 4),
            Fraction::new(1, 2),
            Fraction::new(3, 4),
            Fraction::new(1, 1),
            Fraction::new(3, 2),
        ]
    );
}

/// La musique qui suit un `\new Voice` continue avec la durée par défaut
/// posée dans cette voix (`lily/parser.yy` `NEWCONTEXT` : le contexte est un
/// élément de la séquence). Ici `d'2 e'2` la fixe à la blanche : la dernière
/// note attaque au tick 7 (en noires), pas 6. Les ticks 4 et 8 viennent des
/// accord coupés aux barres.
#[test]
fn duree_par_defaut_apres_un_new_voice() {
    let score =
        read_lilypond("\\score { \\new Staff { \\time 4/4 c'4 \\new Voice { d'2 e'2 } f' g' } }")
            .expect("lecture");
    let mut ticks: Vec<Fraction> = (0..8)
        .flat_map(|voice| accords_de_voix(&score, voice))
        .filter(|(_, notes, _)| !notes.is_empty())
        .map(|(tick, _, _)| tick)
        .collect();
    ticks.sort();
    ticks.dedup();
    assert_eq!(
        ticks,
        vec![
            Fraction::ZERO,
            Fraction::new(1, 4),
            Fraction::new(3, 4),
            Fraction::ONE,
            Fraction::new(5, 4),
            Fraction::new(7, 4),
            Fraction::new(2, 1),
        ]
    );
}

/// `\transposition` est une propriété de Staff posée dans le temps
/// (`lily/audio-item.cc` instrumentTransposition) : elle ne s'applique qu'aux
/// notes écrites **après** elle. Sur `c'4 \transposition a d'4 e'4 f'4`,
/// LilyPond 2.26 joue 60, 59, 61, 62.
#[test]
fn transposition_a_partir_de_sa_declaration() {
    let score = read_lilypond("\\score { \\new Staff { c'4 \\transposition a d'4 e'4 f'4 } }")
        .expect("lecture");
    assert_eq!(hauteurs_attaques(&score), vec![60, 59, 61, 62]);
}
/// Une portée transpositrice dans un StaffGroup devient son propre Part,
/// avec `<transpose>` : le modèle garde `pitch` concert et son tpc cohérent,
/// et l'export écrit la hauteur notée. LilyPond 2.26 joue
/// 66/69/67/71/64/67/62/66.
#[test]
fn export_concert_sans_transpose_de_partie() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score {
 \new StaffGroup <<
  \new Staff { \transposition a \clef treble \key f \major \time 4/4
    R1 <a' c''>2 <bes' d''> <g' bes'>2. <f' a'>4 }
  \new PianoStaff << \new Staff { s1 } >>
 >>
 \midi {}
}
"#,
    )
    .expect("lecture");
    assert_eq!(
        hauteurs_attaques(&score),
        vec![66, 69, 67, 71, 64, 67, 62, 66]
    );
    assert!(score
        .parts
        .iter()
        .any(|part| part.instrument.transpose.is_some()));
    // pitch concert et tpc cohérent : l'alter du tpc redonne la hauteur.
    for note in score.notes.iter() {
        let spiral = (i32::from(note.tpc) + 8).rem_euclid(7) as usize;
        let base = [5, 0, 7, 2, 9, 4, 11][spiral];
        let alter = (i32::from(note.tpc) + 8).div_euclid(7) - 3;
        assert_eq!(
            (base + alter).rem_euclid(12),
            i32::from(note.pitch % 12),
            "pitch et tpc incohérents"
        );
    }
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    let pitches: Vec<String> = xml
        .split("<pitch>")
        .skip(1)
        .map(|fragment| {
            let step = fragment
                .split("<step>")
                .nth(1)
                .unwrap()
                .chars()
                .next()
                .unwrap();
            let octave = fragment
                .split("<octave>")
                .nth(1)
                .unwrap()
                .chars()
                .next()
                .unwrap();
            format!("{step}{octave}")
        })
        .collect();
    assert_eq!(
        pitches,
        vec!["A4", "C5", "B4", "D5", "G4", "B4", "F4", "A4"]
    );
}

/// Un `\new PianoStaff << >>` vide n'engrave aucune portée : le modèle ne
/// doit en créer qu'une pour l'unique `\new Staff` portant la musique.
#[test]
fn groupe_de_portees_vide_ne_cree_pas_de_portee() {
    let score = read_lilypond(
        "\\score { \\new StaffGroup << \\new Staff { c'1 d'1 } \\new PianoStaff << >> >> }",
    )
    .expect("lecture");
    assert_eq!(score.staves.len(), 1);
}

/// La transposition doit conserver l'enharmonie du compositeur : un sol bémol
/// écrit pour une clarinette en si bémol devient un fa bémol concert (tpc 6),
/// pas un mi. `Transpose::transposeTpc` (`engraving/dom/transpose.cpp`).
#[test]
fn transposition_conserve_lenharmonie() {
    let score = read_lilypond(
        r#"\version "2.24.0"
\score { \new StaffGroup <<
  \new Staff { \transposition bes \key g \major \time 4/4 ges'1 g'1 }
  \new Staff { \time 4/4 c''1 d''1 }
>> }
"#,
    )
    .expect("lecture");
    let concert: Vec<_> = score
        .notes
        .iter()
        .filter(|note| note.pitch == 64)
        .map(|note| (note.pitch, note.tpc))
        .collect();
    assert_eq!(concert, vec![(64, 6)], "fa bémol concert, pas mi");
}

/// Reduction reelle de Beatus_vir_primo.ly (trois portees, sept mesures).
#[test]
fn silences_multi_mesures_ne_fusionnent_pas_les_portees_jouees() {
    let source = r#"\version "2.12.2"
\language "deutsch"
staffSopranoI = \new Staff  {
	\time 4/4
	\override Score.MetronomeMark #'stencil = ##f
	\tempo 4 = 100 
	\set Staff.instrumentName="Soprano I"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef treble
	\relative c'' { 	
		\context Voice = "melodySopI" {
			\dynamicUp
			\autoBeamOff
			r8 e e4 d8 d d8. d16 |
			c8 c c4 c8 c c h |
			c4 r8 e, f4 g |
			a r8 g a c c8. h16 |
			c4 r r2 |
			R1 |
			r8 e e4 d8 d d d |
}}}

staffSopranoII = \new Staff  {
	\set Staff.instrumentName="Soprano II"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef treble
	\relative c'' { 	
		\context Voice = "melodySopII" {
			\dynamicUp
			\autoBeamOff
			R1 |
			r8 e e4 d8 d d d |
			c4 r8 g a4 e |
			f r8 g f e d8. d16 |
			c4 r r2 |
			R1*2 |
			r8 e' e4 d8 d d d |
}}}

staffAlto = \new Staff  {
	\set Staff.instrumentName="Alto"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef treble
	\relative c'' { 	
		\context Voice = "melodyAlt" {
			\dynamicUp
			\autoBeamOff
			R1 |
			r8 g g4 f8 a g g |
			g4 r r2 |
			R1*4 |
			r8 g g4 f8 a g g |
			g4 r r2 |
			R1*5 |
}}}

staffTenoreI = \new Staff  {
	\set Staff.instrumentName="Tenore I"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef "G_8"
	\relative c' { 	
		\context Voice = "melodyTenI" {
			\dynamicUp
			\autoBeamOff
			R1 |
			r8 e e4 a,8 a d g, |
			g4 r r2 |
			R1 |
			r2 f8 e f g |
			a h c c c4\melisma h\melismaEnd |
			c r r2 |
}}}

staffTenoreII = \new Staff  {
	\set Staff.instrumentName="Tenore II"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef "G_8"
	\relative c' { 	
		\context Voice = "melodyTenII" {
			\dynamicUp
			\autoBeamOff
			R1 |
			r8 c c4 d8 d d d |
			e4 r r2 |
			R1 |
			r2 a,8 g a h |
			c d e f d2 |
			c4 r r2 |
}}}

staffBasso = \new Staff  {
	\set Staff.instrumentName="Basso"
	\set Staff.midiInstrument="oboe"
	\key c \major
	\clef bass
	\relative c { 	
		\context Voice = "melodyBas" {
			\dynamicUp
			\autoBeamOff
			R1 |
			r8 c e4 f8 f g g |
			c,4 r r2 |
			R1*4 |
			r8 c e4 f8 f g g |
			c,4 r r2 |
			R1*5 |
}}}

staffViolinI = \new Staff  {
	\set Staff.instrumentName="Violino I"
	\set Staff.midiInstrument="violin"
	\key c \major
	\clef treble
	\relative c'' {
		R1 |
		r8 c c4 c8 c c8. h16 |
		c4 r r2 |
		R1*4 |
		r8 c c4 c8 c c8. h16 |
		c4 r r2 |
		r8 g' g4 a16 g f e d e f d |
}}

staffViolinII = \new Staff  {
	\set Staff.instrumentName="Violino II"
	\set Staff.midiInstrument="violin"
	\key c \major
	\clef treble
	\relative c'' {
		R1 |
		r8 e e4 d8 d d d |
		c4 r r2 |
		R1*4 |
		r8 e e4 d8 d d d |
		c g' g4 a16 g f e d e f d |
		e f g f e d c h a8 c c8. h16 |
}}

staffBassoContinuo = \new Staff  {
	\set Staff.instrumentName="Basso"
	\set Staff.midiInstrument="harpsichord"
	\key c \major
	\clef bass
	\relative c {
		c4 e f g |
		c, e f g |
		c, h a g |
		f e f g |
		c h a g |
		f e g2 |
		c4 e f g |
}}

staffBC = \new FiguredBass {
	\figuremode {
		s1*61 |
		\time 3/2
		\tempo 2=320
		\set Score.measureLength = #(ly:make-moment 3 1)
		s\breve. s\breve <_+>1 |
		s\breve. s\breve. |
		s\breve. s\breve. |
		s\breve. s\breve <_+>1 |
		s\breve. s\breve. |
		s\breve. s\breve <_+>1 |
}}


\score {
	<<
		\staffSopranoI
		
		\staffSopranoII
		
		\staffAlto
		
		
		
		
		
	>>
	
	\midi {
	}

	\layout  {
	}
}

\paper {
}

"#;
    let score = read_lilypond(source).expect("lecture");
    assert!(
        score.measures[3].multi_measure_rest.is_none(),
        "la mesure 4 porte des notes de la seconde portee : {:?}",
        score
            .measures
            .iter()
            .map(|measure| measure.multi_measure_rest)
            .collect::<Vec<_>>()
    );
}

/// Réduction réelle de `djinns.ly` : un `~` d'accord ne lie que les notes
/// dont la hauteur est reprise par l'accord suivant. Les liaisons sans
/// correspondance sont abandonnées au pas de temps suivant
/// (`lily/tie-performer.cc`, `stop_translation_timestep`), pas propagées
/// jusqu'à une note de même hauteur plusieurs mesures plus tard.
#[test]
fn liaison_accord_sans_correspondance_ne_se_propage_pas() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \new Staff { \time 4/4 \clef bass \relative c {
  <g bf df f>2~ <af c f> |
  s4 c8. af16 g4 af8. bf16 |
} } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let ties: Vec<_> = score
        .notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| note.tie_back.map(|_| index))
        .collect();
    assert_eq!(
        ties.len(),
        1,
        "seul le fa est lié au fa de l'accord suivant"
    );
}

/// Réduction réelle de `contrapunctusXIV.ly` : un `*n/d` explicite devient la
/// durée par défaut des notes qui suivent (`lily/parser.yy`, `DURATION`).
#[test]
fn multiplicateur_de_duree_propage_aux_notes_nues() {
    let score = lire("c4*3/2 d e");
    let touches: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, _, _)| *tick)
        .collect();
    assert!(
        touches.len() >= 3,
        "les trois notes sont là : {:?}",
        touches
    );
    assert_eq!(
        (touches[1] - touches[0], touches[2] - touches[1]),
        (Fraction::new(3, 8), Fraction::new(3, 8)),
        "le multiplicateur 3/2 vaut pour les deux notes nues : {:?}",
        touches
    );
}

/// `\scaleDurations` ne modifie que sa musique : la note nue qui suit le bloc
/// reprend la durée par défaut d'avant, sans le facteur d'échelle.
#[test]
fn echelle_de_durees_ne_change_pas_la_duree_par_defaut() {
    let score = lire("\\scaleDurations #'(2 . 3) { c16 d16 e16 } f g");
    let touches: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, _, _)| *tick)
        .collect();
    assert!(touches.len() >= 5, "les cinq notes sont là : {:?}", touches);
    assert_eq!(
        touches[4] - touches[3],
        Fraction::new(1, 16),
        "la dernière note vaut une noire, pas 2/3 : {:?}",
        touches
    );
}

/// Réduction réelle de `16title-hub.ly` : une note nue qui hérite d'un
/// multiplicateur `*n/d` doit écrire sa durée réelle, sinon l'export décale
/// toutes les attaques suivantes d'une double croche.
#[test]
fn duree_heritee_du_multiplicateur_ecrite_dans_le_modele() {
    let source = r#"\version "2.24.0"
\language "english"
\score { \new Staff { \time 4/4 \relative c' {
  c8*2/3 \rest d bf' c, \rest d bf' c, \rest d bf' c, \rest d bf'
} } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let xml = scoretk_musicxml_export::export_musicxml(&score).expect("export");
    let durees: Vec<i64> = xml
        .split("<duration>")
        .skip(1)
        .map(|fragment| {
            fragment
                .split("</duration>")
                .next()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    assert!(
        durees.iter().all(|d| *d == durees[0]),
        "toutes les notes ont la même durée réelle : {:?}",
        durees
    );
}

/// Réduction réelle de `Faure-Cantique_De_Jean_Racine-basse.ly` : un texte
/// `^\markup "x"` ne doit pas avaler les silences `r2 r4` qui suivent.
#[test]
fn markup_sans_accolades_ne_mange_pas_les_silences() {
    let source = r#"\version "2.24.0"
\score { \new Staff { \relative c' {
  R1*14^\markup "x"
  r2 r4 aes des c
} } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let touches: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, _, _)| *tick)
        .collect();
    assert!(
        touches.iter().any(|tick| *tick == Fraction::new(59, 4)),
        "la première note suit les deux silences (tick 59/4) : {:?}",
        touches
    );
}

/// Réduction réelle de `divisi-staves.ly` : un chemin de propriété s'arrête
/// après son dernier mot ; `\unset …` ne doit pas avaler le nom de variable
/// qui définit la musique suivante.
#[test]
fn chemin_de_propriete_ne_mange_pas_la_variable_suivante() {
    let source = r#"\version "2.26.0"
tricky = \unset Staff.keepAliveInterfaces
musA = { \tricky c'1 d'1 }
\score { \new Staff \musA }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        accords_de_voix(&score, 0).len(),
        2,
        "les deux notes de musA sont lues"
    );
}

/// Réduction réelle de `accidental-ledger.ly` (lot `ra`) : les noms
/// néerlandais à quarts de ton sont des notes, pas des commandes inconnues.
/// LilyPond 2.27 arrondit leur hauteur MIDI au demi-ton pair le plus proche.
#[test]
fn noms_neerlandais_a_quarts_de_ton() {
    let source = "\\version \"2.26.0\"\n\\language \"nederlands\"\n\
        \\score { \\new Staff { \\relative c''' { gisis1 gisih gis gih g geh ges geseh geses gis? } } }";
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![81, 80, 80, 80, 79, 78, 78, 78, 77, 80],
        "hauteurs MIDI des quarts de ton néerlandais"
    );
}

/// Réduction réelle de `chord-names-languages.ly` (lot `ra`) : la basse d'un
/// accord peut être écrite après la durée (`e1/d`) ; elle appartient à
/// l'accord courant, elle n'ouvre pas un nouvel accord.
#[test]
fn basse_daccord_apres_la_duree() {
    let source = r#"\version "2.26.0"
\score { \new Staff { \chordmode { e1/d c:m } } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let debut: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .take(1)
        .flat_map(|(_, notes, _)| notes.iter().copied())
        .collect();
    let mut debut = debut;
    debut.sort_unstable();
    assert_eq!(
        debut,
        vec![62, 64, 68, 71],
        "l'accord E/D contient le do grave : {:?}",
        debut
    );
}

/// La basse d'un accord est toujours strictement sous la fondamentale :
/// si le degré est égal ou supérieur, elle descend d'une octave.
#[test]
fn basse_daccord_strictement_sous_la_fondamentale() {
    let source = r#"\version "2.26.0"
\score { \new Staff { \chordmode { c1/g b1/b } } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let notes: Vec<u8> = accords_de_voix(&score, 0)
        .iter()
        .take(1)
        .flat_map(|(_, notes, _)| notes.iter().copied())
        .collect();
    assert!(
        notes.contains(&55),
        "la basse de C/G est sol3 (55) : {:?}",
        notes
    );
    let notes2: Vec<u8> = accords_de_voix(&score, 0)
        .iter()
        .skip(1)
        .take(1)
        .flat_map(|(_, notes, _)| notes.iter().copied())
        .collect();
    assert!(
        notes2.contains(&59),
        "la basse de B/B est si3 (59) : {:?}",
        notes2
    );
}
/// Réduction réelle de `carcassi-op60-15.ly` : un `\context Staff` qui ne
/// contient que des propriétés (`\transposition c`) ne crée pas une portée
/// séparée ; sa transposition vaut pour la musique qui suit.
#[test]
fn transposition_dun_contexte_sans_musique() {
    let source = r#"\version "2.26.0"
midiStuff = \context Staff = "guitar" { \transposition c }
mus = { c'4 d' e' f' }
\score { { \midiStuff \mus } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    assert_eq!(
        hauteurs(&score),
        vec![48, 50, 52, 53],
        "la transposition c descend d'une octave"
    );
}

/// Commandes musicales supprimées, réécrites par `python/convertrules.py`
/// règle (2,11,51) et (2,11,52) : le lecteur les accepte telles quelles.
#[test]
fn commandes_musicales_obsoletes_acceptées() {
    let source = r#"\version "2.26.0"
\score { \new Staff {
  c'1 \setTextCresc d'1 \setHairpinDim e'1 \sustainDown f'1
  \sustainUp g'1 \arpeggioUp a'1 \setTextDecresc b'1
} }
"#;
    let (_, warnings) = read_lilypond_reporting(source).expect("lecture");
    let ignorees: Vec<_> = warnings
        .iter()
        .filter(|w| w.message.contains("commande ignorée"))
        .collect();
    assert!(
        ignorees.is_empty(),
        "aucune commande ne doit être ignorée : {:?}",
        ignorees.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// `\transposition` peut changer en cours de partitions (clarinette en la
/// puis en si bémout) : chaque accord prend l'intervalle en vigueur à son tick.
#[test]
fn changement_de_transposition_en_cours_de_piece() {
    let source = r#"\version "2.26.0"
\language "english"
\score { \new Staff { \transposition a c'1 \transposition bf d'1 } \midi {} }
"#;
    let score = read_lilypond(source).expect("lecture");
    let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    assert_eq!(
        pitches,
        vec![57, 60],
        "la seconde note suit le nouvel intervalle : {:?}",
        pitches
    );
}

#[test]
fn acciaccature_apres_arret_de_soufflet() {
    // `\!` est un post-événement de la note précédente : il ne doit pas avaler
    // le `\acciaccatura` suivant comme argument de direction, ni transformer
    // son ornement en note ordinaire (ce qui décalerait toutes les attaques
    // suivantes d'une croche).
    let score = lire("c'8 \\! \\acciaccatura { e'8 } d'8 c'8");
    let mut graces = Vec::new();
    for measure in &score.measures {
        for segment in &measure.segments {
            if let Some(SegmentItem::ChordRest(chord_rest)) = segment.element(Track::from_raw(0)) {
                if let ChordRestKind::Chord(chord) = &chord_rest.kind {
                    for grace in &chord.grace_chords {
                        graces.push((
                            grace.chord.grace,
                            grace
                                .chord
                                .notes
                                .iter()
                                .map(|&id| score.note(id).pitch)
                                .collect::<Vec<_>>(),
                        ));
                    }
                }
            }
        }
    }
    assert_eq!(
        graces,
        vec![(Some(GraceType::Acciaccatura), vec![76])],
        "l'ornement après \\! doit rester une acciaccature"
    );
    assert_eq!(
        accords_de_voix(&score, 0)
            .iter()
            .map(|(tick, pitches, _)| (*tick, pitches.clone()))
            .collect::<Vec<_>>(),
        vec![
            (Fraction::ZERO, vec![72]),
            (Fraction::new(1, 8), vec![74]),
            (Fraction::new(1, 4), vec![72]),
        ]
    );
}

#[test]
fn portee_sans_staff_performer_est_muette_et_le_midi_ne_la_joue_pas() {
    // `ly/performer-init.ly:32` : `Staff_performer` rend une portée audible
    // dans le MIDI ; `\remove "Staff_performer"` (pianoReduction) la rend
    // silencieuse en lecture. Le modèle est le pivot de TOUS les formats :
    // la portée et ses notes restent présentes, mais elle est marquée muette
    // et l'écrivain MIDI ne joue pas ses notes (MuseScore
    // `compatmidirenderinternal.cpp`, `NPlayEvent::isMuted`).
    let src = "\\version \"2.24.0\"\n\
         \\score { << \
           \\new Staff { c'4 d'4 e'4 f'4 } \
           \\new Staff \\with { \\remove \"Staff_performer\" } { c'4 d'4 e'4 f'4 } \
         >> }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(score.staves.len(), 2, "les deux portées sont gravées");
    assert_eq!(
        score.notes.len(),
        8,
        "aucune note du modèle n'est supprimée"
    );
    assert_eq!(
        score
            .staves
            .iter()
            .map(|staff| staff.muted)
            .collect::<Vec<_>>(),
        vec![false, true],
        "seule la seconde portée est muette"
    );
    let octets = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = midly::Smf::parse(&octets).expect("SMF valide");
    let attaques: u32 = smf
        .tracks
        .iter()
        .flat_map(|piste| piste.iter())
        .filter(|evenement| {
            matches!(
                evenement.kind,
                midly::TrackEventKind::Midi {
                    message: midly::MidiMessage::NoteOn { vel, .. },
                    ..
                } if vel > 0
            )
        })
        .count() as u32;
    assert_eq!(attaques, 4, "le MIDI ne joue que la première portée");
}

#[test]
fn unfold_repeats_dun_score_ne_sapplique_pas_a_un_autre() {
    // `scm/lily-library.scm` (`get-outfile-name`) et `scm/midi.scm`
    // (`write-performances-midis`) écrivent un MIDI par score : `base.midi`
    // pour le premier, `base-1.midi` pour le suivant. L'oracle retient
    // `base.midi`. Un `\unfoldRepeats` écrit dans le second score ne doit donc
    // pas déplier les reprises du premier, qui fait référence.
    let src = "\\version \"2.24.0\"\n\
         \\score { \\new Staff { \\repeat volta 2 { c'4 d'4 } \\midi {} \\layout {} } }\n\
         \\score { \\new Staff { \\unfoldRepeats { \\repeat volta 2 { e'4 f'4 } } \\midi {} } }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![60, 62],
        "le premier score garde sa reprise non dépliée"
    );
}

#[test]
fn acciaccatura_avec_accord_simultane_garde_toutes_les_notes() {
    // `scm/music-functions.scm::def-grace-function` enveloppe la musique
    // entière dans `GraceMusic`; `lily/parser.yy::simultaneous_music` fait de
    // `<< a \\ b >>` une musique simultanée dont les deux branches doivent
    // rester des accords d'agrément.
    let src = "\\version \"2.24.0\"\n{ e' \\acciaccatura << e'8 \\ cis' >> }\n";
    let score = read_lilypond(src).expect("lecture");
    let pitches: Vec<u8> = score.notes.iter().map(|note| note.pitch).collect();
    assert_eq!(pitches, vec![64, 64, 61], "hauteurs: {:?}", pitches);
}

#[test]
fn affectation_identifiant_ponctuation_barre() {
    // `lily/parser.yy` accepte une chaîne comme identifiant : `"|" = \bar "||"`
    // redéfinit le contrôle de mesure. Le lecteur sautait la chaîne, laissant
    // `=` et `\bar` avaler la musique qui suit.
    let src = "\\version \"2.24.0\"\n\"|\" = \\bar \"||\"\n{ c'2 | c'2 | }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![60, 60],
        "la musique suit l'affectation"
    );
}

#[test]
fn identifiant_entre_guillemets_echappes() {
    // `lily/parser.yy` accepte `\"nom"` comme identifiant ; les variables
    // assignées avec une chaîne (`"nom" = { … }`) doivent être retrouvées.
    let src = "\\version \"2.24.0\"\n\"violin1\" = { c''4 c'' }\n{ \\\"violin1\" }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![72, 72]
    );
}

#[test]
fn repeat_segno_ne_decale_pas_la_suite() {
    // `scm/music-functions.scm::make-repeat` construit un `SegnoRepeatedMusic`
    // avec le même itérateur qu'un volta (`scm/define-music-types.scm`) : le
    // MIDI d'un score non déplié joue le corps une seule fois, sans avancer
    // la mesure suivante du nombre de reprises.
    let src = "\\version \"2.24.0\"\n\\
         \\new Voice \\fixed c' { \\repeat segno 2 { a1 } b1 }\n";
    let score = read_lilypond(src).expect("lecture");
    let attaques: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, pitches, _)| (*tick, pitches.clone()))
        .collect();
    assert_eq!(
        attaques,
        vec![
            (scoretk_core::model::Fraction::ZERO, vec![69]),
            (scoretk_core::model::Fraction::from_integer(1), vec![71])
        ],
        "attaques: {:?}",
        attaques
    );
}

#[test]
fn make_clusters_ne_joue_pas_de_notes() {
    // `scm/music-functions.scm::note-to-cluster` remplace les `NoteEvent` par
    // des `ClusterNoteEvent`, que le performer MIDI ne joue pas. La portée est
    // gravée mais muette, comme un `Staff_performer` retiré.
    let src = "\\version \"2.24.0\"\n\\
         \\new Staff \\makeClusters { c'4 d'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(score.staves.len(), 1);
    assert!(score.staves[0].muted, "la portée de clusters est muette");
    let octets = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    let smf = midly::Smf::parse(&octets).expect("SMF valide");
    let attaques: u32 = smf
        .tracks
        .iter()
        .flat_map(|piste| piste.iter())
        .filter(|evenement| {
            matches!(
                evenement.kind,
                midly::TrackEventKind::Midi {
                    message: midly::MidiMessage::NoteOn { vel, .. },
                    ..
                } if vel > 0
            )
        })
        .count() as u32;
    assert_eq!(attaques, 0, "le MIDI ne joue pas les clusters");
}

#[test]
fn accord_chordmode_min7() {
    // `scm/chord-entry.scm:235,252` : `min` est un modificateur appliqué à
    // l'accord de base `7`, donc `c:min7` = do, mi♭, sol, si♭.
    let src = "\\version \"2.24.0\"\n\\chords { c:min7 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 63, 67, 70]
    );
}

#[test]
fn grace_dans_une_voix_simultanee_est_jouee() {
    // `scm/music-functions.scm::volta-spec-music` enveloppe la musique
    // d'un `\volta` dans une voix de la `<< … >>`; un ornement en fin de
    // cette voix doit être joué comme dans la voix principale.
    let src = "\\version \"2.24.0\"\n\\
         \\new Voice \\fixed c' { << e2 \\volta 1 { s4... \\grace { g8 f } } >> b1 }\n";
    let score = read_lilypond(src).expect("lecture");
    let pitches: Vec<u8> = score.notes.iter().map(|n| n.pitch).collect();
    assert!(
        pitches.contains(&67) && pitches.contains(&65),
        "les agréments g et f doivent être présents : {:?}",
        pitches
    );
}

#[test]
fn make_clusters_nimporte_où_dans_la_musique_de_portee() {
    // `ly/music-functions-init.ly:1154-1157` : `\makeClusters` est une
    // fonction musicale appliquée à l'expression qui suit, qui peut être une
    // référence de variable. Le mute est relevé où que le commande apparaisse
    // dans la musique de la portée (le modèle n'a pas de mute par voix).
    let src = "\\version \"2.24.0\"\n\
         fragment = { e'4 f'4 }\n\
         \\new Staff { c'4 \\makeClusters \\fragment d'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(score.staves.len(), 1);
    assert!(
        score.staves[0].muted,
        "\\makeClusters au milieu de la musique doit rendre la portée muette"
    );
    assert_eq!(
        score.notes.len(),
        4,
        "les notes restent dans le modèle (pivot de tous les formats)"
    );
}

#[test]
fn accord_chordmode_min11() {
    // `scm/chord-entry.scm:125-145` : `min` est un modificateur leading
    // appliqué à l'accord de base construit depuis le degré qui suit, donc
    // `c:min11` = do, mi♭, sol, si♭, ré, fa (vérifié dans le MIDI de
    // référence produit par LilyPond 2.24).
    let src = "\\version \"2.24.0\"\n\\chords { c:min11 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 63, 67, 70, 74, 77]
    );
}

#[test]
fn liste_markup_entre_accolades_ne_cree_pas_une_partition_vide() {
    // `lily/parser.yy:4160-4165` : les accolades qui suivent une commande de
    // markup sont son argument (`markup_braced_list`), pas une musique à
    // jouer. Sinon le faux `{ "0" "0" }` devenait la première partition et
    // la vraie portée était écartée comme « partition supplémentaire ».
    let src = "\\version \"2.24.0\"\n\
         \\markup \\right-column { \"X\" \"Y\" }\n\
         \\new Staff { c'1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60]
    );
}

#[test]
fn markup_dans_with_conserve_la_musique() {
    // `arpeggio-positions.ily` : `instrumentName = \markup \tiny
    // \right-column { "0" "0" }` dans un `\with`, puis la musique de portée.
    let src = "\\version \"2.24.0\"\n\
         \\new Staff \\with { instrumentName = \\markup \\tiny \\right-column { \"0\" \"0\" } } { c'1 d'1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62]
    );
}

#[test]
fn markup_commande_sans_argument_laisse_la_musique() {
    // `scm/define-markup-commands.scm` : `\foo` n'a aucun argument ; le bloc
    // qui suit est la musique, pas une liste de markup
    // (`markup-invalid-stencil.ly`, `textetc.ly` avec `\etc`).
    let src = "\\version \"2.24.0\"\n\\
         \\markup \\foo\n\\
         { c'1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60]
    );
}

#[test]
fn interpolation_scheme_dune_variable_musicale() {
    // `lily/parser.yy:3221-3225` : `embedded_scm` est un élément musical dans
    // un corps `#{ … #}`. `#musique` désigne la variable Scheme `musique`
    // assignée en syntaxe LilyPond (`bar-number-visibility.ily` : le corps de
    // `\testVisibility` insère `#music`).
    let src = "\\version \"2.24.0\"\nmusique = \\fixed c' { c'4 d' }\n\
fonction =\n#(define-music-function (visibilite) (procedure?)\n\
   #{ #musique #})\n\\fonction#quelquechose\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![72, 74]
    );
}

#[test]
fn skip_avec_musique_ne_joue_pas_cette_musique() {
    // `ly/music-functions-init.ly:2267-2274` : `\skip` accepte une durée ou
    // une musique (`duration-or-music?`). Avec une musique, les notes sont
    // traversées silencieusement ; seul le temps est avancé.
    let src = "\\version \"2.24.0\"\nmusiqueSautee = { c1 | e1 }\n\
\\fixed c' { a1 | \\skip \\musiqueSautee | a1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![69, 69],
        "seules les deux notes a doivent être jouées"
    );
}

#[test]
fn maxima_vaut_huit_rondes() {
    // `scm/define-music-display-methods.scm:114` : `\maxima` a log2 = -3,
    // donc 2^3 = 8 rondes. Le lecteur le confondait avec `\longa` (4 rondes).
    let score = read_lilypond("\\version \"2.24.0\"\n{ c'\\maxima d'1 }").expect("lecture");
    let attaques = accords_de_voix(&score, 0);
    assert_eq!(
        attaques.last().map(|(tick, _, _)| *tick),
        Some(Fraction::from_integer(8)),
        "la maxima doit durer 8 rondes, pas 4 : {:?}",
        attaques
    );
}

#[test]
fn identifiant_de_duree_compresse() {
    // `lily/duration-scheme.cc:53-99` : `ly:make-duration 0 0 1/2` porte un
    // facteur de compression 1/2 sur une ronde. L'identifiant utilisé derrière
    // une note (`c'\wholeHalved`) fixe aussi la durée par défaut suivante.
    let src =
        "\\version \"2.24.0\"\nwholeHalved = #(ly:make-duration 0 0 1/2)\n{ c'\\wholeHalved c }\n";
    let score = read_lilypond(src).expect("lecture");
    let attaques: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, _, _)| *tick)
        .collect();
    assert_eq!(
        attaques,
        vec![Fraction::ZERO, Fraction::new(1, 2)],
        "attaques: {:?}",
        attaques
    );
}

#[test]
fn augmentum_ajoute_un_point_a_la_duree() {
    // `ly/ancient-init.ly:124-130` : `augmentum` vaut
    // `shift-duration-log expr 0 1`, c'est-à-dire un point de plus sur chaque
    // durée de l'expression (les morae du chant grégorien).
    let src = "\\version \"2.24.0\"\n{ \\augmentum c'4 \\augmentum d'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    let attaques: Vec<_> = accords_de_voix(&score, 0)
        .iter()
        .map(|(tick, _, _)| *tick)
        .collect();
    assert_eq!(
        attaques,
        vec![Fraction::ZERO, Fraction::new(3, 8)],
        "attaques: {:?}",
        attaques
    );
}

#[test]
fn pitched_trill_ne_joue_pas_la_note_secondaire() {
    // `ly/music-functions-init.ly:1707-1712` : le second argument de
    // `\pitchedTrill` est une tête de note entre parenthèses, seulement
    // gravée ; le MIDI n'en tient pas compte.
    let src = "\\version \"2.24.0\"\n\\
         \\fixed c' { \\pitchedTrill c'4.\\startTrillSpan es' f' }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![72, 77],
        "la note secondaire es ne doit pas être jouée"
    );
}

#[test]
fn absolute_cache_la_musique_du_relative_entourant() {
    // `ly/music-functions-init.ly:32-36` et Notation Reference §1.1.2 :
    // `\absolute` soustrait sa musique au `\relative` environnant ; la
    // note qui suit reprend la référence relative d'avant le bloc.
    let src = "\\version \"2.24.0\"\n\
         \\relative c' { c'4 g \\absolute { c'' } e' }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![72, 67, 72, 76],
        "c'' absolu vaut do5 ; e' se règle sur sol, pas sur le bloc absolu"
    );
}

#[test]
fn include_avec_identifiant_de_chaine() {
    // `lily/lexer.ll:340-350` : apres `\\include`, un identifiant dont la
    // valeur est une chaine designe le fichier a inclure.
    let fichiers = HashMap::from([("notes.ly", "d'4")]);
    let (score, warnings) = read_lilypond_with_includes(
        "\\version \"2.24.0\"\nwhatToInclude = \"notes.ly\"\n{ c'4 \\include \\whatToInclude e'4 }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 62, 64], "{warnings:?}");
}

#[test]
fn version_dans_un_include_musical_garde_la_duree_nue() {
    // Un fichier inclus peut commencer par `\version "…"` puis une durée nue
    // (`include-identifier.ily`) : la commande ne doit pas avaler cette durée.
    let fichiers = HashMap::from([("avecversion.ly", "\\version \"2.24\" 4")]);
    let (score, warnings) = read_lilypond_with_includes(
        "{ c'4 \\include \"avecversion.ly\" e'4 }\n",
        &mut resolveur(&fichiers),
    )
    .expect("lecture");
    assert_eq!(hauteurs(&score), vec![60, 60, 64], "{warnings:?}");
}
/// Une liaison portée par la dernière note d'un bloc d'agrément
/// (`def-grace-function`, `scm/music-functions.scm:1237-1248`) doit rejoindre
/// la note principale : la note d'agrément ne consomme pas de temps propre,
/// donc `link_ties` (qui apparie par tick) ne peut pas la voir.
#[test]
fn liaison_depuis_agrément_vers_note_principale() {
    let score = lire("\\acciaccatura { d8~ } d4");
    assert_eq!(
        liaisons(&score),
        vec![(62, 62)],
        "la note d'agrément doit être liée à la note principale"
    );
}

/// `\once \override OttavaBracket.transparent = ##t` masque le crochet sans
/// changer la hauteur écrite (`scm/define-grobs.scm:2708`). La relecture doit
/// conserver `visible = faux` sur le spanner, sinon l'export MusicXML ajoute
/// un `<octave-shift>` qui n'existe pas dans la référence.
#[test]
fn ottava_invisible_depuis_override() {
    use scoretk_core::model::SpannerType;
    let score = read_lilypond(
        "\\version \"2.24.0\"\n\\language \"english\"\n\
         \\score { \\new Staff { \\fixed c' { c4 \\once \\override OttavaBracket.transparent = ##t \\ottava 1 d4 \\ottava 0 e4 } } }",
    )
    .expect("lecture \\override OttavaBracket");
    let ottavas: Vec<_> = score
        .spanners
        .iter()
        .filter(|spanner| matches!(spanner.kind, SpannerType::Ottava(_)))
        .collect();
    assert_eq!(ottavas.len(), 1, "une seule ottava");
    assert!(
        !ottavas[0].visible,
        "le crochet d'ottava doit rester masqué"
    );
}

/// En `chordmode`, `\fixed c,` décale d'une octave les notes de la table
/// d'accords (`ly/parser.yy`, `chord_body` ; `scm/chord-entry.scm` applique
/// l'octave de référence à la fondamentale). Sans ce décalage, un
/// `ChordNames` sous `\fixed c,` sonne une octave trop haut (vérifié sur le
/// MIDI de LilyPond 2.24).
#[test]
fn accordmode_sous_fixed_decale_les_hauteurs() {
    let score = read_lilypond(
        "\\version \"2.24.0\"\n\\score { \\new Staff { \\chordmode { \\fixed c, { f2 } } } }",
    )
    .expect("lecture \\chordmode");
    assert_eq!(
        score
            .notes
            .iter()
            .map(|note| note.pitch)
            .collect::<Vec<_>>(),
        vec![53, 57, 60],
        "f sous \\fixed c, doit jouer F3/A3/C4"
    );
}

/// `\articulate` est propre au `\score` où il apparaît, comme
/// `\unfoldRepeats` (`scm/music-functions.scm`). Un second score qui
/// articule sa musique ne doit pas faire croire que la partition retenue
/// (la première avec `\midi`) est articulée : l'écrivain LilyPond
/// envelopperait alors toutes ses notes dans `\articulate`.
#[test]
fn articulate_du_second_score_ne_contamine_pas_le_premier() {
    let source = r#"\version "2.24.0"
\language "english"
musique = { \fixed c' { c4 d e f } }
\score {
  << \new Staff { \musique } >>
  \layout { }
  \midi { }
}
\score {
  << \new Staff { \articulate { \musique } } >>
  \midi { }
}
"#;
    let score = read_lilypond(source).expect("lecture");
    assert!(
        !score.playback_articulate,
        "la partition retenue n'est pas articulée"
    );
}

#[test]
fn seconde_voix_relative_heritant_de_la_premiere() {
    // `lily/music-sequence.cc:143-158` : `music_list_to_relative` enchaîne
    // les branches de `<< … >>`, la seconde part de la dernière hauteur de la
    // première (`lily/relative-octave-check.cc` pour les marques d'octave).
    let src = "\\version \"2.24.0\"\n\\relative << { c''8 r } \\\\ { c,8.[ c'16] } >>\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![72, 60, 72]
    );
}

#[test]
fn cue_during_joue_la_citation() {
    // `ly/music-functions-init.ly:545-557,560-574` : `\cueDuring` insère la
    // musique citée par `\addQuote` dans une voix de cue ; le MIDI joue ces
    // notes à la place des silences de l'argument principal.
    let src = "\\version \"2.24.0\"\n\
         vI = \\fixed c' { g4 g4 g4 g4 }\n\
         \\addQuote vIQuote { \\vI }\n\
         \\new Staff \\fixed c' { c4 \\cueDuring \"vIQuote\" #DOWN { r4 r4 r4 r4 } c4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 67, 67, 67, 67, 60]
    );
    assert!(
        score
            .notes
            .iter()
            .filter(|n| n.pitch == 67)
            .all(|n| n.small),
        "les notes de cue sont en petit format"
    );
    assert!(
        score
            .notes
            .iter()
            .filter(|n| n.pitch == 60)
            .all(|n| !n.small),
        "les notes de la voix principale restent normales"
    );
}

#[test]
fn abbreviations_repeat_unfold_et_percent() {
    // `ly/music-functions-init.ly:742-751` : `\*n` = `\repeat unfold n`,
    // `\%n` = `\repeat percent n`.
    let src = "\\version \"2.24.0\"\n{ \\*4 c'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 60, 60, 60]
    );
}

#[test]
fn text_mark_ne_mange_pas_la_duree_nue_suivante() {
    // `ly/music-functions-init.ly:2365-2368` : `\textMark` prend un seul
    // argument de markup ; la durée nue qui suit est une musique à jouer.
    let src = "\\version \"2.24.0\"\n{ c'4 \\textMark \"marque\" 4 4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 60, 60]
    );
}

#[test]
fn identifiant_numerique_scheme_dans_repeat_unfold() {
    // `lily/parser.yy` : `num = #3` lie un identifiant Scheme au nombre 3,
    // utilisable comme argument de `\repeat unfold \num`.
    let src = "\\version \"2.24.0\"\nnum = #3\nmus = { c'4 }\n{ \\repeat unfold \\num \\mus }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 60, 60]
    );
}

#[test]
fn identifiant_pointedans_un_nom_de_variable() {
    // `identifier-alists.ly` : les noms de variables peuvent contenir un
    // point (`violin.1 = { … }` puis `\violin.1`).
    let src = "\\version \"2.24.0\"\nviolin.1 = { c'4 }\n{ \\violin.1 d'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62]
    );
}

#[test]
fn alternative_a_l_interieur_du_corps_de_reprise() {
    // `\repeat volta 1 { a1 \alternative { b1 } }` : LilyPond accepte
    // l'`\alternative` à l'intérieur des accolades du corps.
    let src = "\\version \"2.24.0\"\n{ \\repeat volta 1 { c'4 \\alternative { d'4 } } }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62]
    );
}

#[test]
fn quote_during_joue_la_citation() {
    // `ly/music-functions-init.ly:2023-2032` : `\quoteDuring` a la même
    // sémantique MIDI que `\cueDuring`, sans direction ni armure.
    let src = "\\version \"2.24.0\"\n\
         src = \\fixed c' { g4 g4 g4 g4 }\n\
         \\addQuote srcId { \\src }\n\
         \\new Staff \\fixed c' { c4 \\quoteDuring \"srcId\" { r4 r4 r4 r4 } }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 67, 67, 67, 67]
    );
    assert!(score
        .notes
        .iter()
        .filter(|n| n.pitch == 67)
        .all(|n| n.small));
}

#[test]
fn with_suivi_d_un_identifiant_de_contexte() {
    // `ly/context-mods-init.ly:52` et `lily/parser.yy:1625` : `\with` accepte
    // un `MUSIC_IDENTIFIER` (`\RemoveEmptyStaves`) comme modification de
    // contexte, sans bloc entre accolades.
    let src = "\\version \"2.24.0\"\n\
         \\new Staff \\with \\RemoveEmptyStaves { c'1 r1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60]
    );
}

#[test]
fn time_numerateur_nul_ignore() {
    // LilyPond avertit « unsupported time signature » et garde la mesure
    // par défaut quand le numérateur est nul (`time-numerator-zero.ly`).
    let src = "\\version \"2.24.0\"\n{ \\time 0/1 c'1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60]
    );
}

#[test]
fn variable_scheme_appellee_par_backslash() {
    // `#(define nom #{ … #})` définit un identifiant LilyPond, utilisable
    // aussi bien par `$nom`/`#nom` que par `\nom` (`utf-8-scheme.ly`).
    let src = "\\version \"2.24.0\"\n#(define musique #{ { c'4 } #})\n\\new Staff \\musique\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60]
    );
}

#[test]
fn parallel_music_avec_enveloppe_fixe() {
    // `parallelMusic-partial.ly` : une enveloppe `\fixed` peut précéder le
    // bloc de `\parallelMusic`; elle s'applique à chaque voix produite.
    let src = "\\version \"2.24.0\"\n\
         \\parallelMusic vI,vII \\fixed c'\n\
         { c'2 | d'2 | e'2 | f'2 | }\n\
         { \\vI \\vII }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![72, 76, 74, 77]
    );
}

#[test]
fn repeat_unfold_avec_alternatives() {
    // `repeat-unfold.ly` : sous `\unfoldRepeats`, le corps est rejoué `n`
    // fois et les alternatives se répartissent sur les passages, la première
    // étant multipliée s'il y en a moins que de reprises.
    let src = "\\version \"2.24.0\"\n\
         \\unfoldRepeats { \\repeat unfold 4 { c'4 d'4 } \\alternative { e'4 f'4 } }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62, 64, 60, 62, 64, 60, 62, 64, 60, 62, 65]
    );
}

#[test]
fn repeat_avec_etc_applique_le_corps_a_l_appel() {
    // `repeat-etc.ly` : `\repeat unfold 4 \etc` définit une fonction
    // partielle dont le corps est fourni à l'appel de la variable.
    let src = "\\version \"2.24.0\"\n\
         testQuad = \\repeat unfold 4 \\etc\n\
         { \\testQuad c'4 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 60, 60, 60]
    );
}

#[test]
fn auto_change_joue_toutes_les_notes() {
    // `ly/music-functions-init.ly:278-303` : `\autoChange` répartit les
    // notes entre deux portées, mais le MIDI joue toutes les notes.
    let src = "\\version \"2.24.0\"\n\
         \\autoChange c' \\with { \\clef treble } \\with { \\clef bass }\n\
         { c'4 d' e' f' }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62, 64, 65]
    );
}

#[test]
fn measure_count_sans_argument_ne_mange_pas_la_duree() {
    // `ly/spanners-init.ly:36-37` : `\startMeasureCount` et
    // `\stopMeasureCount` sont des événements sans argument.
    let src = "\\version \"2.24.0\"\n{ c'1 \\startMeasureCount 1 \\stopMeasureCount 1 }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 60, 60]
    );
}

#[test]
fn part_combine_au_niveau_superieur_joue_les_deux_voix() {
    // `\partCombine` est une fonction musicale : au niveau supérieur, les
    // deux branches forment une seule partition, pas deux partitions dont
    // une seule serait retenue.
    let src = "\\version \"2.24.0\"\n\\partCombine { c'4 d' } { e'4 f' }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62, 64, 65]
    );
}

#[test]
fn note_names_contexte_grave_mais_non_joue() {
    // `lily/note-name-engraver.cc` : NoteNames est un contexte de gravure,
    // sans performer MIDI (accidental-tie.ly, note-name-context-custom.ly).
    let src = "\\version \"2.24.0\"\n\
         << \\new NoteNames { c'4 d' } \\new Voice { e'4 f' } >>\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![64, 65]
    );
}

#[test]
fn fret_boards_contexte_grave_mais_non_joue() {
    // `FretBoards` n'a pas de performer MIDI : sa musique est gravée mais
    // n'ajoute pas de notes jouées (fret-boards.ly).
    let src = "\\version \"2.24.0\"\n\
         << \\new FretBoards { c'4 d' } \\new Voice { e'4 f' } >>\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![64, 65]
    );
}

#[test]
fn part_combine_voix_identiques_fusionnees() {
    // `part-combine-slur.ly` : deux voix strictement identiques sont jouées
    // une seule fois par le part combiner (unissons fusionnés).
    let src = "\\version \"2.24.0\"\n\
         \\partCombine { c'4( d'4~ d'4 f'4) } { c'4( d'4~ d'4 f'4) }\n";
    let score = read_lilypond(src).expect("lecture");
    assert_eq!(
        score.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        vec![60, 62, 62, 65]
    );
}
