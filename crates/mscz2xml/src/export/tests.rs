//! Tests d'export (remplis à l'étape suivante).

use crate::model::Score;

#[test]
fn export_minimal() {
    let xml = super::export_musicxml(&Score::new()).expect("export");
    assert!(xml.contains("<score-partwise version=\"4.0\">"));
}
