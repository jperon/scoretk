//! Bindings WebAssembly de `mscz2xml`, générés par wasm-bindgen.
//!
//! `nix build .#wasm` produit deux variantes à partir de ce crate :
//!
//! - `esm/` : module ES (`wasm-bindgen --target web`), initialisé par
//!   `await init()` ;
//! - `classic/` : script classique (`wasm-bindgen --target no-modules`)
//!   chargeable par `importScripts()` dans un Web Worker, accompagné du
//!   wrapper `mscz2xml.js` qui expose `self.mscz2xml`.
//!
//! L'API exposée est volontairement minimale : [`convert`] et [`version`].
//! Aucune entrée/sortie fichier, aucune horloge : tout se passe en mémoire, et
//! la date d'encodage vient des options.

use wasm_bindgen::prelude::*;

use mscz2xml::{convert as convert_impl, ConvertOptions as CoreConvertOptions, OutputFormat};

/// Options de [`convert`], passées comme objet littéral JavaScript.
///
/// La forme TypeScript est déclarée dans [`TS_OPTIONS`] ; côté Rust ce type
/// n'est qu'un `JsValue` dont on lit la propriété `encodingDate`.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ConvertOptions")]
    pub type ConvertOptions;
}

#[wasm_bindgen(typescript_custom_section)]
const TS_OPTIONS: &'static str = r#"
/** Options de `convert`. Toutes sont facultatives. */
export interface ConvertOptions {
  /**
   * Contenu de `<encoding-date>` (AAAA-MM-JJ). Par défaut, la constante de
   * la bibliothèque, pour des sorties reproductibles.
   */
  encodingDate?: string;
}
"#;

/// Convertit un `.mscz` ou un `.mscx` en MusicXML `partwise` (texte UTF-8).
///
/// `input` est le contenu binaire du fichier. Une entrée illisible lève une
/// `Error` JavaScript dont le message est celui de l'erreur Rust d'origine.
#[wasm_bindgen]
pub fn convert(input: &[u8], options: Option<ConvertOptions>) -> Result<String, JsError> {
    let mut convert_options = CoreConvertOptions {
        format: OutputFormat::MusicXml,
        ..CoreConvertOptions::default()
    };

    if let Some(encoding_date) = options.as_ref().and_then(encoding_date) {
        convert_options.export.encoding_date = encoding_date;
    }

    let bytes =
        convert_impl(input, &convert_options).map_err(|error| JsError::new(&error.to_string()))?;

    // L'export MusicXML est toujours de l'UTF-8 ; ce garde-fou évite une
    // chaîne JS silencieusement mal décodée si ce n'était pas le cas.
    String::from_utf8(bytes)
        .map_err(|error| JsError::new(&format!("sortie MusicXML non UTF-8 : {error}")))
}

/// Version de la bibliothèque, telle que déclarée dans `Cargo.toml`.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Lit `encodingDate` dans l'objet d'options, en ignorant une valeur absente,
/// nulle ou vide.
fn encoding_date(options: &ConvertOptions) -> Option<String> {
    let value: &JsValue = options.as_ref();
    let date = js_sys::Reflect::get(value, &JsValue::from_str("encodingDate")).ok()?;
    let date = date.as_string()?;
    if date.is_empty() {
        None
    } else {
        Some(date)
    }
}
