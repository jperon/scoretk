use std::fs;
fn main() {
    let path = std::env::args().nth(1).expect("fichier");
    let input = fs::read(&path).expect("lecture");
    let container = mscz2xml::MsczContainer::from_bytes(&input).expect("conteneur");
    let score = mscz2xml::read::read_score(&container).expect("lecture");
    let mscx = scoretk_mscx::write_mscx(&score).expect("écriture");
    let container2 = mscz2xml::MsczContainer::from_bytes(mscx.as_bytes()).expect("conteneur 2");
    let score2 = mscz2xml::read::read_score(&container2).expect("relecture");
    let options = mscz2xml::ExportOptions::default();
    let xml = mscz2xml::export::export_musicxml_with(&score2, &options).expect("export");
    print!("{xml}");
}
