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

//! Instantanés de l'écriture LilyPond sur tout le corpus v3/v4.
//!
//! Chaque `.mscx` du corpus est converti et comparé à la sortie versionnée
//! dans `tests/corpus/ly/`. Pour accepter un changement de forme volontaire,
//! régénérer les instantanés :
//!
//! ```sh
//! UPDATE_LILYPOND_SNAPSHOTS=1 cargo test -p scoretk-lilypond-export --test corpus_lilypond
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use scoretk_core::model::{
    tpc, ChordRest, ChordRestKind, Clef, ClefType, Duration, Fraction, Lyrics, LyricsSyllabic,
    Measure, Note, Part, Segment, SegmentItem, SegmentType, Track, Transpose,
};

/// Racine du corpus.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

/// Lit un `.mscx` et le convertit en LilyPond.
fn convertir(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    let container = scoretk_musescore_import::MsczContainer::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("conteneur de {path:?} : {error}"));
    let score = scoretk_musescore_import::read::read_score(&container)
        .unwrap_or_else(|error| panic!("lecture de {path:?} : {error}"));
    scoretk_lilypond_export::write_lilypond(&score)
        .unwrap_or_else(|error| panic!("écriture de {path:?} : {error}"))
}

/// Tous les `.mscx` du corpus, triés.
fn fichiers() -> Vec<PathBuf> {
    let mut fichiers = Vec::new();
    for version in ["v3", "v4"] {
        let dossier = corpus().join(version);
        let mut entrees: Vec<PathBuf> = fs::read_dir(&dossier)
            .unwrap_or_else(|error| panic!("lecture de {dossier:?} : {error}"))
            .filter_map(|entree| entree.ok())
            .map(|entree| entree.path())
            .filter(|chemin| {
                chemin
                    .extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .collect();
        entrees.sort();
        fichiers.extend(entrees);
    }
    assert!(!fichiers.is_empty(), "corpus vide");
    fichiers
}

/// Instantané correspondant à un fichier du corpus.
fn instantane(path: &Path) -> PathBuf {
    let version = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .expect("dossier de version");
    let tige = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("nom de fichier");
    corpus().join("ly").join(format!("{version}_{tige}.ly"))
}

#[test]
fn conversions_egales_aux_instantanes() {
    let mise_a_jour = std::env::var_os("UPDATE_LILYPOND_SNAPSHOTS").is_some();
    let mut mises_a_jour = 0usize;
    let mut compares = 0usize;

    for path in fichiers() {
        let produit = convertir(&path);
        let reference = instantane(&path);
        if mise_a_jour {
            fs::create_dir_all(reference.parent().expect("dossier"))
                .expect("création du dossier d'instantanés");
            fs::write(&reference, &produit)
                .unwrap_or_else(|error| panic!("écriture de {reference:?} : {error}"));
            mises_a_jour += 1;
            continue;
        }
        let attendu = fs::read_to_string(&reference).unwrap_or_else(|error| {
            panic!(
                "instantané {reference:?} illisible ({error}) : \
                 lancer UPDATE_LILYPOND_SNAPSHOTS=1 cargo test -p scoretk-lilypond-export --test corpus_lilypond"
            )
        });
        assert_eq!(produit, attendu, "instantané différent pour {path:?}");
        compares += 1;
    }

    if mise_a_jour {
        eprintln!("{mises_a_jour} instantanés réécrits");
    } else {
        eprintln!("{compares} conversions comparées aux instantanés");
    }
}

#[test]
fn sortie_structuree_et_en_mode_fixed() {
    for path in fichiers() {
        let produit = convertir(&path);
        assert!(
            produit.starts_with("\\version \"2.24.0\"\n"),
            "{path:?} : en-tête de version absent"
        );
        assert!(
            produit.contains("\\language \"english\""),
            "{path:?} : langue absente"
        );
        assert!(
            produit.contains("= \\fixed c' {"),
            "{path:?} : aucune voix en mode fixed"
        );
        assert!(
            !produit.contains("\\relative"),
            "{path:?} : sortie en mode relatif interdite"
        );
        assert!(
            produit.contains("\\score {"),
            "{path:?} : bloc \\score absent"
        );
        assert!(produit.ends_with("}\n"), "{path:?} : fin de document");
    }
}

#[test]
fn clef_initiale_depuis_default_clef() {
    let mut score = scoretk_core::model::Score::new();
    let part = score.add_part(Part::new("Guitare", "Gtr."));
    let staff = score.add_staff(part);
    score.staff_mut(staff).default_clef = Clef::new(ClefType::G8Vb);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    segment.set_element(
        Track::new(staff, 0),
        SegmentItem::ChordRest(ChordRest::rest(Duration::whole())),
    );
    measure.add_segment(segment);
    score.add_measure(measure);

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("\\clef \"treble_8\""),
        "clef initiale absente :\n{produit}"
    );
}

/// Un décalage d'octave pur n'est pas une transposition harmonique : on écrit
/// la hauteur concert sans `\transposition`, ce qui donne le même MIDI avec
/// moins d'octets (cas réel des `v4_hairpin_*`). Une transposition
/// chromatique, elle, doit rester écrite dans la voix, sinon LilyPond joue la
/// hauteur écrite et non la hauteur concert (cas construit `\transposition
/// bf`). Une seconde partie non transpositrice ne reçoit rien.
#[test]
fn transposition_instrument_ecrite_dans_la_voix() {
    let produit = convertir(&corpus().join("v4/hairpin_simple.mscx"));
    assert!(
        !produit.contains("\\transposition"),
        "transposition d'octave redondante :\n{produit}"
    );
    // En mode `\fixed c'`, le si 3 concert s'écrit `b,` et le do 4 `c`.
    for hauteur in ["b,4", "c4", "d4", "e4"] {
        assert!(
            produit.contains(hauteur),
            "hauteur concert {hauteur} absente :\n{produit}"
        );
    }

    // Deux parties : une clarinette en si bémol (écrit = concert + 2) et une
    // flûte non transpositrice, chacune avec accord, silence et barre finale.
    let mut score = scoretk_core::model::Score::new();
    let clarinette = score.add_part(Part::new("Clarinette", "Cl."));
    score.part_mut(clarinette).instrument.transpose = Some(Transpose::new(-2, -2));
    let flute = score.add_part(Part::new("Flûte", "Fl."));
    let staff_cl = score.add_staff(clarinette);
    let staff_fl = score.add_staff(flute);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    for (staff, midi) in [(staff_cl, 62), (staff_fl, 62)] {
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = ChordRest::chord(Duration::whole());
        let mut note = Note::new(midi, tpc::D);
        note.tpc_transposed = if staff == staff_cl { tpc::E } else { tpc::D };
        let note_id = score.add_note(note);
        let ChordRestKind::Chord(ref mut content) = chord.kind else {
            unreachable!()
        };
        content.notes.push(note_id);
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
    }
    score.add_measure(measure);

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("\\transposition bf"),
        "transposition si bémol absente :\n{produit}"
    );
    let voix_flute = produit
        .split("staffTwoVoiceOne = ")
        .nth(1)
        .expect("deuxième voix");
    assert!(
        !voix_flute.contains("\\transposition"),
        "la flûte ne doit pas porter de transposition :\n{voix_flute}"
    );
}

/// `\transposition` est une propriété de contexte qui persiste sur toute la
/// portée : Notation Reference §1.3.4 « The `\transposition` may be changed
/// during a piece » et « `\transposition c` to get correct MIDI output ».
/// Quand une clarinette en si bémol cède la place à un instrument non
/// transpositeur, il faut donc réinitialiser explicitement la propriété, sinon
/// LilyPond continue de jouer les notes de la seconde mesure un ton trop bas.
/// La seconde portée, jamais transpositrice, sert de témoin : sa sortie ne doit
/// pas changer.
#[test]
fn retour_a_une_portee_non_transpositrice_reinitialise_la_transposition() {
    let mut score = scoretk_core::model::Score::new();
    let clarinette = score.add_part(Part::new("Clarinette", "Cl."));
    score.part_mut(clarinette).instrument.transpose = Some(Transpose::new(-2, -2));
    let flute = score.add_part(Part::new("Flûte", "Fl."));
    let staff_cl = score.add_staff(clarinette);
    let staff_fl = score.add_staff(flute);

    // Deux mesures pleines de musique ordinaire : accord de trois notes en
    // ronde (mesure 1), silence puis accord lié à travers la barre (mesure 2),
    // barres de contrôle, deux portées dont une témoin.
    fn accord(
        score: &mut scoretk_core::model::Score,
        transpositeur: bool,
        duree: Duration,
    ) -> (ChordRest, scoretk_core::model::NoteId) {
        let mut chord = ChordRest::chord(duree);
        let ChordRestKind::Chord(ref mut content) = chord.kind else {
            unreachable!()
        };
        let tpcs = if transpositeur {
            [tpc::E, tpc::G_SHARP, tpc::B]
        } else {
            [tpc::D, tpc::F_SHARP, tpc::A]
        };
        let mut premier = None;
        for (midi, tpc_note) in [62, 66, 69].into_iter().zip(tpcs) {
            let mut note = Note::new(midi, tpc_note);
            if transpositeur {
                note.tpc_transposed = tpc_note;
            }
            let id = score.add_note(note);
            if premier.is_none() {
                premier = Some(id);
            }
            content.notes.push(id);
        }
        (chord, premier.unwrap())
    }

    let mut lies: Vec<scoretk_core::model::NoteId> = Vec::new();
    for measure_index in 0usize..2 {
        let start = Fraction::new(measure_index as i64, 1);
        let mut measure = Measure::new(measure_index, start, Fraction::ONE);
        if measure_index == 1 {
            let mut repos = Segment::new(SegmentType::ChordRest, start, 1);
            for staff in [staff_cl, staff_fl] {
                repos.set_element(
                    Track::new(staff, 0),
                    SegmentItem::ChordRest(ChordRest::rest(Duration::half())),
                );
            }
            measure.add_segment(repos);
        }

        if measure_index == 1 {
            let mut seconde = Segment::new(SegmentType::ChordRest, start + Fraction::new(1, 2), 1);
            for (staff, prec) in [staff_cl, staff_fl].into_iter().zip(lies.clone()) {
                let (chord, id) = accord(&mut score, staff == staff_cl, Duration::half());
                score.note_mut(prec).tie_forward = Some(id);
                score.note_mut(id).tie_back = Some(prec);
                seconde.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
            }
            measure.add_segment(seconde);
        } else {
            let mut premiere = Segment::new(SegmentType::ChordRest, start, 1);
            for staff in [staff_cl, staff_fl] {
                let (chord, id) = accord(&mut score, staff == staff_cl, Duration::whole());
                lies.push(id);
                premiere.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
            }
            measure.add_segment(premiere);
        }
        score.add_measure(measure);
    }

    // Changement d'instrument au début de la seconde mesure.
    let mut concert = score.part(clarinette).instrument.clone();
    concert.transpose = None;
    score
        .part_mut(clarinette)
        .set_instrument_at(Fraction::ONE, concert);

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    let voix_cl = produit
        .split("staffOneVoiceOne = ")
        .nth(1)
        .expect("première voix");
    let voix_fl = produit
        .split("staffTwoVoiceOne = ")
        .nth(1)
        .expect("deuxième voix");

    assert!(voix_cl.contains("\\transposition bf"), "{voix_cl}");
    assert!(
        voix_cl.contains("\\transposition c"),
        "propriété de transposition non réinitialisée au changement d'instrument :\n{voix_cl}"
    );
    assert!(
        voix_cl.find("\\transposition bf").unwrap() < voix_cl.find("\\transposition c").unwrap(),
        "ordre des commandes de transposition :\n{voix_cl}"
    );
    assert!(
        !voix_fl.contains("\\transposition"),
        "la portée témoin ne doit pas porter de transposition :\n{voix_fl}"
    );
    // Hauteurs, durées, liaison traversant la barre et deux mesures : la
    // clarinette écrit mi/sol dièse/si (62 + 2), la flûte ré/fa dièse/la.
    for (voix, accord) in [(voix_cl, "<e~ gs b>1"), (voix_fl, "<d~ fs a>1")] {
        assert!(voix.contains(accord), "accord en ronde absent :\n{voix}");
        assert!(voix.contains("r2"), "blanche absente :\n{voix}");
        assert!(
            voix.contains(" |\n") && voix.contains("\\bar \"|.\""),
            "barres de contrôle des deux mesures absentes :\n{voix}"
        );
    }
    assert!(
        voix_cl.contains("<e gs b>2"),
        "accord lié de la seconde mesure absent :\n{voix_cl}"
    );
    assert!(
        voix_fl.contains("<d fs a>2"),
        "accord témoin de la seconde mesure absent :\n{voix_fl}"
    );
}

/// LilyPond 2.26 (version cible de l'oracle) ne connaît pas `\stanza` :
/// plusieurs couplets font échouer la compilation. La forme `\set stanza =
/// "n."`, utilisée par `musicxml2ly`, fonctionne sur 2.26 et est relue par
/// notre lecteur. Cas construit avec deux couplets, une portée, des accords,
/// des silences et une barre de contrôle.
#[test]
fn couplets_compilables_sous_lilypond_2_26() {
    let mut score = scoretk_core::model::Score::new();
    let part = score.add_part(Part::new("Voice", "Vo."));
    let staff = score.add_staff(part);
    let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
    for (tick, texte) in [(0, "premier"), (2, "second")] {
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::new(tick, 1), 1);
        let mut chord = ChordRest::chord(Duration::half());
        let note_id = score.add_note(Note::new(60, tpc::C));
        let ChordRestKind::Chord(ref mut content) = chord.kind else {
            unreachable!()
        };
        content.notes.push(note_id);
        chord
            .lyrics
            .push(Lyrics::new(0, LyricsSyllabic::Single, texte));
        chord
            .lyrics
            .push(Lyrics::new(1, LyricsSyllabic::Single, texte));
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
    }
    score.add_measure(measure);

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("\\set stanza = \"1.\""),
        "numéro de couplet 1 absent :\n{produit}"
    );
    assert!(
        produit.contains("\\set stanza = \"2.\""),
        "numéro de couplet 2 absent :\n{produit}"
    );
    assert!(
        !produit.contains("\\stanza"),
        "\\stanza n'est pas compilable par LilyPond 2.26 :\n{produit}"
    );
}

/// Un silence multi-mesures est marqué sur sa première mesure, mais les
/// mesures suivantes existent aussi dans le modèle avec un silence de mesure.
/// Écrire `R2.*N` puis N-1 fois `R2.` allonge la partition de N mesures et
/// décale toutes les attaques (cas réel : `01- Magnificat.mscz`).
#[test]
fn silence_multi_mesures_non_double() {
    let mut score = scoretk_core::model::Score::new();
    let part = score.add_part(Part::new("Voice", "Vo."));
    let staff = score.add_staff(part);
    let mesure = |score: &mut scoretk_core::model::Score,
                  index: usize,
                  start: Fraction,
                  len: Fraction,
                  multi: Option<u16>,
                  note: bool| {
        let mut measure = Measure::new(index, start, Fraction::new(3, 4));
        measure.len = len;
        measure.nominal_len = Fraction::new(3, 4);
        measure.multi_measure_rest = multi;
        let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
        let mut chord = if note {
            ChordRest::chord(Duration::whole())
        } else {
            ChordRest::rest(Duration::measure())
        };
        if note {
            let note_id = score.add_note(Note::new(60, tpc::C));
            let ChordRestKind::Chord(ref mut content) = chord.kind else {
                unreachable!()
            };
            content.notes.push(note_id);
        }
        segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
        measure.add_segment(segment);
        measure
    };
    let trois_quarts = Fraction::new(3, 4);
    for (index, start, len, multi, note) in [
        (0, Fraction::ZERO, Fraction::new(9, 4), Some(3), false),
        (1, trois_quarts, trois_quarts, None, false),
        (2, Fraction::new(3, 2), trois_quarts, None, false),
        (3, Fraction::new(9, 4), trois_quarts, None, true),
    ] {
        let measure = mesure(&mut score, index, start, len, multi, note);
        score.add_measure(measure);
    }

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("R2.*3"),
        "silence multi-mesures absent :\n{produit}"
    );
    assert_eq!(
        produit.matches("R2.").count(),
        1,
        "les mesures couvertes par R2.*3 ne doivent pas être réécrites :\n{produit}"
    );
    assert_eq!(
        produit.matches("c1").count(),
        1,
        "une seule note :\n{produit}"
    );
}

/// Les identifiants LilyPond ne peuvent pas contenir de chiffre : avec plus
/// de dix portées, `staff11VoiceOne` est analysé comme `staff` suivi de `11`
/// et toute la compilation échoue (cas réel : `Veni veni Emmanuel` à 13
/// portées). Le nom doit rester un identifiant valide, quelle que soit la
/// portée. Musique ordinaire : accords, silences et barres sur chaque portée.
/// Une mesure couverte par un `R2.*N` n'est redondante que si elle ne
/// contient que des silences. Les sources `.ly` écrivent parfois `R2*N`
/// puis une musique distincte : la supprimer ferait perdre des notes (cas
/// réel : `PriereASteJeanneDArc.ly`, 22 notes perdues).
#[test]
fn silence_multi_mesures_ne_mange_pas_les_notes_suivantes() {
    let mut score = scoretk_core::model::Score::new();
    let part = score.add_part(Part::new("Voice", "Vo."));
    let staff = score.add_staff(part);
    let trois_quarts = Fraction::new(3, 4);

    let mut premiere = Measure::new(0, Fraction::ZERO, trois_quarts);
    premiere.len = Fraction::new(6, 4);
    premiere.multi_measure_rest = Some(2);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    segment.set_element(
        Track::new(staff, 0),
        SegmentItem::ChordRest(ChordRest::rest(Duration::measure())),
    );
    premiere.add_segment(segment);
    score.add_measure(premiere);

    let mut suivante = Measure::new(1, trois_quarts, trois_quarts);
    let mut segment = Segment::new(SegmentType::ChordRest, Fraction::ZERO, 1);
    let mut chord = ChordRest::chord(Duration::whole());
    let note_id = score.add_note(Note::new(72, tpc::C));
    let ChordRestKind::Chord(ref mut content) = chord.kind else {
        unreachable!()
    };
    content.notes.push(note_id);
    segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
    suivante.add_segment(segment);
    score.add_measure(suivante);

    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("R2.*2"),
        "multi-silence absent :\n{produit}"
    );
    assert!(
        produit.contains("c'1"),
        "note de la mesure suivante supprimée :\n{produit}"
    );
}

#[test]
fn noms_de_variables_valides_au_dela_de_dix_portees() {
    let mut score = scoretk_core::model::Score::new();
    let part = score.add_part(Part::new("Choeur", "Ch."));
    for index in 0..12 {
        let staff = score.add_staff(part);
        let mut measure = Measure::new(0, Fraction::ZERO, Fraction::ONE);
        for (tick, silence) in [(0, false), (1, true)] {
            let mut segment = Segment::new(SegmentType::ChordRest, Fraction::new(tick, 1), 1);
            let chord = if silence {
                ChordRest::rest(Duration::half())
            } else {
                let mut chord = ChordRest::chord(Duration::half());
                let note_id = score.add_note(Note::new(60 + index as u8, tpc::C));
                let ChordRestKind::Chord(ref mut content) = chord.kind else {
                    unreachable!()
                };
                content.notes.push(note_id);
                chord
            };
            segment.set_element(Track::new(staff, 0), SegmentItem::ChordRest(chord));
            measure.add_segment(segment);
        }
        score.add_measure(measure);
    }
    let produit = scoretk_lilypond_export::write_lilypond(&score).expect("écriture");
    assert!(
        produit.contains("staffOneOneVoiceOne"),
        "nom de la onzième portée absent :\n{produit}"
    );
    for identifiant in produit
        .split_whitespace()
        .filter(|mot| mot.starts_with("staff"))
    {
        let nom = identifiant.trim_matches(|c: char| !c.is_alphanumeric());
        assert!(
            !nom.chars().any(|c| c.is_ascii_digit()),
            "identifiant LilyPond invalide {nom:?} :\n{produit}"
        );
    }
}

#[test]
fn clef_initiale_des_fichiers_v4() {
    for nom in ["v4/tenuto_accent.mscx", "v4/beamStemDir.mscx"] {
        let produit = convertir(&corpus().join(nom));
        assert!(
            produit.contains("\\clef bass"),
            "clef de fa absente dans {nom} :\n{produit}"
        );
    }
}
