//! Writing-system and language coverage measured from a font's character map.
//! The names and language codes match the person's font catalog (Adobe Fonts style).

/// Writing systems with characters that prove coverage.
pub const SCRIPTS: &[(&str, &str)] = &[
    ("Latin", "AZaz"),
    ("Cyrillic", "\u{0416}\u{0414}\u{0436}\u{044F}"),
    ("Greek", "\u{03A9}\u{03A3}\u{03BB}\u{03C9}"),
    ("Arabic", "\u{0628}\u{062A}\u{0645}"),
    ("Hebrew", "\u{05D0}\u{05D1}\u{05E9}"),
    ("Japanese", "\u{3042}\u{30A2}"),
    ("Korean", "\u{D55C}\u{AE00}"),
    ("Simplified Chinese", "\u{8FD9}\u{4EEC}"),
    ("Traditional Chinese", "\u{9019}\u{5011}"),
    ("Devanagari", "\u{0915}\u{0916}"),
    ("Thai", "\u{0E01}\u{0E02}"),
    ("Khmer", "\u{1780}\u{1781}"),
    ("Tamil", "\u{0B95}\u{0B99}"),
    ("Bengali", "\u{0995}\u{0996}"),
    ("Armenian", "\u{0531}\u{0561}"),
    ("Georgian", "\u{10D0}\u{10D1}"),
    ("Ethiopic", "\u{1200}\u{1208}"),
];

/// Languages: (code, name, characters that prove support). Codes match the catalog.
pub const LANGS: &[(&str, &str, &str)] = &[
    ("en", "English", "AZaz"),
    ("fr", "French", "éèêëàâçœÿ"),
    ("de", "German", "äöüßÄÖÜ"),
    ("es", "Spanish", "ñÑ¿¡áéíóú"),
    ("it", "Italian", "àèéìòù"),
    ("pt", "Portuguese", "ãõçáâêó"),
    ("nl", "Dutch", "ëïé"),
    ("ca", "Catalan", "çàèéíïòóú"),
    ("da", "Danish", "æøåÆØÅ"),
    ("no", "Norwegian", "æøåÆØÅ"),
    ("sv", "Swedish", "åäöÅÄÖ"),
    ("fi", "Finnish", "äöÄÖ"),
    ("is", "Icelandic", "ðþæÐÞÆ"),
    ("et", "Estonian", "õäöüšž"),
    ("lv", "Latvian", "āčēģīķļņšūž"),
    ("lt", "Lithuanian", "ąčęėįšųūž"),
    ("pl", "Polish", "ąćęłńśźż"),
    ("cs", "Czech", "čďěňřšťůž"),
    ("sk", "Slovak", "äčďĺľňôŕšťž"),
    ("sl", "Slovenian", "čšž"),
    ("hr", "Croatian", "čćđšž"),
    ("bs", "Bosnian", "čćđšž"),
    ("hu", "Hungarian", "őűáéö"),
    ("ro", "Romanian", "ăâî"),
    ("tr", "Turkish", "ğıİşç"),
    ("az", "Azerbaijani", "əğışÇ"),
    ("sq", "Albanian", "ëç"),
    ("eu", "Basque", "ñ"),
    ("gl", "Galician", "ñáé"),
    ("cy", "Welsh", "ŵŷâ"),
    ("ga", "Irish", "áéíóú"),
    ("mt", "Maltese", "ċġħż"),
    ("af", "Afrikaans", "êëïôû"),
    ("id", "Indonesian", "AZaz"),
    ("ms", "Malay", "AZaz"),
    ("fil", "Filipino", "ñ"),
    ("sw", "Swahili", "AZaz"),
    ("zu", "Zulu", "AZaz"),
    ("haw", "Hawaiian", "āēīōū"),
    ("mi", "Maori", "āēīōū"),
    ("vi", "Vietnamese", "ăâđêôơưạảấầẫậếệịọộớợụứự"),
    ("ru", "Russian", "абвгдежзийклмнопрстуфхцчшщъыьэюяё"),
    ("uk", "Ukrainian", "ґєії"),
    ("be", "Belarusian", "ўіё"),
    ("bg", "Bulgarian", "абвгджзийщъюя"),
    ("mk", "Macedonian", "ѓќљњџѕј"),
    ("sr", "Serbian", "ђћџљњј"),
    ("kk", "Kazakh", "әғқңөұүһі"),
    ("el", "Greek", "αβγδεζηθλμξπσφχψω"),
    ("ja", "Japanese", "\u{3042}\u{30A2}"),
    ("ko", "Korean", "\u{D55C}\u{AE00}"),
    ("zh-Hans", "Chinese (Simplified)", "\u{8FD9}\u{4EEC}"),
    ("zh-Hant", "Chinese (Traditional)", "\u{9019}\u{5011}"),
    ("ar", "Arabic", "\u{0627}\u{0628}\u{062A}"),
    ("he", "Hebrew", "\u{05D0}\u{05D1}\u{05D2}"),
    ("hi", "Hindi", "\u{0915}\u{0916}\u{0917}"),
    ("th", "Thai", "\u{0E01}\u{0E02}\u{0E04}"),
];

/// Languages written in their own (non-Latin) script.
const NON_LATIN: &[&str] = &["ru", "uk", "be", "bg", "mk", "sr", "kk", "el", "ja", "ko", "zh-Hans", "zh-Hant", "ar", "he", "hi", "th"];

pub fn script_index(name: &str) -> Option<usize> {
    SCRIPTS.iter().position(|(n, _)| *n == name)
}

pub fn lang_index(code: &str) -> Option<usize> {
    LANGS.iter().position(|(c, _, _)| *c == code)
}

fn has_all(face: &ttf_parser::Face, chars: &str) -> bool {
    chars.chars().all(|c| face.glyph_index(c).map(|g| g.0 != 0).unwrap_or(false))
}

/// (scripts bitmask, languages bitmask, caps only)
pub fn measure(face: &ttf_parser::Face) -> (u32, u64, bool) {
    let mut scripts = 0u32;
    for (i, (_, chars)) in SCRIPTS.iter().enumerate() {
        if has_all(face, chars) {
            scripts |= 1 << i;
        }
    }
    let latin = scripts & 1 != 0;
    let mut langs = 0u64;
    for (i, (code, _, chars)) in LANGS.iter().enumerate() {
        let needs_latin = !NON_LATIN.contains(code);
        if (!needs_latin || latin) && has_all(face, chars) {
            langs |= 1 << i;
        }
    }
    let lower = face.glyph_index('a');
    let upper = face.glyph_index('A');
    let caps_only = upper.is_some() && (lower.is_none() || lower == upper);
    (scripts, langs, caps_only)
}
