// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// Portage de `engraving/dom/staff.cpp` (`Staff::updateOttava`, `pitchOffset`),
// `engraving/dom/note.cpp` (`Note::ppitch`, `ottaveCapoFret`) et
// `engraving/dom/ottava.h` (`ottavaDefault`) de MuseScore Studio, Copyright (C)
// MuseScore Limited and others, modifié (réécrit en Rust) en 2026.
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

//! Hauteur jouée sous une ottava.
//!
//! La norme SMF (§Channel Voice de MIDI 1.0, `docs/agents/format-ref/midi.md`)
//! ne définit pas les ottavas : le décalage est une décision de rendu de
//! MuseScore. On suit donc `Staff::updateOttava`/`pitchOffset`
//! (`engraving/dom/staff.cpp:1279-1301`) et `Note::ppitch`
//! (`engraving/dom/note.cpp:2668-2690`) : une ottava inscrit son `pitchShift`
//! au tick de début et `0` au tick de fin, et la hauteur jouée est la dernière
//! valeur inscrite à un tick ≤ celui de la note. Référence de comportement :
//! MIDI exporté par MuseScore 4.7.4 (hauteur et attaque), reproduit ici sans
//! fixture binaire.

use midly::{MidiMessage, Smf, TrackEventKind};
use scoretk_musescore_import::MsczContainer;

/// Attaque, touche et extinction d'une note du SMF, par piste.
fn notes(bytes: &[u8]) -> Vec<(usize, i64, u8, i64)> {
    let smf = Smf::parse(bytes).expect("SMF valide");
    let mut result = Vec::new();
    for (track_index, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0i64;
        let mut open: Vec<(u8, i64)> = Vec::new();
        for event in track {
            tick += i64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
            {
                let key = key.as_int();
                if vel.as_int() > 0 {
                    open.push((key, tick));
                } else if let Some(position) = open.iter().position(|(pitch, _)| *pitch == key) {
                    let (pitch, start) = open.remove(position);
                    result.push((track_index, start, pitch, tick));
                }
            }
        }
        assert!(
            open.is_empty(),
            "note non éteinte sur la piste {track_index}"
        );
    }
    result.sort_unstable();
    result
}

/// Partition 4.10 minimale dont on fournit le contenu des portées.
fn mscx(top: &str, bottom: Option<&str>) -> String {
    let (parts, bottom_staff) = match bottom {
        Some(body) => (
            r#"
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <Staff id="2"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>"#,
            format!("    <Staff id=\"2\">\n{body}    </Staff>\n"),
        ),
        None => (
            r#"
    <Part id="1">
      <Staff id="1"><StaffType group="pitched"><name>stdNormal</name></StaffType></Staff>
      <trackName>Piano</trackName>
      <Instrument id="keyboard.piano">
        <longName>Piano</longName><shortName>Pno.</shortName><trackName>Piano</trackName>
        <instrumentId>keyboard.piano</instrumentId>
        <Channel><program value="0"/></Channel>
      </Instrument>
    </Part>"#,
            String::new(),
        ),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.10">
  <Score>
    <Division>480</Division>{parts}
    <Staff id="1">
{top}    </Staff>
{bottom_staff}  </Score>
</museScore>
"#
    )
}

/// Début d'ottava : `where` est le contenu de `<location>`.
fn ottava_start(subtype: &str, where_: &str) -> String {
    format!(
        "          <Spanner type=\"Ottava\">\n\
           <Ottava><subtype>{subtype}</subtype></Ottava>\n\
           <next><location>{where_}</location></next>\n\
           </Spanner>\n"
    )
}

fn ottava_stop(where_: &str) -> String {
    format!(
        "          <Spanner type=\"Ottava\">\n\
           <prev><location>{where_}</location></prev>\n\
           </Spanner>\n"
    )
}

fn chord(notes: &str) -> String {
    format!("          <Chord><durationType>quarter</durationType>{notes}</Chord>\n")
}

fn note(pitch: u8, tpc: i8) -> String {
    format!("<Note><pitch>{pitch}</pitch><tpc>{tpc}</tpc></Note>")
}

fn tied_note(pitch: u8, tpc: i8, half: &str) -> String {
    format!(
        "<Note><Spanner type=\"Tie\"><Tie></Tie>{half}</Spanner>\
         <pitch>{pitch}</pitch><tpc>{tpc}</tpc></Note>"
    )
}

fn render(source: &str) -> Vec<u8> {
    let container = MsczContainer::from_bytes(source.as_bytes()).expect("conteneur");
    let score = scoretk_musescore_import::read::read_score(&container).expect("lecture .mscx");
    scoretk_midi_export::write_midi(&score).expect("écriture MIDI")
}

#[test]
fn ottava_8va_sur_deux_notes_puis_fin() {
    // Référence MuseScore 4.7.4 : deux noires sous 8va (C4→C5, D4→D5), puis
    // une noire hors ottava (E4) à la mesure suivante.
    let source = mscx(
        &format!(
            "      <Measure>\n        <voice>\n\
             {}\
             {}\
             {}\
             </voice>\n      </Measure>\n\
             <Measure>\n        <voice>\n\
             {}\
             {}\
             </voice>\n      </Measure>\n",
            "          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>\n",
            ottava_start("8va", "<measures>1</measures>"),
            chord(&note(60, 14)) + &chord(&note(62, 16)),
            ottava_stop("<measures>-1</measures>"),
            chord(&note(64, 18)),
        ),
        None,
    );
    assert_eq!(
        notes(&render(&source)),
        vec![(0, 0, 72, 479), (0, 480, 74, 959), (0, 1920, 64, 2399)]
    );
}

#[test]
fn ottava_par_portee_avec_accord_silence_et_lien() {
    // Musique ordinaire : deux portées, un accord, un silence et deux noires
    // liées sous l'ottava. Seule la portée qui porte l'ottava est transposée.
    let top = format!(
        "      <Measure>\n        <voice>\n\
         {}\
         {}\
         {}\
         {}\
         {}\
         </voice>\n      </Measure>\n\
         <Measure>\n        <voice>\n\
         {}\
         {}\
         {}\
         </voice>\n      </Measure>\n",
        "          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>\n",
        ottava_start("8va", "<measures>1</measures>"),
        chord(&format!("{}{}", note(60, 14), note(64, 18))),
        "          <Rest><durationType>quarter</durationType></Rest>\n",
        chord(&tied_note(
            62,
            16,
            "<next><location><fractions>1/2</fractions></location></next>"
        )),
        ottava_stop("<measures>-1</measures>"),
        chord(&tied_note(
            62,
            16,
            "<prev><location><fractions>-1/2</fractions></location></prev>"
        )),
        chord(&note(60, 14)),
    );
    let bottom = format!(
        "      <Measure>\n        <voice>\n\
         {}\
         {}\
         </voice>\n      </Measure>\n\
         <Measure>\n        <voice>\n\
         {}\
         {}\
         </voice>\n      </Measure>\n",
        ottava_start("8vb", "<measures>1</measures>"),
        chord(&note(48, 14)) + &chord(&note(50, 16)),
        ottava_stop("<measures>-1</measures>"),
        chord(&note(48, 14)),
    );
    // La portée haute (piste 0) joue 72/76 sous 8va, la note liée à 74 dure
    // deux noires, puis la note hors ottava reste 60. La basse (piste 1) joue
    // une octave plus bas sous 8vb, et retrouve 48 après la fin.
    assert_eq!(
        notes(&render(&mscx(&top, Some(&bottom)))),
        vec![
            (0, 0, 72, 479),
            (0, 0, 76, 479),
            (0, 960, 74, 1919),
            (0, 2400, 60, 2879),
            (1, 0, 36, 479),
            (1, 480, 38, 959),
            (1, 1920, 48, 2399),
        ]
    );
}

#[test]
fn ottava_fin_en_milieu_de_mesure_et_cas_sans_ottava() {
    // Cas limite : la fin de l'ottava tombe sur le troisième temps. La note
    // qui commence exactement au tick de fin n'est plus décalée. Le même
    // extrait sans les deux spanners doit rester identique à avant.
    let with = mscx(
        &format!(
            "      <Measure>\n        <voice>\n\
             {}\
             {}\
             {}\
             {}\
             {}\
             </voice>\n      </Measure>\n",
            "          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>\n",
            ottava_start("8va", "<fractions>1/2</fractions>"),
            chord(&note(60, 14)) + &chord(&note(62, 16)),
            ottava_stop("<fractions>-1/2</fractions>"),
            chord(&note(64, 18)),
        ),
        None,
    );
    let without = mscx(
        &format!(
            "      <Measure>\n        <voice>\n\
             {}\
             {}\
             </voice>\n      </Measure>\n",
            "          <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>\n",
            chord(&note(60, 14)) + &chord(&note(62, 16)) + &chord(&note(64, 18)),
        ),
        None,
    );
    assert_eq!(
        notes(&render(&with)),
        vec![(0, 0, 72, 479), (0, 480, 74, 959), (0, 960, 64, 1439)]
    );
    assert_eq!(
        notes(&render(&without)),
        vec![(0, 0, 60, 479), (0, 480, 62, 959), (0, 960, 64, 1439)]
    );
}

#[test]
fn octave_shift_musicxml_joue_comme_ottava() {
    // `<octave-shift>` est lu par `scoretk-musicxml-import` comme un
    // `SpannerType::Ottava` : la chaîne MusicXML → modèle → MIDI doit donc
    // transposer de la même façon que le lecteur MuseScore.
    let musicxml = r#"<?xml version="1.0" encoding="utf-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef></attributes>
      <direction placement="above"><direction-type><octave-shift type="down" size="8" number="1"/></direction-type></direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
    </measure>
    <measure number="2">
      <direction placement="above"><direction-type><octave-shift type="stop" size="8" number="1"/></direction-type></direction>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"#;
    // L'import MusicXML retire le décalage de la hauteur lue (le modèle stocke
    // la hauteur écrite, 48/50 ici) ; le rendu le réapplique et retrouve les
    // hauteurs sonores du XML, 60/62, puis 64 hors ottava.
    let score =
        scoretk_musicxml_import::read_musicxml(musicxml.as_bytes()).expect("lecture MusicXML");
    let bytes = scoretk_midi_export::write_midi(&score).expect("écriture MIDI");
    assert_eq!(
        notes(&bytes),
        vec![(0, 0, 60, 479), (0, 480, 62, 959), (0, 960, 64, 1439)]
    );
}
