//! Account-specific ordering. Stable ties retain the familiar default order.
use std::collections::BTreeMap;

pub fn ordered(counts: &BTreeMap<String, u64>) -> Vec<&'static str> {
    let mut emojis = domino_core::REACTIONS.to_vec();
    emojis.sort_by_key(|emoji| std::cmp::Reverse(counts.get(*emoji).copied().unwrap_or(0)));
    emojis
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descending_counts_keep_default_ties_and_unused_emotes() {
        let counts = [
            ("😭".into(), 5),
            ("❤️".into(), 2),
            ("😂".into(), 2),
            ("unsupported".into(), u64::MAX),
        ]
        .into();
        let result = ordered(&counts);
        assert_eq!(&result[..3], &["😭", "😂", "❤️"]);
        assert_eq!(
            &result[3..],
            &domino_core::REACTIONS
                .iter()
                .copied()
                .filter(|s| !["😭", "😂", "❤️"].contains(s))
                .collect::<Vec<_>>()
        );
        assert_eq!(ordered(&BTreeMap::new()), domino_core::REACTIONS);
    }
}
