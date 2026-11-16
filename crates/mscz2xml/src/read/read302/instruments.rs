//! Changements d'instrument.
//!
//! Portage de `TRead::read(InstrumentChange*)`
//! (`engraving/rw/read400/tread.cpp`) et de l'enregistrement dans la partie
//! (`Segment::add` : `part->setInstrument(is->instrument(), tick())`).

use super::xml::Node;
use super::{annotations, part, tables, Reader, VoiceCtx};
use crate::model::{Annotation, InstrumentChange, Placement, TextFormat};

/// Lit un `<InstrumentChange>` et enregistre son instrument dans la partie.
///
/// L'instrument suit le même chemin que l'instrument principal
/// (`read_instrument`), reconnaissance comprise : les fichiers 3.x ne stockent
/// ni identifiant MusicXML ni programme pour un changement d'instrument.
pub(crate) fn read_instrument_change(
    reader: &mut Reader,
    node: &Node,
    ctx: &VoiceCtx,
) -> Annotation {
    let instrument = node
        .child("Instrument")
        .map(|instrument| part::read_instrument(reader, instrument));
    if let Some(instrument) = &instrument {
        let part_id = reader.score.staves[ctx.staff].part;
        reader
            .score
            .part_mut(part_id)
            .set_instrument_at(ctx.tick, instrument.clone());
    }

    let placement = node
        .child("placement")
        .map(|child| tables::placement(&child.trimmed_text()));
    Annotation::InstrumentChange(InstrumentChange {
        text: annotations::rich_text(node, &TextFormat::default()),
        placement: placement.unwrap_or(Placement::Above),
        visible: annotations::visible(node),
        instrument,
    })
}
