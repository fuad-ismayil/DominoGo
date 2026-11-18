//! Contextual feedback derived only from the visible authoritative state.
use crate::{
    board,
    i18n::{t, tf},
    types::SGameState,
};

pub fn guidance(state: &SGameState, user: &str, pending: bool, connected: bool) -> String {
    if !connected {
        return t("Reconnecting · your move will be available when connected.");
    }
    if pending {
        return t("Confirming your move…");
    }
    if state.status != "playing" {
        return String::new();
    }
    if state.players.get(state.turn_index).map(String::as_str) != Some(user) {
        return t("Your hand · plan your next move");
    }
    let hand = state.hands.get(user).map(Vec::as_slice).unwrap_or_default();
    let playable = hand.iter().find(|tile| {
        board::placements(state, user)
            .iter()
            .any(|(t, _)| t == *tile)
    });
    if state.board.is_empty() {
        if let Some(tile) = playable {
            return if state.round_number == 1 {
                tf(
                    "Open with {a}–{b} · select the tile, then the opening.",
                    &[("a", tile[0].to_string()), ("b", tile[1].to_string())],
                )
            } else {
                t("Open with any tile · select it, then the opening.")
            };
        }
    }
    if playable.is_none() && board::can_draw(state, user) {
        let count = (domino_core::draw_count_for(state.players.len()) as i64)
            .min(state.market_count)
            .max(0);
        return tf(
            "No matching tile · draw {count} (market: {remaining}).",
            &[
                ("count", count.to_string()),
                ("remaining", state.market_count.to_string()),
            ],
        );
    }
    if board::can_pass(state, user) {
        return t("No matching tile and no draw available · pass your turn.");
    }
    if state.mode == "telefon" {
        return t(
            "Select a glowing tile, then a highlighted end. Side branches must score in fives.",
        );
    }
    if let [Some(left), Some(right)] = state.open_ends {
        return tf(
            "Match {left} or {right} · select a glowing tile, then an end.",
            &[("left", left.to_string()), ("right", right.to_string())],
        );
    }
    t("Select a glowing tile, then an open end.")
}

/// Final totals cannot establish whether this round promoted earlier pending points.
/// Describe observable totals rather than inventing a previous score or lost points.
pub fn score_explanation(state: &SGameState) -> String {
    if state.mode == "telefon" {
        return t("Open ends score in fives. Round bonuses are rounded to the nearest five. First to 365 wins.");
    }
    if !matches!(state.status.as_str(), "round_end" | "finished") {
        return String::new();
    }
    let Some(winner) = state.last_winner.as_ref() else {
        return String::new();
    };
    let Some(score) = state.scores.get(winner) else {
        return String::new();
    };
    let collected: i32 = state
        .players
        .iter()
        .filter(|id| *id != winner)
        .map(|id| board::hand_points(state.hands.get(id).map(Vec::as_slice).unwrap_or_default()))
        .sum();
    if score.temporary > 0 {
        tf(
            "Collected {points} · Pending total {total}. Bank above 12 with consecutive wins.",
            &[
                ("points", collected.to_string()),
                ("total", score.temporary.to_string()),
            ],
        )
    } else {
        tf(
            "Collected {points} · Banked total {total} / 101.",
            &[
                ("points", collected.to_string()),
                ("total", score.permanent.to_string()),
            ],
        )
    }
}

/// Explain an observed score transition; mismatched or incomplete snapshots use totals only.
pub fn score_transaction(state: &SGameState, previous: Option<&SGameState>, user: &str) -> String {
    if state.mode == "telefon" {
        return score_explanation(state);
    }
    let fallback = || score_explanation(state);
    let Some(previous) = previous.filter(|old| {
        old.status == "playing"
            && old.round_number == state.round_number
            && old.players == state.players
            && matches!(state.status.as_str(), "round_end" | "finished")
    }) else {
        return fallback();
    };
    let Some(winner) = state.last_winner.as_ref() else {
        return fallback();
    };
    let (Some(old), Some(new)) = (previous.scores.get(winner), state.scores.get(winner)) else {
        return fallback();
    };
    // All remaining hands must be public before attributing a round transaction.
    if state.players.iter().any(|id| !state.hands.contains_key(id)) {
        return fallback();
    }
    let points: i32 = state
        .players
        .iter()
        .filter(|id| *id != winner)
        .map(|id| board::hand_points(&state.hands[id]))
        .sum();
    let (key, before, total) = if old.permanent > 0
        && new.permanent == old.permanent + points
        && new.temporary == 0
    {
        (
            "Banked {before} + Collected {points} = Banked {total} / 101.",
            old.permanent,
            new.permanent,
        )
    } else if old.permanent == 0
        && old.temporary + points > 12
        && new.permanent == old.temporary + points
        && new.temporary == 0
    {
        ("Pending {before} + Collected {points} = Banked {total} / 101. Above 12: points banked.", old.temporary, new.permanent)
    } else if old.permanent == 0
        && new.permanent == 0
        && new.temporary == old.temporary + points
        && new.temporary <= 12
    {
        ("Pending {before} + Collected {points} = Pending {total}. Bank above 12 with consecutive wins.", old.temporary, new.temporary)
    } else {
        return fallback();
    };
    let mut text = tf(
        key,
        &[
            ("before", before.to_string()),
            ("points", points.to_string()),
            ("total", total.to_string()),
        ],
    );
    if user != winner {
        if let (Some(old), Some(new)) = (previous.scores.get(user), state.scores.get(user)) {
            if old.temporary > 0 && new.temporary == 0 && old.permanent == new.permanent {
                text.push(' ');
                text.push_str(&tf(
                    "Your {points} pending points cleared after this round loss.",
                    &[("points", old.temporary.to_string())],
                ));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SScore;
    fn state() -> SGameState {
        SGameState {
            status: "playing".into(),
            players: vec!["me".into(), "other".into()],
            hands: [("me".into(), vec![[0, 0], [3, 3], [1, 2]])].into(),
            round_number: 1,
            ..Default::default()
        }
    }
    #[test]
    fn opening_respects_double_priority_and_later_round_freedom() {
        let mut s = state();
        assert_eq!(
            guidance(&s, "me", false, true),
            "Open with 3–3 · select the tile, then the opening."
        );
        s.round_number = 2;
        assert!(guidance(&s, "me", false, true).contains("any tile"));
    }
    #[test]
    fn blocked_draw_is_capped_by_remaining_market_then_passes() {
        let mut s = state();
        s.board = vec![[6, 6]];
        s.open_ends = [Some(6), Some(6)];
        s.market_count = 1;
        assert_eq!(
            guidance(&s, "me", false, true),
            "No matching tile · draw 1 (market: 1)."
        );
        s.market_count = 0;
        assert!(guidance(&s, "me", false, true).contains("pass your turn"));
        s.market_count = 4;
        s.players.extend(["third".into(), "fourth".into()]);
        assert!(guidance(&s, "me", false, true).contains("pass your turn"));
    }
    #[test]
    fn optional_draw_does_not_obscure_available_placement_and_turn_status_wins() {
        let mut s = state();
        s.board = vec![[1, 3]];
        s.open_ends = [Some(1), Some(3)];
        s.market_count = 10;
        assert!(guidance(&s, "me", false, true).starts_with("Match 1 or 3"));
        s.turn_index = 1;
        assert_eq!(
            guidance(&s, "me", false, true),
            "Your hand · plan your next move"
        );
        assert!(guidance(&s, "me", true, false).starts_with("Reconnecting"));
    }
    #[test]
    fn review_counts_lone_blank_and_explains_pending_without_inventing_history() {
        let mut s = state();
        s.status = "round_end".into();
        s.last_winner = Some("me".into());
        s.hands.insert("other".into(), vec![[0, 0]]);
        s.scores.insert(
            "me".into(),
            SScore {
                temporary: 12,
                permanent: 0,
            },
        );
        assert_eq!(
            score_explanation(&s),
            "Collected 10 · Pending total 12. Bank above 12 with consecutive wins."
        );
        s.scores.insert(
            "me".into(),
            SScore {
                temporary: 0,
                permanent: 22,
            },
        );
        assert_eq!(
            score_explanation(&s),
            "Collected 10 · Banked total 22 / 101."
        );
        s.status = "stopped".into();
        assert!(score_explanation(&s).is_empty());
    }
    fn transaction(
        previous_pending: i32,
        previous_banked: i32,
        pending: i32,
        banked: i32,
    ) -> (SGameState, SGameState) {
        let mut old = state();
        old.scores.insert(
            "me".into(),
            SScore {
                temporary: previous_pending,
                permanent: previous_banked,
            },
        );
        let mut new = old.clone();
        new.status = "round_end".into();
        new.last_winner = Some("me".into());
        new.hands.insert("me".into(), vec![]);
        new.hands.insert("other".into(), vec![[0, 0]]);
        new.scores.insert(
            "me".into(),
            SScore {
                temporary: pending,
                permanent: banked,
            },
        );
        (old, new)
    }
    #[test]
    fn observed_pending_accumulation_and_promotion_use_actual_previous_total() {
        let (old, new) = transaction(2, 0, 12, 0);
        assert_eq!(
            score_transaction(&new, Some(&old), "me"),
            "Pending 2 + Collected 10 = Pending 12. Bank above 12 with consecutive wins."
        );
        let (old, new) = transaction(3, 0, 0, 13);
        assert_eq!(
            score_transaction(&new, Some(&old), "me"),
            "Pending 3 + Collected 10 = Banked 13 / 101. Above 12: points banked."
        );
    }
    #[test]
    fn established_banked_score_and_local_lost_pending_are_observed() {
        let (mut old, mut new) = transaction(0, 40, 0, 50);
        old.scores.insert(
            "other".into(),
            SScore {
                temporary: 8,
                permanent: 0,
            },
        );
        new.scores.insert("other".into(), SScore::default());
        assert_eq!(score_transaction(&new,Some(&old),"other"), "Banked 40 + Collected 10 = Banked 50 / 101. Your 8 pending points cleared after this round loss.");
        assert!(!score_transaction(&new, Some(&old), "me").contains("cleared"));
    }
    #[test]
    fn absent_stale_and_inconsistent_snapshots_never_invent_a_transaction() {
        let (mut old, new) = transaction(3, 0, 0, 13);
        assert_eq!(score_transaction(&new, None, "me"), score_explanation(&new));
        old.round_number += 1;
        assert_eq!(
            score_transaction(&new, Some(&old), "me"),
            score_explanation(&new)
        );
        old.round_number = new.round_number;
        old.scores.get_mut("me").unwrap().temporary = 1;
        assert_eq!(
            score_transaction(&new, Some(&old), "me"),
            score_explanation(&new)
        );
        assert_eq!(guidance(&old, "me", true, true), "Confirming your move…");
    }
}
