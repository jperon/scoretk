// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 jperon
//! Détection d'un n-olet 3:2 avec un accord intercalé.
//!
//! `OSalutarisHostia` et `Ackermans_NoelDesPetitsSantons` portent des n-olets
//! 3:2 (`<time-modification>3/2`) dont un accord étranger tombe entre deux
//! positions. L'ancien détecteur exigeait une grille sans accord intercalé et
//! n'en trouvait aucun ; `findTupletApproximation` retient un accord par
//! position puis `addChordsBetweenTupletNotes` ajoute les accords intermédiaires.
mod common;
use common::{smf_with, Ev};
use midly::num::u15;
use midly::{Format, Timing};
use scoretk_core::model::{ChordRestKind, Fraction, Score, SegmentItem};

fn tuplet_elements(score: &Score) -> Option<(Fraction, usize)> {
    let tuplet = score.tuplets.first()?;
    let first_id = scoretk_core::model::TupletId::from_index(0);
    let mut count = 0;
    for measure in &score.measures {
        for segment in &measure.segments {
            for (_, item) in segment.iter_elements() {
                if let SegmentItem::ChordRest(cr) = item {
                    if let ChordRestKind::Chord(_) = &cr.kind {
                        if cr.tuplet == Some(first_id) {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    Some((tuplet.ratio, count))
}

#[test]
fn nolet_trois_deux_avec_accord_intercale() {
    let mut events = vec![(0, Ev::TimeSig(4, 2)), (0, Ev::Tempo(500_000))];
    // 3:2 sur une blanche : positions 0, 4/3, 8/3 ; accord étranger à 2,0.
    for (on, pitch, len) in [
        (0.0, 65u8, 4.0 / 3.0),
        (4.0 / 3.0, 67, 2.0 / 3.0),
        (2.0, 64, 2.0 / 3.0),
        (8.0 / 3.0, 65, 2.0 / 3.0),
        (4.0, 65, 1.0),
    ] {
        events.push(((on * 480.0) as u32, Ev::NoteOn(0, pitch, 90)));
        events.push((((on + len) * 480.0) as u32, Ev::NoteOff(0, pitch)));
    }
    events.sort_by_key(|(tick, _)| *tick);
    let bytes = smf_with(Format::Parallel, Timing::Metrical(u15::new(480)), &[events]);
    let options = scoretk_midi_import::ImportOptions {
        tuplet_notation: true,
        ..scoretk_midi_import::ImportOptions::default()
    };
    let score = scoretk_midi_import::read_midi_with(&bytes, &options).expect("lecture");
    let (ratio, count) = tuplet_elements(&score).expect("n-olet détecté");
    assert_eq!(ratio, Fraction::new(3, 2), "ratio du triolet");
    assert!(
        count >= 4,
        "les trois positions et l'accord intercalé : {count}"
    );
}
