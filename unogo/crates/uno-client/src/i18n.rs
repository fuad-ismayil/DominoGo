pub use crate::shared_i18n::{init, language, set_language, Language};
pub fn t(key: &str) -> String {
    use std::{collections::BTreeMap, sync::OnceLock};
    static CATALOG: OnceLock<BTreeMap<String, [String; 3]>> = OnceLock::new();
    let extra = CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/uno.json")).expect("UNO translations")
    });
    let index = match language() {
        Language::Az => Some(0),
        Language::Tr => Some(1),
        Language::Ru => Some(2),
        _ => None,
    };
    if let (Some(index), Some(translated)) = (index, extra.get(key)) {
        return translated[index].clone();
    }
    crate::shared_i18n::t(key)
}
