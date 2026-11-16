//! Liaisons de prolongation, liaisons d'expression et spanners.
//!
//! En 3.x, un connecteur s'écrit en deux moitiés `<Spanner type="…">`, chacune
//! portant une extrémité implicite (sa position) et l'autre extrémité dans
//! `<prev>` ou `<next>` sous forme de `<location>` relative. Les moitiés sont
//! appariées en fin de lecture, comme `ConnectorInfoReader::read` et
//! `ReadContext::addConnectorInfo` (`engraving/rw/read400/`).
//!
//! `Note::tie_forward`/`tie_back` étant des identifiants de notes, les liaisons
//! de prolongation sont recréées à l'appariement ; une liaison sans extrémité
//! est rattachée par `searchTieNote` (`Score::connectTies`).

use super::tables;
use super::xml::Node;
use super::{Reader, VoiceCtx};
use crate::model::{
    Fraction, Measure, NoteId, SegmentType, Spanner, SpannerAnchor, SpannerType, Track, VoltaInfo,
    VOICES,
};

/// Position d'une extrémité de connecteur.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Anchor {
    /// Tick absolu depuis le début de la partition.
    tick: Fraction,
    /// Mesure contenant ce tick.
    measure: usize,
    /// Piste d'attache.
    track: Track,
    /// Indice de l'ornement (`Chord::graceIndex`) ; `None` pour un accord normal.
    grace: Option<usize>,
    /// Indice de la note dans son accord ; `None` pour un accord ou une mesure.
    note: Option<usize>,
}

impl Anchor {
    fn same_position(self, other: Self) -> bool {
        self.tick == other.tick
            && self.track == other.track
            && self.grace == other.grace
            && self.note == other.note
    }
}

/// `<location>` relative à l'extrémité courante.
///
/// Portage de `Location` (`engraving/dom/location.h`) limité aux champs écrits
/// par les lecteurs 3.x.
#[derive(Clone, Copy, Default)]
struct Rel {
    measures: i32,
    fractions: Fraction,
    staves: i32,
    voices: i32,
    notes: i32,
}

impl Rel {
    /// Position absolue de l'extrémité, `Location::toAbsolute`.
    fn absolute(self, current: Anchor, measures: &[Measure]) -> Anchor {
        let target = current.measure as i64 + i64::from(self.measures);
        let target = usize::try_from(target)
            .ok()
            .filter(|index| *index < measures.len());
        let base = target
            .and_then(|index| measures.get(index))
            .map_or(Fraction::ZERO, |measure| measure.start_tick);
        let current_base = measures
            .get(current.measure)
            .map_or(Fraction::ZERO, |measure| measure.start_tick);
        let raw_track = i64::from(current.track.raw())
            + i64::from(self.voices)
            + i64::from(self.staves) * VOICES as i64;
        Anchor {
            tick: base + (current.tick - current_base) + self.fractions,
            measure: target.unwrap_or(current.measure),
            track: Track::from_raw(raw_track.max(0) as u32),
            // `Location::toAbsolute` n'additionne jamais l'indice d'ornement : une
            // extrémité relative garde la valeur « non renseignée », qui ne peut
            // correspondre qu'à un accord normal (comportement du C++).
            grace: None,
            note: current
                .note
                .map(|index| (index as i64 + i64::from(self.notes)).max(0) as usize),
        }
    }
}

/// Ce à quoi une moitié de connecteur est accrochée.
#[derive(Clone, Copy)]
enum Receiver {
    Note(NoteId),
    Other,
}

/// Contenu lu dans l'enfant du `<Spanner>` portant le même nom que son `type`.
#[derive(Clone)]
enum Payload {
    Tie,
    Slur,
    Spanner(SpannerType, Option<String>, Option<VoltaInfo>),
}

/// Moitié de connecteur en attente d'appariement.
pub(crate) struct Connector {
    name: String,
    anchor: Anchor,
    prev: Option<Rel>,
    next: Option<Rel>,
    receiver: Receiver,
    payload: Option<Payload>,
    abs_prev: Option<Anchor>,
    abs_next: Option<Anchor>,
    prev_link: Option<usize>,
    next_link: Option<usize>,
}

/// Lit un `<Spanner>` rattaché à la mesure (soufflet, pédale, ottava…).
pub(crate) fn read_spanner(reader: &mut Reader, node: &Node, ctx: &VoiceCtx) {
    read(reader, node, ctx, Receiver::Other, None, None);
}

/// Lit un `<Spanner>` rattaché à un accord (liaison d'expression).
pub(crate) fn read_chord_spanner(reader: &mut Reader, node: &Node, ctx: &VoiceCtx) {
    read(reader, node, ctx, Receiver::Other, None, None);
}

/// Lit un `<Spanner>` rattaché à une note (liaison, glissando…).
pub(crate) fn read_note_spanner(
    reader: &mut Reader,
    node: &Node,
    ctx: &VoiceCtx,
    note: NoteId,
    grace: Option<usize>,
    index: usize,
) {
    read(reader, node, ctx, Receiver::Note(note), grace, Some(index));
}

fn read(
    reader: &mut Reader,
    node: &Node,
    ctx: &VoiceCtx,
    receiver: Receiver,
    grace: Option<usize>,
    note: Option<usize>,
) {
    let Some(name) = node.attr("type") else {
        return;
    };
    let name = name.to_owned();
    let mut prev = None;
    let mut next = None;
    let mut payload = None;

    for child in node.children() {
        match child.name() {
            "prev" => prev = child.child("location").map(read_rel),
            "next" => next = child.child("location").map(read_rel),
            tag if tag == name => payload = read_payload(child),
            _ => {}
        }
    }

    reader.connectors.push(Connector {
        name,
        anchor: Anchor {
            tick: ctx.tick,
            measure: ctx.measure,
            track: ctx.track,
            grace,
            note,
        },
        prev,
        next,
        receiver,
        payload,
        abs_prev: None,
        abs_next: None,
        prev_link: None,
        next_link: None,
    });
}

fn read_rel(node: &Node) -> Rel {
    let mut rel = Rel::default();
    for child in node.children() {
        match child.name() {
            "measures" => rel.measures = child.int_text().unwrap_or(0),
            "fractions" => rel.fractions = child.fraction().unwrap_or(Fraction::ZERO),
            "staves" => rel.staves = child.int_text().unwrap_or(0),
            "voices" => rel.voices = child.int_text().unwrap_or(0),
            "notes" => rel.notes = child.int_text().unwrap_or(0),
            // TODO(parité): `<grace>` n'est pas propagé par `toAbsolute` en 3.x.
            _ => {}
        }
    }
    rel
}

fn read_payload(node: &Node) -> Option<Payload> {
    let payload = match node.name() {
        "Tie" => Payload::Tie,
        "Slur" => Payload::Slur,
        "HairPin" => Payload::Spanner(
            SpannerType::Hairpin(tables::hairpin_type(node)),
            text_of(node),
            None,
        ),
        "Ottava" => Payload::Spanner(SpannerType::Ottava(tables::ottava_type(node)), None, None),
        "Pedal" => Payload::Spanner(SpannerType::Pedal, text_of(node), None),
        "Volta" => Payload::Spanner(
            SpannerType::Volta,
            text_of(node),
            Some(super::repeats::read_volta(node)),
        ),
        "TextLine" => Payload::Spanner(SpannerType::TextLine, text_of(node), None),
        "Trill" => Payload::Spanner(SpannerType::Trill, None, None),
        "Glissando" => Payload::Spanner(SpannerType::Glissando, None, None),
        other => Payload::Spanner(SpannerType::Other(other.to_owned()), text_of(node), None),
    };
    Some(payload)
}

fn text_of(node: &Node) -> Option<String> {
    ["beginText", "text"]
        .iter()
        .find_map(|name| node.child(name))
        .map(Node::text)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Apparie les moitiés de connecteurs et crée les objets du modèle.
pub(crate) fn resolve(reader: &mut Reader) {
    let absolute: Vec<(Option<Anchor>, Option<Anchor>)> = reader
        .connectors
        .iter()
        .map(|connector| {
            (
                connector
                    .prev
                    .map(|rel| rel.absolute(connector.anchor, &reader.score.measures)),
                connector
                    .next
                    .map(|rel| rel.absolute(connector.anchor, &reader.score.measures)),
            )
        })
        .collect();
    for (connector, (prev, next)) in reader.connectors.iter_mut().zip(absolute) {
        connector.abs_prev = prev;
        connector.abs_next = next;
    }

    let mut active: Vec<usize> = Vec::new();
    for index in 0..reader.connectors.len() {
        active.push(index);
        let mut linked = false;
        for &other in active.iter().take(active.len().saturating_sub(1)) {
            if connect(&mut reader.connectors, other, index) {
                linked = true;
                break;
            }
        }
        if !linked {
            continue;
        }
        let first = chain_first(&reader.connectors, index);
        let last = chain_last(&reader.connectors, index);
        let finished =
            reader.connectors[first].prev.is_none() && reader.connectors[last].next.is_none();
        if finished {
            let chain = chain_of(&reader.connectors, first);
            emit(reader, &chain);
            active.retain(|candidate| !chain.contains(candidate));
        }
    }

    // Liaisons de prolongation sans extrémité : `Score::connectTies` cherche la
    // note suivante de même hauteur sur la même piste.
    let dangling: Vec<(NoteId, Anchor)> = reader
        .connectors
        .iter()
        .filter(|connector| {
            connector.next.is_some()
                && connector.next_link.is_none()
                && matches!(connector.payload, Some(Payload::Tie))
        })
        .filter_map(|connector| match connector.receiver {
            Receiver::Note(note) => Some((note, connector.anchor)),
            Receiver::Other => None,
        })
        .collect();
    for (note, anchor) in dangling {
        search_tie_note(reader, note, anchor);
    }
}

fn connect(connectors: &mut [Connector], first: usize, second: usize) -> bool {
    if connectors[first].name != connectors[second].name {
        return false;
    }

    let first_ends = connectors[first].next.is_some() && connectors[first].next_link.is_none();
    let second_starts = connectors[second].prev.is_some() && connectors[second].prev_link.is_none();
    if first_ends
        && second_starts
        && same(connectors[first].abs_next, Some(connectors[second].anchor))
        && same(connectors[second].abs_prev, Some(connectors[first].anchor))
    {
        connectors[first].next_link = Some(second);
        connectors[second].prev_link = Some(first);
        return true;
    }

    let first_starts = connectors[first].prev.is_some() && connectors[first].prev_link.is_none();
    let second_ends = connectors[second].next.is_some() && connectors[second].next_link.is_none();
    if first_starts
        && second_ends
        && same(connectors[first].abs_prev, Some(connectors[second].anchor))
        && same(connectors[second].abs_next, Some(connectors[first].anchor))
    {
        connectors[first].prev_link = Some(second);
        connectors[second].next_link = Some(first);
        return true;
    }

    false
}

fn same(left: Option<Anchor>, right: Option<Anchor>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same_position(right),
        _ => false,
    }
}

fn chain_first(connectors: &[Connector], start: usize) -> usize {
    let mut current = start;
    while let Some(previous) = connectors[current].prev_link {
        if previous == start {
            break;
        }
        current = previous;
    }
    current
}

fn chain_last(connectors: &[Connector], start: usize) -> usize {
    let mut current = start;
    while let Some(next) = connectors[current].next_link {
        if next == start {
            break;
        }
        current = next;
    }
    current
}

fn chain_of(connectors: &[Connector], first: usize) -> Vec<usize> {
    let mut chain = vec![first];
    let mut current = first;
    while let Some(next) = connectors[current].next_link {
        if chain.contains(&next) {
            break;
        }
        chain.push(next);
        current = next;
    }
    chain
}

fn emit(reader: &mut Reader, chain: &[usize]) {
    let (Some(&first), Some(&last)) = (chain.first(), chain.last()) else {
        return;
    };
    let start = reader.connectors[first].anchor;
    let end = reader.connectors[last].anchor;
    let payload = chain
        .iter()
        .find_map(|&index| reader.connectors[index].payload.clone());
    let notes: Vec<NoteId> = chain
        .iter()
        .filter_map(|&index| match reader.connectors[index].receiver {
            Receiver::Note(note) => Some(note),
            Receiver::Other => None,
        })
        .collect();

    match payload {
        Some(Payload::Tie) => {
            for pair in notes.windows(2) {
                let (start_note, end_note) = (pair[0], pair[1]);
                reader.score.note_mut(start_note).tie_forward = Some(end_note);
                reader.score.note_mut(end_note).tie_back = Some(start_note);
            }
        }
        Some(Payload::Slur) => push_spanner(reader, SpannerType::Slur, None, None, start, end),
        Some(Payload::Spanner(kind, text, volta)) => {
            push_spanner(reader, kind, text, volta, start, end)
        }
        None => {}
    }
}

fn push_spanner(
    reader: &mut Reader,
    kind: SpannerType,
    text: Option<String>,
    volta: Option<VoltaInfo>,
    start: Anchor,
    end: Anchor,
) {
    let mut spanner = Spanner::new(
        kind,
        SpannerAnchor::new(start.tick, start.track),
        SpannerAnchor::new(end.tick, end.track),
    );
    spanner.text = text;
    spanner.volta = volta;
    reader.score.add_spanner(spanner);
}

fn search_tie_note(reader: &mut Reader, note: NoteId, from: Anchor) {
    let pitch = reader.score.note(note).pitch;
    let mut found = None;
    'measures: for index in from.measure..reader.score.measures.len() {
        let measure = &reader.score.measures[index];
        for segment in &measure.segments {
            if segment.kind != SegmentType::ChordRest {
                continue;
            }
            if index == from.measure && segment.tick <= from.tick {
                continue;
            }
            let Some(chord_rest) = segment.chord_rest(from.track) else {
                continue;
            };
            if let Some(chord) = chord_rest.chord_ref() {
                if let Some(&candidate) = chord
                    .notes
                    .iter()
                    .find(|&&id| reader.score.note(id).pitch == pitch)
                {
                    found = Some(candidate);
                }
            }
            // Le C++ s'arrête au premier accord ou silence de la piste.
            break 'measures;
        }
    }

    if let Some(end) = found {
        reader.score.note_mut(note).tie_forward = Some(end);
        reader.score.note_mut(end).tie_back = Some(note);
    }
}
