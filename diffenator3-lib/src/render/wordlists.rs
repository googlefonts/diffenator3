use harfrust::{script, Direction, Script};
use lazy_static::lazy_static;
use std::io::{BufRead, BufReader};

macro_rules! include_script {
    ($var:ident, $path:literal ) => {
        lazy_static! {
            pub static ref $var: Vec<u8> = {
                let mut input: Vec<u8> = Vec::new();
                let compressed = include_bytes!($path);
                brotli::BrotliDecompress(&mut compressed.as_ref(), &mut input)
                    .expect("Could not decompress");
                input
            };
        }
    };
}

include_script!(ADLAM, "../../wordlists/Adlam.txt.br");
include_script!(ARABIC, "../../wordlists/Arabic.txt.br");
include_script!(ARMENIAN, "../../wordlists/Armenian.txt.br");
include_script!(AVESTAN, "../../wordlists/Avestan.txt.br");
include_script!(BENGALI, "../../wordlists/Bengali.txt.br");
include_script!(BOPOMOFO, "../../wordlists/Bopomofo.txt.br");
include_script!(
    CANADIAN_ABORIGINAL,
    "../../wordlists/Canadian_Aboriginal.txt.br"
);
include_script!(CHAKMA, "../../wordlists/Chakma.txt.br");
include_script!(CHEROKEE, "../../wordlists/Cherokee.txt.br");
include_script!(COMMON, "../../wordlists/Common.txt.br");
include_script!(CYRILLIC, "../../wordlists/Cyrillic.txt.br");
include_script!(DEVANAGARI, "../../wordlists/Devanagari.txt.br");
include_script!(ETHIOPIC, "../../wordlists/Ethiopic.txt.br");
include_script!(GEORGIAN, "../../wordlists/Georgian.txt.br");
include_script!(GRANTHA, "../../wordlists/Grantha.txt.br");
include_script!(GREEK, "../../wordlists/Greek.txt.br");
include_script!(GUJARATI, "../../wordlists/Gujarati.txt.br");
include_script!(GURMUKHI, "../../wordlists/Gurmukhi.txt.br");
include_script!(HEBREW, "../../wordlists/Hebrew.txt.br");
include_script!(HIRAGANA, "../../wordlists/Hiragana.txt.br");
include_script!(JAPANESE, "../../wordlists/Japanese.txt.br");
include_script!(KANNADA, "../../wordlists/Kannada.txt.br");
include_script!(KATAKANA, "../../wordlists/Katakana.txt.br");
include_script!(KHMER, "../../wordlists/Khmer.txt.br");
include_script!(LAO, "../../wordlists/Lao.txt.br");
include_script!(LATIN, "../../wordlists/Latin.txt.br");
include_script!(LISU, "../../wordlists/Lisu.txt.br");
include_script!(MALAYALAM, "../../wordlists/Malayalam.txt.br");
include_script!(MONGOLIAN, "../../wordlists/Mongolian.txt.br");
include_script!(MYANMAR, "../../wordlists/Myanmar.txt.br");
include_script!(OL_CHIKI, "../../wordlists/Ol_Chiki.txt.br");
include_script!(ORIYA, "../../wordlists/Oriya.txt.br");
include_script!(OSAGE, "../../wordlists/Osage.txt.br");
include_script!(SINHALA, "../../wordlists/Sinhala.txt.br");
include_script!(SYRIAC, "../../wordlists/Syriac.txt.br");
include_script!(TAMIL, "../../wordlists/Tamil.txt.br");
include_script!(TELUGU, "../../wordlists/Telugu.txt.br");
include_script!(THAI, "../../wordlists/Thai.txt.br");
include_script!(THANAA, "../../wordlists/Thanaa.txt.br");
include_script!(TIBETAN, "../../wordlists/Tibetan.txt.br");
include_script!(TIFINAGH, "../../wordlists/Tifinagh.txt.br");
include_script!(VAI, "../../wordlists/Vai.txt.br");

pub(crate) fn get_wordlist(script: &str) -> Option<Vec<String>> {
    let compressed = match script {
        "Adlam" => ADLAM.as_slice(),
        "Arabic" => ARABIC.as_slice(),
        "Armenian" => ARMENIAN.as_slice(),
        "Avestan" => AVESTAN.as_slice(),
        "Bengali" => BENGALI.as_slice(),
        "Bopomofo" => BOPOMOFO.as_slice(),
        "Canadian_Aboriginal" => CANADIAN_ABORIGINAL.as_slice(),
        "Chakma" => CHAKMA.as_slice(),
        "Cherokee" => CHEROKEE.as_slice(),
        "Common" => COMMON.as_slice(),
        "Cyrillic" => CYRILLIC.as_slice(),
        "Devanagari" => DEVANAGARI.as_slice(),
        "Ethiopic" => ETHIOPIC.as_slice(),
        "Georgian" => GEORGIAN.as_slice(),
        "Grantha" => GRANTHA.as_slice(),
        "Greek" => GREEK.as_slice(),
        "Gujarati" => GUJARATI.as_slice(),
        "Gurmukhi" => GURMUKHI.as_slice(),
        "Hebrew" => HEBREW.as_slice(),
        "Hiragana" => HIRAGANA.as_slice(),
        "Japanese" => JAPANESE.as_slice(),
        "Kannada" => KANNADA.as_slice(),
        "Katakana" => KATAKANA.as_slice(),
        "Khmer" => KHMER.as_slice(),
        "Lao" => LAO.as_slice(),
        "Latin" => LATIN.as_slice(),
        "Lisu" => LISU.as_slice(),
        "Malayalam" => MALAYALAM.as_slice(),
        "Mongolian" => MONGOLIAN.as_slice(),
        "Myanmar" => MYANMAR.as_slice(),
        "Ol_Chiki" => OL_CHIKI.as_slice(),
        "Oriya" => ORIYA.as_slice(),
        "Osage" => OSAGE.as_slice(),
        "Sinhala" => SINHALA.as_slice(),
        "Syriac" => SYRIAC.as_slice(),
        "Tamil" => TAMIL.as_slice(),
        "Telugu" => TELUGU.as_slice(),
        "Thai" => THAI.as_slice(),
        "Thanaa" => THANAA.as_slice(),
        "Tibetan" => TIBETAN.as_slice(),
        "Tifinagh" => TIFINAGH.as_slice(),
        "Vai" => VAI.as_slice(),

        _ => return None,
    };
    let buf = BufReader::new(compressed);
    let wordlist: Vec<String> = buf
        .lines()
        .map(|l| l.expect("Could not parse line"))
        .collect();
    Some(wordlist)
}

pub fn get_script_tag(script: &str) -> Option<Script> {
    match script {
        "Adlam" => Some(script::ADLAM),
        "Arabic" => Some(script::ARABIC),
        "Armenian" => Some(script::ARMENIAN),
        "Avestan" => Some(script::AVESTAN),
        "Bengali" => Some(script::BENGALI),
        "Bopomofo" => Some(script::BOPOMOFO),
        "Canadian_Aboriginal" => Some(script::CANADIAN_SYLLABICS),
        "Chakma" => Some(script::CHAKMA),
        "Cherokee" => Some(script::CHEROKEE),
        "Common" => Some(script::COMMON),
        "Cyrillic" => Some(script::CYRILLIC),
        "Devanagari" => Some(script::DEVANAGARI),
        "Ethiopic" => Some(script::ETHIOPIC),
        "Georgian" => Some(script::GEORGIAN),
        "Grantha" => Some(script::GRANTHA),
        "Greek" => Some(script::GREEK),
        "Gujarati" => Some(script::GUJARATI),
        "Gurmukhi" => Some(script::GURMUKHI),
        "Hebrew" => Some(script::HEBREW),
        "Hiragana" => Some(script::HIRAGANA),
        "Kannada" => Some(script::KANNADA),
        "Katakana" => Some(script::KATAKANA),
        "Khmer" => Some(script::KHMER),
        "Lao" => Some(script::LAO),
        "Latin" => Some(script::LATIN),
        "Lisu" => Some(script::LISU),
        "Malayalam" => Some(script::MALAYALAM),
        "Mongolian" => Some(script::MONGOLIAN),
        "Myanmar" => Some(script::MYANMAR),
        "Ol_Chiki" => Some(script::OL_CHIKI),
        "Oriya" => Some(script::ORIYA),
        "Osage" => Some(script::OSAGE),
        "Sinhala" => Some(script::SINHALA),
        "Syriac" => Some(script::SYRIAC),
        "Tamil" => Some(script::TAMIL),
        "Telugu" => Some(script::TELUGU),
        "Thai" => Some(script::THAI),
        "Tibetan" => Some(script::TIBETAN),
        "Tifinagh" => Some(script::TIFINAGH),
        "Vai" => Some(script::VAI),
        _ => None,
    }
}

pub fn get_script_direction(script: &str) -> Direction {
    match script {
        "Arabic" => Direction::RightToLeft,
        "Avestan" => Direction::RightToLeft,
        "Hebrew" => Direction::RightToLeft,
        "Syriac" => Direction::RightToLeft,
        "Thanaa" => Direction::RightToLeft,
        _ => Direction::LeftToRight,
    }
}
