use serde::{Deserialize, Serialize};
use uno_core::{Card, Color, Value, View};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub is_guest: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Seat {
    pub slot: usize,
    pub user_id: Option<String>,
    pub name: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub id: String,
    pub host_id: String,
    pub max_players: usize,
    pub status: String,
    pub match_id: Option<String>,
    pub players: Vec<Seat>,
    pub local_seat: usize,
    pub accepted_sequence: u64,
    pub game: Option<View>,
    pub pause_votes: Option<Vec<String>>,
    #[serde(default)]
    pub server_now: Option<u64>,
    #[serde(default)]
    pub reactions: Vec<serde_json::Value>,
}
pub fn color_name(c: Option<Color>) -> &'static str {
    match c {
        Some(Color::Red) => "Red",
        Some(Color::Yellow) => "Yellow",
        Some(Color::Green) => "Green",
        Some(Color::Blue) => "Blue",
        None => "Wild",
    }
}
pub fn card_value(c: Card) -> String {
    match c.value {
        Value::Number(n) => n.to_string(),
        Value::Skip => "Skip".into(),
        Value::Reverse => "Reverse".into(),
        Value::DrawTwo => "+2".into(),
        Value::Wild => "Wild".into(),
        Value::WildFour => "+4".into(),
    }
}
/// Extend a selection only if it forms a legal same-number row.
pub fn select_cards(view: &View, selected: &[u8], clicked: u8) -> Vec<u8> {
    if selected.len() > 1 && selected.contains(&clicked) {
        let remaining: Vec<_> = selected
            .iter()
            .copied()
            .filter(|id| *id != clicked)
            .collect();
        return if remaining.len() > 1 && !uno_core::number_group_playable(view, &remaining) {
            remaining.last().copied().into_iter().collect()
        } else {
            remaining
        };
    }
    if selected.contains(&clicked) {
        return selected.to_vec();
    }
    let mut candidate = selected.to_vec();
    candidate.push(clicked);
    if uno_core::number_group_playable(view, &candidate) {
        candidate
    } else {
        vec![clicked]
    }
}
pub fn invite_code(query: &str) -> Result<Option<String>, &'static str> {
    let mut found = None;
    for pair in query.trim_start_matches('?').split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "join" {
            if found.is_some()
                || value.len() != 7
                || !value.bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return Err("Enter a seven-character table code");
            }
            found = Some(value.to_ascii_uppercase());
        }
    }
    Ok(found)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invite_parameters_are_bounded() {
        assert_eq!(
            invite_code("?join=abc1234").unwrap(),
            Some("ABC1234".into())
        );
        for q in [
            "?join=",
            "?join=abc1234&join=abc1234",
            "?join=../abcd",
            "?join=abc%204",
        ] {
            assert!(invite_code(q).is_err());
        }
        assert_eq!(invite_code("?other=hi").unwrap(), None);
    }
}
