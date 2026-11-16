//! Textes riches et crédits de page de titre.
//!
//! Portage de :
//!
//! - `TextBase::createBlocks` et `TextBase::fragmentList`
//!   (`engraving/dom/textbase.cpp`) : analyse du balisage interne MuseScore
//!   (`<b>`, `<i>`, `<u>`, `<s>`, `<font face=… size=…>`, `<sym>`, `<br>`
//!   implicite par `\n`) en fragments de texte formatés ;
//! - `MScoreTextToMusicXml` (`importexport/musicxml/internal/shared/musicxmlfonthandler.cpp`)
//!   pour la restitution des formats en attributs MusicXML ;
//! - `ExportMusicXml::credits` et `tidToCreditType`
//!   (`importexport/musicxml/internal/export/exportmusicxml.cpp`) pour les
//!   textes de cadre de titre (`VBox`) exportés en `<credit>`.
//!
//! Les tailles et polices par défaut ne sont pas dans le fichier `.mscx` : elles
//! viennent du style de la partition. Le modèle ne lit pas `<Style>`, il retient
//! donc uniquement les formats **explicites** (`<font face=…>`,
//! `<font size=…>`, `<b>`…) et laisse l'export comparer au style par défaut de
//! MuseScore 4.7 ([`DEFAULT_TEXT_FONT`], [`DEFAULT_TEXT_SIZE`]).

/// Police par défaut des textes de portée (`staffTextFontFace` du style
/// MuseScore 4.7).
pub const DEFAULT_TEXT_FONT: &str = "Edwin";

/// Taille par défaut des textes de portée (`staffTextFontSize` du style
/// MuseScore 4.7), en points.
pub const DEFAULT_TEXT_SIZE: f64 = 10.0;

/// Police interne des symboles musicaux (`ScoreText` dans MuseScore).
///
/// Un fragment de symbole porte cette police ; l'export la remplace par la
/// police de texte musical ([`DEFAULT_TEXT_FONT`]), comme `updateFormat` dans
/// `musicxmlfonthandler.cpp`.
pub const MUSICAL_TEXT_FONT: &str = "ScoreText";

/// Format d'un fragment de texte.
///
/// Correspond à `CharFormat` (`engraving/dom/textbase.h`), réduit aux
/// propriétés que l'export MusicXML restitue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFormat {
    /// Gras (`<b>`).
    pub bold: bool,
    /// Italique (`<i>`).
    pub italic: bool,
    /// Souligné (`<u>`).
    pub underline: bool,
    /// Barré (`<s>`).
    pub strike: bool,
    /// Police explicite (`<font face="…">`) ; `None` = police du style.
    pub font_family: Option<String>,
    /// Taille explicite (`<font size="…">`) ; `None` = taille du style.
    pub font_size: Option<f64>,
}

impl TextFormat {
    /// Vrai si le format ne porte aucun attribut explicite.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Fragment de texte formaté.
///
/// Correspond à `TextFragment` (`engraving/dom/textbase.h`) : le texte affiché
/// et son format. Un symbole musical y figure par son point de code MuseScore
/// (zone privée), comme dans le C++.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFragment {
    /// Texte du fragment, symboles compris.
    pub text: String,
    /// Format du fragment.
    pub format: TextFormat,
}

impl TextFragment {
    /// Construit un fragment.
    pub fn new(text: impl Into<String>, format: TextFormat) -> Self {
        Self {
            text: text.into(),
            format,
        }
    }
}

/// Texte riche MuseScore.
///
/// Le balisage est analysé à la construction : les fragments obtenus sont ceux
/// que `TextBase::fragmentList()` renvoie à l'export MusicXML (un saut de ligne
/// allonge le dernier fragment de la ligne d'un `\n`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichText {
    fragments: Vec<TextFragment>,
    plain: String,
}

impl RichText {
    /// Construit un texte à partir de son balisage MuseScore.
    ///
    /// Un texte sans balisage donne un unique fragment, comme dans MuseScore ;
    /// `<b>`, `<i>`, `<u>`, `<s>`, `<font>`, `<sym>` et `\n` sont interprétés.
    /// Le texte est du **balisage** : un `&` qui n'ouvre pas une entité valide
    /// est perdu, comme dans `TextBase::createBlocks`.
    pub fn new(markup: impl Into<String>) -> Self {
        Self::from_markup(&markup.into())
    }

    /// Construit un texte à partir de son balisage MuseScore.
    pub fn from_markup(markup: &str) -> Self {
        Self::from_markup_with(markup, &TextFormat::default())
    }

    /// Construit un texte à partir de son balisage, avec un format de départ.
    ///
    /// Le format de départ est celui du style du texte (`TextStyle` dans
    /// MuseScore) : le curseur de `TextBase::createBlocks` part du format du
    /// style, si bien que `<i>`/`</i>` ne font que basculer par rapport à lui.
    pub fn from_markup_with(markup: &str, base: &TextFormat) -> Self {
        let blocks = parse_blocks(markup, base);
        let mut fragments = Vec::new();
        let mut plain = String::new();
        for block in blocks {
            let last = block.fragments.len();
            for (index, mut fragment) in block.fragments.into_iter().enumerate() {
                if block.eol && index + 1 == last {
                    fragment.text.push('\n');
                }
                plain.push_str(&fragment.text);
                fragments.push(fragment);
            }
            if block.eol && last == 0 {
                plain.push('\n');
            }
        }
        Self { fragments, plain }
    }

    /// Fragments formatés, dans l'ordre du texte.
    pub fn fragments(&self) -> &[TextFragment] {
        &self.fragments
    }

    /// Texte concaténé, sauts de ligne compris.
    ///
    /// Correspond à `TextBase::plainText()`.
    pub fn plain(&self) -> &str {
        &self.plain
    }

    /// Vrai si le texte ne contient aucun caractère.
    pub fn is_empty(&self) -> bool {
        self.plain.is_empty()
    }

    /// Vrai si le texte ne contient que des espaces.
    pub fn is_blank(&self) -> bool {
        self.plain.trim().is_empty()
    }
}

/// Ligne de texte pendant l'analyse du balisage (`TextBlock`).
#[derive(Default)]
struct Block {
    fragments: Vec<TextFragment>,
    eol: bool,
}

/// Ajoute un caractère au bloc courant, en fusionnant avec le fragment voisin
/// de même format (`TextBlock::insert`).
fn insert_char(blocks: &mut [Block], format: &TextFormat, character: char) {
    let Some(block) = blocks.last_mut() else {
        return;
    };
    if block
        .fragments
        .last()
        .is_some_and(|last| last.format == *format)
    {
        block
            .fragments
            .last_mut()
            .expect("fragment présent")
            .text
            .push(character);
    } else {
        block
            .fragments
            .push(TextFragment::new(character.to_string(), format.clone()));
    }
}

/// Insère un fragment vide en tête de bloc (`TextBlock::insertEmptyFragmentIfNeeded`).
fn insert_empty_fragment(blocks: &mut [Block], format: &TextFormat) {
    let Some(block) = blocks.last_mut() else {
        return;
    };
    if block.fragments.is_empty() {
        block
            .fragments
            .insert(0, TextFragment::new(String::new(), format.clone()));
    }
}

/// Ajoute une continuation de texte à la fin du texte déjà présent.
fn append_str(blocks: &mut [Block], format: &TextFormat, text: &str) {
    for character in text.chars() {
        insert_char(blocks, format, character);
    }
}

/// Analyse le balisage MuseScore en lignes de fragments.
///
/// Reproduit la machine à états de `TextBase::createBlocks`
/// (`engraving/dom/textbase.cpp`) : texte brut, balises `<…>` et entités `&…;`,
/// avec `<sym>nom</sym>` pour les symboles musicaux. Les entités sont
/// conservées telles quelles, comme dans MuseScore (voir plus bas).
fn parse_blocks(markup: &str, base: &TextFormat) -> Vec<Block> {
    /// État de l'analyseur.
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Text,
        Tag,
        Entity,
    }

    let mut blocks = vec![Block::default()];
    let mut format = base.clone();
    let mut state = State::Text;
    let mut token = String::new();
    let mut symbol_state = false;
    let mut symbol = String::new();

    for character in markup.chars() {
        match state {
            State::Text => match character {
                '<' => {
                    state = State::Tag;
                    token.clear();
                }
                '&' => {
                    state = State::Entity;
                    token.clear();
                }
                '\n' => {
                    insert_empty_fragment(&mut blocks, &format);
                    if let Some(block) = blocks.last_mut() {
                        block.eol = true;
                    }
                    blocks.push(Block::default());
                    insert_empty_fragment(&mut blocks, &format);
                }
                _ => {
                    if symbol_state {
                        symbol.push(character);
                    } else {
                        insert_char(&mut blocks, &format, character);
                    }
                }
            },
            State::Tag => {
                if character == '>' {
                    state = State::Text;
                    if token == "sym" {
                        symbol_state = true;
                        symbol.clear();
                    } else if token == "/sym" {
                        symbol_state = false;
                        let mut symbol_format = format.clone();
                        symbol_format.font_family = Some(MUSICAL_TEXT_FONT.to_owned());
                        if let Some(character) = symbol_character(&symbol) {
                            insert_char(&mut blocks, &symbol_format, character);
                        }
                        // Le C++ restaure le format précédent après le symbole.
                    } else {
                        apply_tag(&mut format, &token);
                    }
                } else {
                    token.push(character);
                }
            }
            State::Entity => {
                if character == ';' {
                    state = State::Text;
                    // MuseScore conserve les entités nommées telles quelles :
                    // `readXml` ré-échappe le texte lu (`text().toXmlEscaped()`),
                    // si bien que le texte interne contient `&quot;` et que
                    // l'export l'écrit brut. Les références numériques, elles,
                    // sont résolues par l'analyseur XML puis ré-écrites en
                    // clair (`&#233;` devient `é`).
                    let decoded = decode_character_reference(&token)
                        .map(|character| character.to_string())
                        .unwrap_or_else(|| format!("&{token};"));
                    if symbol_state {
                        symbol.push_str(&decoded);
                    } else {
                        append_str(&mut blocks, &format, &decoded);
                    }
                } else {
                    token.push(character);
                }
            }
        }
    }

    blocks
}

/// Applique une balise de format (`TextBase::prepareFormat`).
fn apply_tag(format: &mut TextFormat, token: &str) {
    match token {
        "b" => format.bold = true,
        "/b" => format.bold = false,
        "i" => format.italic = true,
        "/i" => format.italic = false,
        "u" => format.underline = true,
        "/u" => format.underline = false,
        "s" => format.strike = true,
        "/s" => format.strike = false,
        // Les indices et exposants ne changent pas la sortie MusicXML.
        "sub" | "/sub" | "sup" | "/sup" => {}
        _ => {
            let Some(remainder) = token.strip_prefix("font ") else {
                return;
            };
            if let Some(value) = property_value(remainder, "size") {
                if let Ok(size) = value.parse::<f64>() {
                    format.font_size = Some(size);
                }
            } else if let Some(value) = property_value(remainder, "face") {
                format.font_family = Some(value.to_owned());
            }
        }
    }
}

/// Valeur d'une propriété `nom="valeur"` de balise `<font …>`.
///
/// Comme `parseNumProperty`/`parseStringProperty` dans
/// `engraving/dom/textbase.cpp`, la valeur s'arrête au guillemet fermant : une
/// balise `<font face="X" size="12">` n'applique que la première propriété.
fn property_value<'a>(remainder: &'a str, name: &str) -> Option<&'a str> {
    let rest = remainder.strip_prefix(name)?;
    let rest = rest.strip_prefix("=\"")?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Résout une référence de caractère numérique (`#233`, `#xE9`).
fn decode_character_reference(token: &str) -> Option<char> {
    let value = token.strip_prefix('#')?;
    let code = match value.strip_prefix('x').or_else(|| value.strip_prefix('X')) {
        Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
        None => value.parse().ok()?,
    };
    char::from_u32(code)
}

/// Point de code MuseScore d'un symbole musical connu.
///
/// Les valeurs viennent des motifs de `TempoText::findTempoDuration`
/// (`engraving/dom/tempotext.cpp`, table `tp`) : ce sont les points de code
/// SMuFL utilisés par MuseScore pour les figures de métronome. Les symboles
/// hors de cette table sont ignorés (le modèle n'a pas la table SMuFL
/// complète).
pub fn symbol_character(name: &str) -> Option<char> {
    let character = match name {
        "space" => ' ',
        "metAugmentationDot" => '\u{ECB7}',
        "metNoteDoubleWhole" => '\u{ECA0}',
        "metNoteDoubleWholeSquare" => '\u{ECA1}',
        "metNoteWhole" => '\u{ECA2}',
        "metNoteHalfUp" => '\u{ECA3}',
        "metNoteQuarterUp" => '\u{ECA5}',
        "metNote8thUp" => '\u{ECA7}',
        "metNote16thUp" => '\u{ECA9}',
        "metNote32ndUp" => '\u{ECAB}',
        "metNote64thUp" => '\u{ECAD}',
        "metNote128thUp" => '\u{ECAF}',
        "metNote256thUp" => '\u{ECB1}',
        "metNote512thUp" => '\u{ECB3}',
        "metNote1024thUp" => '\u{ECB5}',
        _ => return None,
    };
    Some(character)
}

/// Texte d'un cadre `TBox`, exporté en `<words>` de direction.
///
/// Correspond à `TBox::text()` (`engraving/dom/tbox.h`) : MuseScore l'écrit au
/// début de la première mesure du système qui porte le cadre
/// (`ExportMusicXml::tboxTextAsWords`). Le modèle n'a qu'une page et pas de
/// systèmes : le texte est rattaché à l'indice de la mesure qui suit le cadre.
#[derive(Clone, Debug, PartialEq)]
pub struct TboxText {
    /// Texte du cadre.
    pub text: RichText,
    /// Indice de la mesure devant laquelle le texte est écrit.
    pub measure: usize,
    /// Placement (`above` si le cadre est au-dessus de la mesure).
    pub placement: super::annotation::Placement,
}

impl TboxText {
    /// Construit le texte d'un cadre rattaché à une mesure.
    pub fn new(text: RichText, measure: usize) -> Self {
        Self {
            text,
            measure,
            placement: super::annotation::Placement::Above,
        }
    }
}

/// Style d'un texte de cadre de titre.
///
/// Correspond à `TextStyleType` (`engraving/dom/textstyle.h`) réduit aux styles
/// que l'export MusicXML reconnaît (`tidToCreditType`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreditStyle {
    /// Titre de l'œuvre (`title`).
    Title,
    /// Sous-titre (`subtitle`).
    Subtitle,
    /// Compositeur (`composer`).
    Composer,
    /// Parolier (`lyricist`).
    Lyricist,
    /// Sous-titre de partie ou autre style du fichier.
    Other(String),
}

impl CreditStyle {
    /// Style correspondant au nom écrit dans `<style>` du fichier.
    pub fn from_name(name: &str) -> Self {
        match name {
            "title" | "Title" => Self::Title,
            "subtitle" | "Subtitle" => Self::Subtitle,
            "composer" | "Composer" => Self::Composer,
            "lyricist" | "Lyricist" | "poet" => Self::Lyricist,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Type MusicXML (`<credit-type>`), vide si le style n'en a pas.
    pub fn credit_type(&self) -> Option<&'static str> {
        match self {
            Self::Title => Some("title"),
            Self::Subtitle => Some("subtitle"),
            Self::Composer => Some("composer"),
            Self::Lyricist => Some("lyricist"),
            Self::Other(_) => None,
        }
    }

    /// Taille de police par défaut du style (MuseScore 4.7).
    pub fn default_font_size(&self) -> f64 {
        match self {
            Self::Title => 22.0,
            Self::Subtitle => 14.0,
            Self::Composer | Self::Lyricist | Self::Other(_) => DEFAULT_TEXT_SIZE,
        }
    }

    /// Alignement horizontal par défaut du style.
    pub fn default_justify(&self) -> CreditJustify {
        match self {
            Self::Composer | Self::Lyricist => CreditJustify::Right,
            Self::Title | Self::Subtitle | Self::Other(_) => CreditJustify::Center,
        }
    }

    /// Alignement vertical par défaut du style.
    pub fn default_valign(&self) -> CreditValign {
        match self {
            Self::Composer | Self::Lyricist => CreditValign::Bottom,
            Self::Title | Self::Subtitle | Self::Other(_) => CreditValign::Top,
        }
    }
}

/// Alignement horizontal d'un texte (`<credit-words justify=…>`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreditJustify {
    /// À gauche.
    Left,
    /// Centré.
    #[default]
    Center,
    /// À droite.
    Right,
}

impl CreditJustify {
    /// Nom MusicXML.
    pub const fn musicxml_name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    /// Alignement correspondant au nom MuseScore (`left`, `hcenter`…).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Self::Left),
            "hcenter" | "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// Alignement vertical d'un texte (`<credit-words valign=…>`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreditValign {
    /// En haut.
    #[default]
    Top,
    /// Au milieu.
    Middle,
    /// En bas.
    Bottom,
    /// Sur la ligne de base.
    Baseline,
}

impl CreditValign {
    /// Nom MusicXML.
    pub const fn musicxml_name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
            Self::Baseline => "baseline",
        }
    }

    /// Alignement correspondant au nom MuseScore (`top`, `vcenter`…).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "top" => Some(Self::Top),
            "vcenter" | "middle" => Some(Self::Middle),
            "bottom" => Some(Self::Bottom),
            "baseline" => Some(Self::Baseline),
            _ => None,
        }
    }
}

/// Texte de cadre de titre exporté en `<credit>`.
///
/// Correspond à un `Text` enfant d'un `VBox` (`engraving/dom/vbox.h`) ; MuseScore
/// l'exporte par `ExportMusicXml::credits`. Les positions (`default-x`,
/// `default-y`) relèvent de la mise en page et ne sont pas modélisées.
#[derive(Clone, Debug, PartialEq)]
pub struct Credit {
    /// Texte.
    pub text: RichText,
    /// Style du texte.
    pub style: CreditStyle,
    /// Alignement horizontal.
    pub justify: CreditJustify,
    /// Alignement vertical.
    pub valign: CreditValign,
    /// Page (1-based).
    pub page: u32,
}

impl Credit {
    /// Construit un crédit aligné selon les valeurs par défaut de son style.
    pub fn new(text: RichText, style: CreditStyle) -> Self {
        let justify = style.default_justify();
        let valign = style.default_valign();
        Self {
            text,
            style,
            justify,
            valign,
            page: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_sans_balisage() {
        let text = RichText::new("Visible text");
        assert_eq!(text.fragments().len(), 1);
        assert_eq!(text.plain(), "Visible text");
        assert_eq!(text.fragments()[0].format, TextFormat::default());
    }

    #[test]
    fn texte_vide() {
        let text = RichText::new("");
        assert!(text.is_empty());
        assert!(text.fragments().is_empty());
        let markup = RichText::new("<b></b><font face=\"Edwin\"></font>");
        assert!(markup.is_empty());
        assert!(markup.fragments().is_empty());
    }

    #[test]
    fn gras_italique_et_police() {
        let text = RichText::new("a<b>b</b><i>c</i><font face=\"FreeSerif\">d</font>");
        assert_eq!(text.plain(), "abcd");
        let formats: Vec<_> = text.fragments().iter().map(|f| f.format.clone()).collect();
        assert_eq!(formats.len(), 4);
        assert!(!formats[0].bold);
        assert!(formats[1].bold);
        assert!(formats[2].italic);
        assert_eq!(formats[3].font_family.as_deref(), Some("FreeSerif"));
    }

    #[test]
    fn taille_de_police_explicite() {
        let text = RichText::new("<font size=\"16\">gros</font>");
        assert_eq!(text.fragments()[0].format.font_size, Some(16.0));
    }

    #[test]
    fn symbole_de_metronome() {
        let text = RichText::new("<sym>metNoteQuarterUp</sym> = 80");
        assert_eq!(text.plain(), "\u{ECA5} = 80");
        assert_eq!(text.fragments().len(), 2);
        assert_eq!(
            text.fragments()[0].format.font_family.as_deref(),
            Some(MUSICAL_TEXT_FONT)
        );
        assert_eq!(text.fragments()[1].text, " = 80");
    }

    #[test]
    fn saut_de_ligne_et_fragment_vide() {
        let text = RichText::new("ligne\nautre");
        assert_eq!(text.plain(), "ligne\nautre");
        assert_eq!(text.fragments().len(), 2);
        assert_eq!(text.fragments()[0].text, "ligne\n");

        // Un saut de ligne final laisse une ligne vide, comme MuseScore.
        let trailing = RichText::new("fin\n");
        assert_eq!(trailing.fragments().len(), 2);
        assert_eq!(trailing.fragments()[0].text, "fin\n");
        assert!(trailing.fragments()[1].text.is_empty());
    }

    #[test]
    fn entites() {
        // MuseScore conserve les entités telles quelles (`readXml` ré-échappe
        // le texte), l'export les écrit donc brutes.
        let text = RichText::new("a &amp; b &lt; c");
        assert_eq!(text.plain(), "a &amp; b &lt; c");
    }

    #[test]
    fn format_de_credit_par_style() {
        let credit = Credit::new(RichText::new("Titre"), CreditStyle::Title);
        assert_eq!(credit.style.credit_type(), Some("title"));
        assert_eq!(credit.justify, CreditJustify::Center);
        assert_eq!(credit.valign, CreditValign::Top);
        assert_eq!(credit.style.default_font_size(), 22.0);

        let composer = Credit::new(RichText::new("Composer"), CreditStyle::Composer);
        assert_eq!(composer.justify, CreditJustify::Right);
        assert_eq!(composer.valign, CreditValign::Bottom);
        assert_eq!(composer.style.default_font_size(), DEFAULT_TEXT_SIZE);
    }
}
