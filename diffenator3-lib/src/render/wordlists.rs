use harfrust::{script, Direction, Script};
use static_lang_word_lists::WordList;

pub(crate) fn get_wordlist(script: &str) -> Option<&WordList> {
    let wl = match script {
        "Adlam" => &static_lang_word_lists::DIFFENATOR_ADLAM,
        "Arabic" => &static_lang_word_lists::DIFFENATOR_ARABIC,
        "Armenian" => &static_lang_word_lists::DIFFENATOR_ARMENIAN,
        "Avestan" => &static_lang_word_lists::DIFFENATOR_AVESTAN,
        "Bengali" => &static_lang_word_lists::DIFFENATOR_BENGALI,
        "Bopomofo" => &static_lang_word_lists::DIFFENATOR_BOPOMOFO,
        "Canadian_Aboriginal" => &static_lang_word_lists::DIFFENATOR_CANADIAN_ABORIGINAL,
        "Chakma" => &static_lang_word_lists::DIFFENATOR_CHAKMA,
        "Cherokee" => &static_lang_word_lists::DIFFENATOR_CHEROKEE,
        "Common" => &static_lang_word_lists::DIFFENATOR_COMMON,
        "Cyrillic" => &static_lang_word_lists::DIFFENATOR_CYRILLIC,
        "Devanagari" => &static_lang_word_lists::DIFFENATOR_DEVANAGARI,
        "Ethiopic" => &static_lang_word_lists::DIFFENATOR_ETHIOPIC,
        "Georgian" => &static_lang_word_lists::DIFFENATOR_GEORGIAN,
        // "Grantha" => &static_lang_word_lists::DIFFENATOR_GRANTHA,
        "Greek" => &static_lang_word_lists::DIFFENATOR_GREEK,
        "Gujarati" => &static_lang_word_lists::DIFFENATOR_GUJARATI,
        "Gurmukhi" => &static_lang_word_lists::DIFFENATOR_GURMUKHI,
        "Hebrew" => &static_lang_word_lists::DIFFENATOR_HEBREW,
        "Hiragana" => &static_lang_word_lists::DIFFENATOR_HIRAGANA,
        "Japanese" => &static_lang_word_lists::DIFFENATOR_JAPANESE,
        // "Kannada" => &static_lang_word_lists::DIFFENATOR_KANNADA,
        "Katakana" => &static_lang_word_lists::DIFFENATOR_KATAKANA,
        "Khmer" => &static_lang_word_lists::DIFFENATOR_KHMER,
        "Lao" => &static_lang_word_lists::DIFFENATOR_LAO,
        "Latin" => &static_lang_word_lists::DIFFENATOR_LATIN,
        "Lisu" => &static_lang_word_lists::DIFFENATOR_LISU,
        "Malayalam" => &static_lang_word_lists::DIFFENATOR_MALAYALAM,
        "Mongolian" => &static_lang_word_lists::DIFFENATOR_MONGOLIAN,
        "Myanmar" => &static_lang_word_lists::DIFFENATOR_MYANMAR,
        "Ol_Chiki" => &static_lang_word_lists::DIFFENATOR_OL_CHIKI,
        "Oriya" => &static_lang_word_lists::DIFFENATOR_ORIYA,
        "Osage" => &static_lang_word_lists::DIFFENATOR_OSAGE,
        "Sinhala" => &static_lang_word_lists::DIFFENATOR_SINHALA,
        "Syriac" => &static_lang_word_lists::DIFFENATOR_SYRIAC,
        "Tamil" => &static_lang_word_lists::DIFFENATOR_TAMIL,
        "Telugu" => &static_lang_word_lists::DIFFENATOR_TELUGU,
        "Thai" => &static_lang_word_lists::DIFFENATOR_THAI,
        "Thanaa" => &static_lang_word_lists::DIFFENATOR_THANAA,
        "Tibetan" => &static_lang_word_lists::DIFFENATOR_TIBETAN,
        "Tifinagh" => &static_lang_word_lists::DIFFENATOR_TIFINAGH,
        "Vai" => &static_lang_word_lists::DIFFENATOR_VAI,

        _ => return None,
    };
    Some(wl)
}

// pub(crate) in harfrust, annoyingly.
pub fn direction_from_script(script: Script) -> Option<Direction> {
    // https://docs.google.com/spreadsheets/d/1Y90M0Ie3MUJ6UVCRDOypOtijlMDLNNyyLk36T6iMu0o

    match script {
            // Unicode-1.1 additions
            script::ARABIC |
            script::HEBREW |

            // Unicode-3.0 additions
            script::SYRIAC |
            script::THAANA |

            // Unicode-4.0 additions
            script::CYPRIOT |

            // Unicode-4.1 additions
            script::KHAROSHTHI |

            // Unicode-5.0 additions
            script::PHOENICIAN |
            script::NKO |

            // Unicode-5.1 additions
            script::LYDIAN |

            // Unicode-5.2 additions
            script::AVESTAN |
            script::IMPERIAL_ARAMAIC |
            script::INSCRIPTIONAL_PAHLAVI |
            script::INSCRIPTIONAL_PARTHIAN |
            script::OLD_SOUTH_ARABIAN |
            script::OLD_TURKIC |
            script::SAMARITAN |

            // Unicode-6.0 additions
            script::MANDAIC |

            // Unicode-6.1 additions
            script::MEROITIC_CURSIVE |
            script::MEROITIC_HIEROGLYPHS |

            // Unicode-7.0 additions
            script::MANICHAEAN |
            script::MENDE_KIKAKUI |
            script::NABATAEAN |
            script::OLD_NORTH_ARABIAN |
            script::PALMYRENE |
            script::PSALTER_PAHLAVI |

            // Unicode-8.0 additions
            script::HATRAN |

            // Unicode-9.0 additions
            script::ADLAM |

            // Unicode-11.0 additions
            script::HANIFI_ROHINGYA |
            script::OLD_SOGDIAN |
            script::SOGDIAN |

            // Unicode-12.0 additions
            script::ELYMAIC |

            // Unicode-13.0 additions
            script::CHORASMIAN |
            script::YEZIDI |

            // Unicode-14.0 additions
            script::OLD_UYGHUR |

            // Unicode-16.0 additions
            script::GARAY => {
                Some(Direction::RightToLeft)
            }

            // https://github.com/harfbuzz/harfbuzz/issues/1000
            script::OLD_HUNGARIAN |
            script::OLD_ITALIC |
            script::RUNIC |
            script::TIFINAGH => {
                None
            }

            _ => Some(Direction::LeftToRight),
        }
}
