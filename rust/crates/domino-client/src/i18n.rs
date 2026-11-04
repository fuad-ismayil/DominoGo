//! Complete Rust-side UI localization. Original English strings are stable keys.
use std::{collections::BTreeMap, sync::OnceLock};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    Az,
    Tr,
    Ru,
}
impl Language {
    pub const ALL: [Self; 4] = [Self::En, Self::Az, Self::Tr, Self::Ru];
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Az => "az",
            Self::Tr => "tr",
            Self::Ru => "ru",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Az => "Azərbaycanca",
            Self::Tr => "Türkçe",
            Self::Ru => "Русский",
        }
    }
    pub fn locale(self) -> &'static str {
        match self {
            Self::En => "en-GB",
            Self::Az => "az-AZ",
            Self::Tr => "tr-TR",
            Self::Ru => "ru-RU",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "az" => Self::Az,
            "tr" => Self::Tr,
            "ru" => Self::Ru,
            _ => Self::En,
        }
    }
}
type Catalog = BTreeMap<String, String>;
fn catalogs() -> &'static [Catalog; 4] {
    static CATALOGS: OnceLock<[Catalog; 4]> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        [
            include_str!("../locales/en.json"),
            include_str!("../locales/az.json"),
            include_str!("../locales/tr.json"),
            include_str!("../locales/ru.json"),
        ]
        .map(|s| serde_json::from_str(s).expect("validated translation catalogue"))
    })
}
pub fn translate(language: Language, source: &str) -> String {
    let index = match language {
        Language::En => 0,
        Language::Az => 1,
        Language::Tr => 2,
        Language::Ru => 3,
    };
    catalogs()[index]
        .get(source)
        .cloned()
        .unwrap_or_else(|| source.to_owned())
}
#[cfg(target_arch = "wasm32")]
pub static LANGUAGE: dioxus::prelude::GlobalSignal<Language> =
    dioxus::prelude::Signal::global(Language::default);
pub fn language() -> Language {
    #[cfg(target_arch = "wasm32")]
    {
        use dioxus::prelude::*;
        *LANGUAGE.read()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Language::En
    }
}
pub fn t(source: &str) -> String {
    translate(language(), source)
}
pub fn tf(source: &str, values: &[(&str, String)]) -> String {
    let text = t(source);
    let mut output = String::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}').map(|n| n + start) else {
            break;
        };
        output.push_str(&rest[..start]);
        let key = &rest[start + 1..end];
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            output.push_str(value);
        } else {
            output.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}
/// Positional formatting for preformatted values (including zero-padded time).
pub fn ta(source: &str, values: &[String]) -> String {
    let text = t(source);
    let mut output = String::new();
    let mut rest = text.as_str();
    for value in values {
        let Some(start) = rest.find('{') else { break };
        let Some(end) = rest[start..].find('}').map(|n| n + start) else {
            break;
        };
        output.push_str(&rest[..start]);
        output.push_str(value);
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}
#[cfg(target_arch = "wasm32")]
pub fn init() {
    use dioxus::prelude::*;
    let browser = crate::browser::window();
    let saved = browser
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item("domino.language").ok().flatten());
    let selected = Language::parse(
        &saved
            .or_else(|| browser.navigator().language())
            .unwrap_or_default(),
    );
    LANGUAGE.signal().set(selected);
    if let Some(root) = crate::browser::document().document_element() {
        let _ = root.set_attribute("lang", selected.code());
    }
}
#[cfg(target_arch = "wasm32")]
pub fn set_language(language: Language) {
    use dioxus::prelude::*;
    LANGUAGE.signal().set(language);
    if let Ok(Some(s)) = crate::browser::window().local_storage() {
        let _ = s.set_item("domino.language", language.code());
    }
    if let Some(root) = crate::browser::document().document_element() {
        let _ = root.set_attribute("lang", language.code());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tokens(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            if let Some(end) = rest[start..].find('}') {
                found.push(rest[start..start + end + 1].to_owned());
                rest = &rest[start + end + 1..];
            } else {
                break;
            }
        }
        found.sort();
        found
    }
    #[test]
    fn every_language_covers_the_interface_and_retains_all_placeholders() {
        let all = catalogs();
        for catalog in &all[1..] {
            assert_eq!(
                catalog.keys().collect::<Vec<_>>(),
                all[0].keys().collect::<Vec<_>>()
            );
            for (key, value) in catalog {
                assert!(!value.trim().is_empty(), "{key}");
                assert_eq!(tokens(key), tokens(value), "{key}");
            }
        }
    }
    #[test]
    fn localized_values_interpolate_and_language_preferences_parse() {
        assert_eq!(Language::parse("az-AZ"), Language::Az);
        assert_eq!(Language::parse("tr_TR"), Language::Tr);
        assert_eq!(Language::parse("ru"), Language::Ru);
        for lang in [Language::Az, Language::Tr, Language::Ru] {
            assert_ne!(translate(lang, "Next round"), "Next round");
            assert_ne!(
                translate(lang, "All seats are taken."),
                "All seats are taken."
            );
        }
        assert_eq!(
            tf("Send reaction {emoji}", &[("emoji", "👍".into())]),
            "Send reaction 👍"
        );
        assert_eq!(ta("{} and {}", &["1".into(), "2".into()]), "1 and 2");
    }
}
