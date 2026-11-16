//! Tests d'export des nuances et soufflets sur des partitions construites à la
//! main.

use mscz2xml::export::export_musicxml;
use mscz2xml::model::{
    tpc, Annotation, ChordRest, Duration, Dynamic, Fraction, Hairpin, HairpinType, Measure, Note,
    Part, Placement, Score, Segment, SegmentItem, SegmentType, Spanner, SpannerAnchor, SpannerType,
    StaffId, Tick, Track,
};

fn piano(score: &mut Score) -> (StaffId, StaffId) {
    let part = score.add_part(Part::new("Piano", "Piano"));
    let upper = score.add_staff(part);
    (upper, upper)
}

fn ajouter_mesure(score: &mut Score, start: Fraction, len: Fraction) -> usize {
    let mut measure = Measure::new(score.measures.len(), start, len);
    measure.len = len;
    score.add_measure(measure).index()
}

fn ajouter_accord(score: &mut Score, measure_index: usize, track: Track, tick: Tick) {
    let note = score.add_note(Note::new(60, tpc::C));
    let mut chord_rest = ChordRest::chord(Duration::whole());
    chord_rest.chord_mut().expect("accord").notes.push(note);
    let mut segment = Segment::new(SegmentType::ChordRest, tick, score.staves.len());
    segment.set_element(track, SegmentItem::ChordRest(chord_rest));
    score.measures[measure_index].add_segment(segment);
}

fn score_simple() -> (Score, Track, usize) {
    let mut score = Score::new();
    let (staff, _) = piano(&mut score);
    let track = Track::new(staff, 0);
    let measure = ajouter_mesure(&mut score, Fraction::ZERO, Fraction::ONE);
    ajouter_accord(&mut score, measure, track, Fraction::ZERO);
    (score, track, measure)
}

fn ajouter_nuance(score: &mut Score, measure: usize, track: Track, dynamic: Dynamic) {
    let segment = score.measures[measure]
        .segments
        .last_mut()
        .expect("segment d'accord");
    segment.add_annotation(track, Annotation::Dynamic(dynamic));
}

#[test]
fn nuance_avec_velocity_et_placement() {
    let (mut score, track, measure) = score_simple();
    let mut dynamic = Dynamic::new("p");
    dynamic.velocity = Some(49);
    ajouter_nuance(&mut score, measure, track, dynamic);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<direction placement=\"below\">"));
    assert!(xml.contains("<p/>"));
    assert!(xml.contains("<sound dynamics=\"54.44\"/>"));
}

#[test]
fn nuance_personnalisee_et_niente_4_7() {
    let (mut score, track, measure) = score_simple();
    let mut above = Dynamic::new("dolce");
    above.custom = true;
    above.placement = Placement::Above;
    above.velocity = Some(80);
    ajouter_nuance(&mut score, measure, track, above);

    let mut niente = Dynamic::new("n");
    niente.velocity = Some(49);
    ajouter_nuance(&mut score, measure, track, niente);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<direction placement=\"above\">"));
    assert!(xml.contains("<other-dynamics>dolce</other-dynamics>"));
    // MuseScore 4.7.4 ne reconnaît pas « n » comme nuance MusicXML.
    assert!(xml.contains("<other-dynamics>n</other-dynamics>"));
    assert!(!xml.contains("<n/>"));
}

#[test]
fn nuance_vide_et_invisible() {
    let (mut score, track, measure) = score_simple();
    let mut vide = Dynamic::new("");
    vide.velocity = Some(33);
    ajouter_nuance(&mut score, measure, track, vide);

    let mut invisible = Dynamic::new("fff");
    invisible.visible = false;
    invisible.velocity = Some(126);
    ajouter_nuance(&mut score, measure, track, invisible);

    let xml = export_musicxml(&score).expect("export");
    // Une nuance vide produit un <dynamics> vide, sans <other-dynamics>.
    assert!(xml.contains("<dynamics>\n") && !xml.contains("<other-dynamics>"));
    assert!(!xml.contains("<fff/>"));
    assert!(!xml.contains("140.00"));
}

fn soufflet(
    score: &mut Score,
    track: Track,
    debut: Tick,
    fin: Tick,
    hairpin: Hairpin,
    kind: HairpinType,
) {
    let mut spanner = Spanner::new(
        SpannerType::Hairpin(kind),
        SpannerAnchor::new(debut, track),
        SpannerAnchor::new(fin, track),
    );
    spanner.hairpin = Some(hairpin);
    score.add_spanner(spanner);
}

#[test]
fn soufflet_wedge_niente_et_numeros() {
    let (mut score, track, _) = score_simple();
    let mut data = Hairpin::new();
    data.niente = true;
    soufflet(
        &mut score,
        track,
        Fraction::ZERO,
        Fraction::new(1, 2),
        data.clone(),
        HairpinType::Diminuendo,
    );
    soufflet(
        &mut score,
        track,
        Fraction::ZERO,
        Fraction::new(1, 2),
        data,
        HairpinType::Crescendo,
    );

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<wedge type=\"diminuendo\" number=\"1\"/>"));
    assert!(xml.contains("<wedge type=\"crescendo\" niente=\"yes\" spread=\"11.5\" number=\"2\"/>"));
    assert!(!xml.contains("<wedge type=\"stop\" niente=\"yes\" number=\"1\"/>"));
}

#[test]
fn soufflet_en_texte_avec_dynamiques() {
    let (mut score, track, _) = score_simple();
    let mut data = Hairpin::new();
    data.line = true;
    data.begin_text =
        "cresc. from <sym>dynamicMezzo</sym><sym>dynamicForte</sym> to <sym>dynamicForte</sym><sym>dynamicForte</sym><sym>dynamicForte</sym>"
            .to_owned();
    soufflet(
        &mut score,
        track,
        Fraction::ZERO,
        Fraction::ONE,
        data,
        HairpinType::Crescendo,
    );

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains(
        "<words font-family=\"FreeSerif\" font-size=\"12\" font-style=\"italic\">cresc. from </words>"
    ));
    assert!(xml.contains("<mf/>"));
    assert!(xml.contains(
        "<words font-family=\"FreeSerif\" font-size=\"12\" font-style=\"italic\"> to </words>"
    ));
    assert!(xml.contains("<fff/>"));
    assert!(xml.contains("<dashes type=\"start\" number=\"1\"/>"));
    assert!(xml.contains("<dashes type=\"stop\" number=\"1\"/>"));
}

#[test]
fn nuance_rattachee_a_la_deuxieme_portee() {
    let mut score = Score::new();
    let part = score.add_part(Part::new("Piano", "Piano"));
    let _upper = score.add_staff(part);
    let lower = score.add_staff(part);
    let measure = ajouter_mesure(&mut score, Fraction::ZERO, Fraction::ONE);
    let track = Track::new(lower, 0);
    ajouter_accord(&mut score, measure, track, Fraction::ZERO);
    let mut dynamic = Dynamic::new("mf");
    dynamic.velocity = Some(80);
    ajouter_nuance(&mut score, measure, track, dynamic);

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<staff>2</staff>"));
    assert!(xml.contains("<mf/>"));
}

#[test]
fn soufflet_place_au_dessus() {
    let (mut score, track, _) = score_simple();
    let mut data = Hairpin::new();
    data.placement = Placement::Above;
    soufflet(
        &mut score,
        track,
        Fraction::ZERO,
        Fraction::ONE,
        data,
        HairpinType::Crescendo,
    );

    let xml = export_musicxml(&score).expect("export");
    assert!(xml.contains("<direction placement=\"above\">"));
    assert!(xml.contains("<wedge type=\"crescendo\" spread=\"11.5\" number=\"1\"/>"));
}
