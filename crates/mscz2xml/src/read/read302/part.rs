//! Parties, portées et instruments.
//!
//! Portage de `TRead::read(Part*)` et `readProperties(Part*)`
//! (`engraving/rw/read400/tread.cpp`) ainsi que de `Read302::fixInstrumentId`
//! (`engraving/rw/read302/read302.cpp`).

use super::tables;
use super::xml::Node;
use super::Reader;
use crate::model::{Clef, ClefType, Instrument, Part, Staff, StaffBracket, Transpose};

/// Lit un `<Part>` : portées déclarées, noms et instrument principal.
pub(crate) fn read_part(reader: &mut Reader, node: &Node) {
    let instrument_node = node.child("Instrument");
    let instrument = instrument_node
        .map(|child| read_instrument(reader, child))
        .unwrap_or_default();
    // `Staff::init` prend la clef par défaut sur l'instrument ; `<defaultClef>`
    // la remplace ensuite.
    let instrument_clefs = instrument_node
        .map(read_instrument_clefs)
        .unwrap_or_default();

    let long_name = node
        .child("name")
        .map(Node::trimmed_text)
        .or_else(|| node.child("longName").map(Node::trimmed_text))
        .unwrap_or_else(|| instrument.long_name.clone());
    let short_name = node
        .child("shortName")
        .map(Node::trimmed_text)
        .unwrap_or_else(|| instrument.short_name.clone());
    let name = node
        .child("trackName")
        .map(Node::trimmed_text)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            let track_name = node
                .child("Instrument")
                .and_then(|instrument| instrument.child("trackName"))
                .map(Node::trimmed_text);
            track_name.filter(|name| !name.is_empty())
        })
        .unwrap_or_else(|| long_name.clone());

    let mut part = Part::new(long_name.clone(), short_name.clone());
    part.name = name;
    part.instrument = Instrument {
        long_name: long_name.clone(),
        short_name: short_name.clone(),
        ..instrument
    };
    let part_id = reader.score.add_part(part);

    let mut declared = 0usize;
    for staff_node in node.children_named("Staff") {
        let mut staff = Staff::new(part_id);
        let mut clef = None;
        if let Some(staff_type) = staff_node.child("StaffType") {
            if let Some(lines) = staff_type.child("lines").and_then(Node::int_text) {
                staff.lines = lines.clamp(1, 32) as u8;
            }
            staff.small = staff_type
                .child("small")
                .and_then(Node::int_text)
                .unwrap_or(0)
                != 0;
            clef = match staff_type.attr("group") {
                Some("tab") => Some(Clef::new(ClefType::Tab)),
                Some("percussion") => Some(Clef::new(ClefType::Perc)),
                _ => None,
            };
        }
        if staff_node
            .child("small")
            .and_then(Node::int_text)
            .unwrap_or(0)
            != 0
        {
            staff.small = true;
        }
        // `<barLineSpan>` : la valeur 1 vaut « barre commune avec la portée
        // suivante », comme `TRead::read(Staff*)`.
        if let Some(value) = staff_node.child("barLineSpan").and_then(Node::int_text) {
            staff.bar_line_span = value != 0;
        }
        for bracket in staff_node.children_named("bracket") {
            let kind = bracket.attr_i32("type").unwrap_or(-1);
            if kind < 0 {
                continue;
            }
            staff.brackets.push(StaffBracket {
                kind: kind.clamp(0, 127) as i8,
                span: bracket.attr_i32("span").unwrap_or(0).max(0) as usize,
                visible: bracket.attr_i32("visible").unwrap_or(1) != 0,
            });
        }
        // `<defaultClef>` fixe les deux clefs ; les variantes concert et
        // transposée une seule, en partant de sol/sol comme `ClefTypeList`.
        let mut declared_clef = false;
        for tag in [
            "defaultClef",
            "defaultConcertClef",
            "defaultTransposingClef",
        ] {
            let Some(value) = staff_node.child(tag) else {
                continue;
            };
            let Some(clef_type) = tables::clef_type(&value.trimmed_text()) else {
                continue;
            };
            declared_clef = true;
            let pair = clef.get_or_insert(Clef::new(ClefType::G));
            match tag {
                "defaultClef" => *pair = Clef::new(clef_type),
                "defaultConcertClef" => pair.concert = clef_type,
                _ => pair.transposed = clef_type,
            }
        }
        if !declared_clef {
            if let Some(instrument_clef) = instrument_clefs.get(declared) {
                clef = Some(*instrument_clef);
            }
        }
        staff.default_clef = clef.unwrap_or(Clef::new(ClefType::G));
        reader.staff_clefs.push(clef);
        reader.score.add_staff_with(part_id, staff);
        declared += 1;
    }
    // TODO(parité): les instruments secondaires déclarés directement dans la
    // partie (`<Instrument>` successifs) ne sont pas modélisés ; les
    // changements d'instrument le sont par `instruments.rs`.
}

pub(crate) fn read_instrument(reader: &Reader, node: &Node) -> Instrument {
    let track_name = node
        .child("trackName")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let mut attribute_id = node.attr("id").unwrap_or_default().to_owned();
    if reader.version.as_u32() == 302 {
        // MuseScore 3.6.x écrivait des identifiants d'instrument erronés.
        attribute_id = fix_instrument_id(&attribute_id, &track_name);
    }
    let musicxml_id = node
        .child("instrumentId")
        .map(Node::trimmed_text)
        .unwrap_or_default();
    let mut instrument = Instrument {
        id: if musicxml_id.is_empty() {
            // L'identifiant d'attribut n'est un identifiant MusicXML que pour
            // les fichiers qui n'en écrivent pas ; il sert surtout de repli
            // pour `fixInstrumentId`.
            attribute_id
        } else {
            musicxml_id
        },
        long_name: node
            .child("longName")
            .map(Node::trimmed_text)
            .unwrap_or_default(),
        short_name: node
            .child("shortName")
            .map(Node::trimmed_text)
            .unwrap_or_default(),
        track_name,
        transpose: None,
        midi_program: None,
        midi_channel: None,
        volume: None,
        pan: None,
    };

    let diatonic = node.child("transposeDiatonic").and_then(Node::int_text);
    let chromatic = node.child("transposeChromatic").and_then(Node::int_text);
    if diatonic.is_some() || chromatic.is_some() {
        instrument.transpose = Some(Transpose::new(
            diatonic.unwrap_or(0),
            chromatic.unwrap_or(0),
        ));
    }

    // Programme MIDI du premier canal sonore (`<Channel><program value>`).
    for channel in node.children_named("Channel") {
        if let Some(program) = channel.child("program") {
            let value = program.attr_i32("value").or_else(|| program.int_text());
            if let Some(value) = value {
                if (0..=127).contains(&value) {
                    instrument.midi_program = Some(value as u8);
                }
                break;
            }
        }
    }

    // Canal MIDI du premier canal sonore (`<Channel><midiChannel>`, base zéro).
    instrument.midi_channel = node
        .children_named("Channel")
        .next()
        .and_then(|channel| channel.child("midiChannel"))
        .and_then(Node::int_text)
        .and_then(|value| u8::try_from(value.clamp(0, 127)).ok());

    // Les partitions antérieures à 3.6 ne stockent ni identifiant MusicXML ni
    // programme par défaut : `Instrument::updateInstrumentId` les retrouve dans
    // le catalogue à partir des noms. `read(Instrument*)` complète ensuite un
    // programme absent, y compris en 3.6.
    let (recognized_id, recognized_program) = crate::read::instruments::recognize(
        &instrument.id,
        [
            instrument.track_name.as_str(),
            instrument.long_name.as_str(),
            instrument.short_name.as_str(),
        ],
        instrument.midi_program.map(i16::from),
    );
    if reader.version.as_u32() != 302 {
        instrument.id = recognized_id;
    }
    if instrument.midi_program.is_none() {
        instrument.midi_program = recognized_program.and_then(|value| u8::try_from(value).ok());
    }

    // Réglages de canal : contrôleur 7 (volume) et 10 (panoramique).
    for channel in node.children_named("Channel") {
        for controller in channel.children_named("controller") {
            let Some(value) = controller
                .attr_i32("value")
                .or_else(|| controller.int_text())
                .filter(|value| (0..=127).contains(value))
            else {
                continue;
            };
            match controller.attr_i32("ctrl") {
                Some(7) => instrument.volume = Some(value as u8),
                Some(10) => instrument.pan = Some(value as u8),
                _ => {}
            }
        }
    }

    instrument
}

/// Clefs par portée d'un `<Instrument>` (`<clef>`, `<concertClef>`,
/// `<transposingClef>`, attribut `staff` à partir de 1).
///
/// `Instrument::setClefType` crée les entrées manquantes avec la paire sol/sol ;
/// `<concertClef>` et `<transposingClef>` ne changent qu'une moitié.
fn read_instrument_clefs(node: &Node) -> Vec<Clef> {
    let mut clefs: Vec<Clef> = Vec::new();
    for child in node.children() {
        let change = match child.name() {
            "clef" => ClefChange::Both,
            "concertClef" => ClefChange::Concert,
            "transposingClef" => ClefChange::Transposed,
            _ => continue,
        };
        let index = child
            .attr("staff")
            .and_then(|value| value.trim().parse::<usize>().ok())
            .map_or(0, |value| value.saturating_sub(1));
        let Some(kind) = tables::clef_type(&child.trimmed_text()) else {
            continue;
        };
        while clefs.len() <= index {
            clefs.push(Clef::new(ClefType::G));
        }
        match change {
            ClefChange::Both => clefs[index] = Clef::new(kind),
            ClefChange::Concert => clefs[index].concert = kind,
            ClefChange::Transposed => clefs[index].transposed = kind,
        }
    }
    clefs
}

/// Moitié de `ClefTypeList` modifiée par une balise de clef d'instrument.
#[derive(Clone, Copy)]
enum ClefChange {
    Both,
    Concert,
    Transposed,
}

/// Corrige les identifiants d'instrument faux de MuseScore 3.6.x.
///
/// Transcription de `Read302::fixInstrumentId`.
fn fix_instrument_id(id: &str, track_name: &str) -> String {
    let track_name = track_name.to_lowercase();
    let fixed = match id {
        "Winds" => Some("winds"),
        "harmonica-d12high-g" => Some("harmonica-d10high-g"),
        "harmonica-d12f" => Some("harmonica-d10f"),
        "harmonica-d12d" => Some("harmonica-d10d"),
        "harmonica-d12c" => Some("harmonica-d10c"),
        "harmonica-d12a" => Some("harmonica-d10a"),
        "harmonica-d12-g" => Some("harmonica-d10g"),
        "drumset" if track_name == "percussion" => Some("percussion"),
        "cymbal" if track_name == "cymbals" => Some("marching-cymbals"),
        "bass-drum" if track_name == "bass drums" => Some("marching-bass-drums"),
        _ => None,
    };
    if let Some(fixed) = fixed {
        return fixed.to_owned();
    }

    let Some(model) = id.strip_prefix("mdl-") else {
        return id.to_owned();
    };
    let fixed = match model {
        "snareline" | "snareline-a" | "snaresolo" | "snaresolo-a" => "marching-snare",
        "tenorline" | "tenorsolo" | "flubs" => "marching-tenor-drums",
        "bassline-10" | "bassline-5" => "marching-bass-drums",
        "cymballine" => "marching-cymbals",
        "showtenorline" | "rail" | "drumset" | "sampler" => "drumset",
        _ => id,
    };
    fixed.to_owned()
}
