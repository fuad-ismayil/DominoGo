//! Invite-link validation and user-facing outcomes, shared with native checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteProblem {
    Full,
    Started,
    Missing,
    Invalid,
    Unavailable,
}
impl InviteProblem {
    pub fn title(self) -> &'static str {
        match self {
            Self::Full => "All seats are taken.",
            Self::Started => "This round is already underway.",
            Self::Missing | Self::Invalid => "This seat is no longer available.",
            Self::Unavailable => "We couldn’t reach the table.",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Full => "Every seat at this table is taken. Ask your host to free up a seat, or start a table of your own.",
            Self::Started => "The host has already dealt the tiles. Ask them for a seat at the next table, or start one of your own.",
            Self::Missing | Self::Invalid => "This invite doesn’t lead to an open table. Ask your host for a fresh link, or start a table of your own.",
            Self::Unavailable => "Check your connection and open the invite again. Your next round will be here when you’re ready.",
        }
    }
}
pub fn normalize_table_code(raw: &str) -> Option<String> {
    (raw.len() == 7 && raw.bytes().all(|c| c.is_ascii_alphanumeric()))
        .then(|| raw.to_ascii_uppercase())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invite_codes_accept_only_seven_ascii_letters_and_digits() {
        assert_eq!(normalize_table_code("abC1234").as_deref(), Some("ABC1234"));
        for invalid in [
            "", "ABC123", "ABC12345", "ABC/123", " ABC123", "çBC123", "<script>",
        ] {
            assert!(normalize_table_code(invalid).is_none());
        }
    }
}
