// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon

//! Chaînes de liaisons : règle exacte de `abc2xml.py` 269,
//! `MusicXml.doNotations` (lignes 1438-1460), et ABC 2.1 §4.11.
//!
//! Une liaison `-` en attente n'est conservée que si sa hauteur est présente
//! dans l'évènement courant : `pt == ptup` pour une note seule, `pt in pts`
//! pour un accord (les notes d'accord non premières n'exécutent pas le
//! nettoyage). Sinon elle est **convertie en slur** (`tied` → `slur`) et
//! retirée des liaisons en attente. La garder telle quelle ferait lier deux
//! notes séparées par une hauteur différente : `abc2midi` compte alors les
//! deux attaques, alors qu'une note avec `<tie type="stop">` n'en a qu'une.
//!
//! Les cinq réductions ci-dessous viennent de vrais airs. Le premier test
//! (Laudate) documente aussi la limite partagée avec `abc2xml.py` : les trois
//! liaisons `d- B- F-` écrites en notes successives sont converties en slurs
//! (seule `F-` se prolonge dans l'accord suivant) ; le correctif naïf qui
//! conserve toutes les liaisons perd les quatre autres airs.

use scoretk_abc_import::read_abc;
use scoretk_core::model::SpannerType;

/// Nombre de slurs créés par la conversion des liaisons invalides.
fn slurs(src: &str) -> usize {
    let score = read_abc(src).expect("lecture");
    score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == SpannerType::Slur)
        .count()
}

/// Vrai si la note à l'index donné commence encore une liaison de prolongement.
fn tied_forward(src: &str, index: usize) -> bool {
    let score = read_abc(src).expect("lecture");
    score.notes[index].tie_forward.is_some()
}

const LAUDATE: &str = "X:1\nM:4/4\nL:1/8\nK:Eb\n\
dfdBFd-B-F- |\n[DFBd]4z2[DFB-]2 |\n";

const X988: &str = "X:988\nM:6/8\nL:1/8\nK:D\n\
B|AFF DFF|AFF A2 B|AFF DFF|GEF G2 B|\n\
|AFF DFF|AFG ABc|dB/-c/-d/-B/ AGF|GEF G2:|\n";

const X1715: &str = "X:1715\nM:C|\nL:1/8\nK:D\n\
(3A-B-c|dAFA dfaf|gfef d2 A2|BdAd GdFd|FEED E2 (3A-B-c|\n";

const X1782: &str = "X:1782\nM:C|\nL:1/8\nK:A\n\
e2 a2 a/-b/-a/-g/ a>g|e2 a>b a>ge>d|\n";

const X1799: &str = "X:1799\nM:C|\nL:1/8\nK:G\n\
D2|G3/2A/BG AGED|cBce dBAB|G3/2A/BG AGED|E/-A3/2AB A2 G-E|\n\
G3/2A/BG AGED|cBce gedB|G3/2A/BG AGED|D/-G3/2GA G2:|\n\
|:D2|GABc d2 de|(3=fgf ed e2 d2|~g-f-g.e d2 dB|GABG A3 B|\n\
c2 ec B2 dB|A2 A3/2B/ A2 G2|~g-fga g-edB|A2 AB A2 G-E|\n\
G3/2A/BG AGED|cBce dBAB|G3/2A/BG AGED|D/-G3/2GA G2:|\n";

#[test]
fn laudate_liaisons_successives_deviennent_slurs() {
    // `abc2xml.py` 269 sur cet extrait :
    //   "-- tie between different pitches: d0 converted to slur"
    //   "-- tie between different pitches: B0 converted to slur"
    // Le `F-` seul se prolonge dans `[DFBd]` ; `d-` et `B-` deviennent des
    // slurs. Les conserver tous est le changement naïf qui casse X:988 etc.
    assert_eq!(slurs(LAUDATE), 2, "d- et B- doivent devenir des slurs");
    // index 5 = le `d-` de `dfdBFd-` ; il ne doit pas traverser B et F.
    assert!(!tied_forward(LAUDATE, 5), "d- ne doit pas traverser B et F");
}

#[test]
fn x988_tie_b_ne_traverse_pas_c_et_d() {
    // `dB/-c/-d/-B/ AGF` : abc2xml convertit B, c et d en slurs. Le B initial
    // ne doit pas être lié au dernier B de la même mesure (une note de moins
    // pour le comparateur, donc un air exact perdu).
    assert!(
        !tied_forward(X988, 0),
        "B/- ne doit pas traverser c/- et d/-"
    );
    assert!(slurs(X988) >= 3, "B, c et d convertis en slurs");
}

#[test]
fn x1715_ties_du_triolet_ne_traversent_pas_c() {
    // `(3A-B-c|dAFA` : A- et B- sont convertis en slurs dès que `c` arrive.
    assert!(!tied_forward(X1715, 0), "A- ne doit pas traverser B et c");
    assert!(!tied_forward(X1715, 1), "B- ne doit pas traverser c");
    assert!(slurs(X1715) >= 2, "A et B convertis en slurs");
}

#[test]
fn x1782_ties_du_groupe_ne_traversent_pas_la_suivante() {
    // `a/-b/-a/-g/ a>g` : abc2xml convertit a, b et a en slurs ; le `a` qui
    // suit ne doit pas recevoir de liaison depuis le début du groupe.
    assert!(
        !tied_forward(X1782, 0),
        "le premier a/- ne doit pas traverser b"
    );
    assert!(slurs(X1782) >= 3, "a, b et a convertis en slurs");
}

#[test]
fn x1799_ties_de_roulement_ne_traversent_pas_la_suivante() {
    // `~g-f-g.e` : abc2xml convertit le g en slur dès que `f` arrive.
    assert_eq!(
        slurs(X1799),
        9,
        "les neuf liaisons de hauteurs différentes sont des slurs"
    );
    // Aucune liaison ne doit rester sur la première note du reduced.
    assert!(!tied_forward(X1799, 0), "g- ne doit pas traverser f");
}
