//! Interface languages: English (default), Ukrainian and Russian.
//!
//! English is the source language of the code. Every text the user can read is written
//! in English and looked up here: `tr("Chat")` returns the Ukrainian or Russian version
//! when that language is selected, and the English text itself otherwise (so a missing
//! translation can never break the app). Texts with variable parts use named
//! placeholders: `trf("{name} left", &[("name", &name)])`.
//!
//! Tests scan the source files and fail when a text is missing from the table, when a
//! translation has different placeholders, or when Ukrainian and Russian are mixed up.
use std::{
    collections::HashMap,
    fmt::Display,
    sync::{
        atomic::{AtomicU8, Ordering},
        OnceLock,
    },
};

#[path = "i18n_table.rs"]
mod table;
use table::TABLE;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Uk,
    Ru,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Uk, Lang::Ru];

    /// Code stored in the settings file.
    pub fn code(self) -> &'static str {
        match self { Lang::En => "en", Lang::Uk => "uk", Lang::Ru => "ru" }
    }
    /// Unknown or empty codes mean English, the default language.
    pub fn from_code(code: &str) -> Lang {
        match code.trim().to_ascii_lowercase().as_str() {
            "uk" | "ua" => Lang::Uk,
            "ru" => Lang::Ru,
            _ => Lang::En,
        }
    }
    /// Each language is always shown in its own language, so it can be found even when
    /// the rest of the interface is not understood.
    pub fn native_name(self) -> &'static str {
        match self { Lang::En => "English", Lang::Uk => "Українська", Lang::Ru => "Русский" }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}
pub fn language() -> Lang {
    match CURRENT.load(Ordering::Relaxed) { 1 => Lang::Uk, 2 => Lang::Ru, _ => Lang::En }
}

type Translations = (&'static str, &'static str);

fn index() -> &'static HashMap<&'static str, Translations> {
    static INDEX: OnceLock<HashMap<&'static str, Translations>> = OnceLock::new();
    INDEX.get_or_init(|| TABLE.iter().map(|(en, uk, ru)| (*en, (*uk, *ru))).collect())
}

/// Translates an English text into the current language.
pub fn tr(english: &'static str) -> &'static str {
    translate(english, language())
}

fn translate(english: &'static str, lang: Lang) -> &'static str {
    match (lang, index().get(english)) {
        (Lang::Uk, Some((uk, _))) => uk,
        (Lang::Ru, Some((_, ru))) => ru,
        _ => english,
    }
}

/// Marks an English text that is stored as it is and translated later with [`tr_stored`].
pub fn english(text: &'static str) -> &'static str {
    text
}

/// Translates a text that is only known at run time (stored chat notices). Anything that is
/// not an English key from the table is returned unchanged.
pub fn tr_stored(text: &str) -> String {
    match (language(), index().get(text)) {
        (Lang::Uk, Some((uk, _))) => (*uk).to_string(),
        (Lang::Ru, Some((_, ru))) => (*ru).to_string(),
        _ => text.to_string(),
    }
}

/// Translates a text with `{name}` placeholders and fills them in.
pub fn trf(english: &'static str, args: &[(&str, &dyn Display)]) -> String {
    fill(tr(english), args)
}

fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = template.to_string();
    for (name, value) in args {
        let mut value = value.to_string();
        // System errors often end with a full stop already: avoid "…unplugged.."
        if template.contains(&format!("{{{name}}}.")) {
            value = value.trim_end_matches(['.', ' ']).to_string();
        }
        text = text.replace(&format!("{{{name}}}"), &value);
    }
    text
}

/// A number with the decimal separator of the current language (1.5 or 1,5).
pub fn decimal(value: f32, digits: usize) -> String {
    decimal_in(language(), value, digits)
}

fn decimal_in(lang: Lang, value: f32, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    match lang { Lang::En => text, _ => text.replace('.', ",") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    const SOURCES: &[(&str, &str)] = &[
        ("main.rs", include_str!("main.rs")),
        ("mesh.rs", include_str!("mesh.rs")),
        ("voice.rs", include_str!("voice.rs")),
        ("video.rs", include_str!("video.rs")),
        ("stream.rs", include_str!("stream.rs")),
        ("screen.rs", include_str!("screen.rs")),
        ("sounds.rs", include_str!("sounds.rs")),
        ("ui/mod.rs", include_str!("ui/mod.rs")),
        ("ui/chat.rs", include_str!("ui/chat.rs")),
        ("ui/rail.rs", include_str!("ui/rail.rs")),
        ("ui/settings.rs", include_str!("ui/settings.rs")),
        ("ui/share.rs", include_str!("ui/share.rs")),
        ("ui/stage.rs", include_str!("ui/stage.rs")),
        ("ui/theme.rs", include_str!("ui/theme.rs")),
        ("ui/tiles.rs", include_str!("ui/tiles.rs")),
        ("ui/widgets.rs", include_str!("ui/widgets.rs")),
    ];

    /// The source without `#[cfg(test)] mod ... { }` blocks (they contain sample calls).
    fn without_test_modules(source: &str) -> String {
        let mut out = String::new();
        let mut rest = source;
        while let Some(position) = rest.find("#[cfg(test)]\nmod ") {
            out.push_str(&rest[..position]);
            let after = &rest[position..];
            let open = after.find('{').expect("test module without a body");
            let (mut depth, mut in_string, mut escaped, mut end) = (0i32, false, false, after.len());
            for (offset, c) in after[open..].char_indices() {
                if in_string {
                    if escaped { escaped = false; } else if c == '\\' { escaped = true; } else if c == '"' { in_string = false; }
                    continue;
                }
                match c {
                    '"' => in_string = true,
                    '{' => depth += 1,
                    '}' => { depth -= 1; if depth == 0 { end = open + offset + 1; break; } }
                    _ => {}
                }
            }
            rest = &after[end..];
        }
        out.push_str(rest);
        out
    }

    /// Every English text passed to `tr(`, `trf(` or `english(` in the source files.
    fn used_texts() -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for (_, source) in SOURCES {
            // Only the code, not the test modules: they contain sample calls.
            let code = without_test_modules(source);
            let code = code.as_str();
            for marker in ["tr(", "trf(", "english("] {
                let mut rest = code;
                while let Some(position) = rest.find(marker) {
                    let before = rest[..position].chars().last();
                    rest = &rest[position + marker.len()..];
                    // `str(` or `fmt(` must not count as `tr(`.
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') { continue; }
                    // The text may start on the next line; a variable argument is not a literal.
                    let trimmed = rest.trim_start();
                    let Some(literal) = trimmed.strip_prefix('"') else { continue };
                    let mut text = String::new();
                    let mut chars = literal.chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => match chars.next() {
                                Some('"') => text.push('"'),
                                Some('\\') => text.push('\\'),
                                Some('n') => text.push('\n'),
                                other => panic!("unsupported escape {other:?} in a translated text"),
                            },
                            '"' => break,
                            c => text.push(c),
                        }
                    }
                    found.insert(text);
                }
            }
        }
        found
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            names.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        names
    }

    fn is_cyrillic(c: char) -> bool { ('\u{0400}'..='\u{04FF}').contains(&c) }

    #[test]
    fn english_is_the_default_language() {
        assert_eq!(Lang::default(), Lang::En);
        assert_eq!(Lang::from_code(""), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
        assert_eq!(Lang::from_code(" UK "), Lang::Uk);
        assert_eq!(Lang::from_code("ru"), Lang::Ru);
        for lang in Lang::ALL { assert_eq!(Lang::from_code(lang.code()), lang); }
    }

    #[test]
    fn every_text_used_in_the_code_has_a_translation() {
        let table: HashSet<&str> = TABLE.iter().map(|(en, _, _)| *en).collect();
        let missing: Vec<String> = used_texts().into_iter().filter(|t| !table.contains(t.as_str())).collect();
        assert!(missing.is_empty(), "texts missing from i18n_table.rs:\n{}", missing.join("\n"));
    }

    #[test]
    fn the_table_has_no_unused_or_duplicate_entries() {
        let used = used_texts();
        let mut seen = HashSet::new();
        for (en, _, _) in TABLE {
            assert!(seen.insert(*en), "duplicate entry: {en}");
            assert!(used.contains(*en), "entry is not used in the code: {en}");
        }
    }

    #[test]
    fn translations_are_complete_and_keep_the_same_placeholders() {
        for (en, uk, ru) in TABLE {
            assert!(!en.is_empty() && !uk.is_empty() && !ru.is_empty(), "empty text for {en:?}");
            assert_eq!(placeholders(en), placeholders(uk), "Ukrainian placeholders differ: {en}");
            assert_eq!(placeholders(en), placeholders(ru), "Russian placeholders differ: {en}");
            assert!(!en.chars().any(is_cyrillic), "the key must be English: {en}");
            for (name, text) in [("Ukrainian", uk), ("Russian", ru)] {
                let long_words = en.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| w.len() >= 4).count();
                if long_words >= 2 {
                    assert!(text.chars().any(is_cyrillic), "{name} text is not translated: {en}");
                }
            }
        }
    }

    #[test]
    fn ukrainian_and_russian_are_not_mixed_up() {
        for (en, uk, ru) in TABLE {
            assert!(!uk.chars().any(|c| "ыэъёЫЭЪЁ".contains(c)), "Russian letters in Ukrainian text: {uk}");
            assert!(!ru.chars().any(|c| "іїєґІЇЄҐ".contains(c)), "Ukrainian letters in Russian text: {ru} ({en})");
        }
    }

    #[test]
    fn translation_switches_with_the_language_and_falls_back_to_english() {
        assert_eq!(translate("Chat", Lang::En), "Chat");
        assert_eq!(translate("Settings", Lang::Uk), "Налаштування");
        assert_eq!(translate("Settings", Lang::Ru), "Настройки");
        assert_eq!(translate("A text that is not in the table", Lang::Uk), "A text that is not in the table");
        assert_eq!(fill(translate("{name} left", Lang::Uk), &[("name", &"Олена")]), "Олена вийшов(-ла)");
        assert_eq!(fill(translate("{name} left", Lang::Ru), &[("name", &"Олег")]), "Олег вышел(-ла)");
        assert_eq!(fill("{a} and {a} and {b}", &[("a", &1), ("b", &"x")]), "1 and 1 and x");
        assert_eq!(fill("Failed: {error}. Retry.", &[("error", &"unplugged.")]), "Failed: unplugged. Retry.");
        assert_eq!(fill("Failed: {error}", &[("error", &"unplugged.")]), "Failed: unplugged.");
        assert_eq!(decimal_in(Lang::Uk, 1.5, 1), "1,5");
        assert_eq!(decimal_in(Lang::Ru, 6.25, 2), "6,25");
        assert_eq!(decimal_in(Lang::En, 1.5, 1), "1.5");
    }
}
