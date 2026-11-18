-- Additive migration: existing DominoGo tables, IDs and statistics are retained.
CREATE TABLE IF NOT EXISTS uno_tables (
    id TEXT PRIMARY KEY CHECK(length(id)=7),
    game_id TEXT NOT NULL DEFAULT 'unogo' CHECK(game_id='unogo'),
    host_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    capacity INTEGER NOT NULL CHECK(capacity BETWEEN 2 AND 10),
    status TEXT NOT NULL DEFAULT 'waiting',
    match_id TEXT,
    state_json TEXT,
    saved_at INTEGER NOT NULL,
    bot_due INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS uno_seats (
    table_id TEXT NOT NULL REFERENCES uno_tables(id) ON DELETE CASCADE,
    seat INTEGER NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    PRIMARY KEY(table_id,seat),
    UNIQUE(table_id,user_id)
);
CREATE TABLE IF NOT EXISTS uno_results (
    match_id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL DEFAULT 'unogo' CHECK(game_id='unogo'),
    table_id TEXT NOT NULL,
    completed_at INTEGER NOT NULL,
    summary_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS uno_result_players (
    match_id TEXT NOT NULL REFERENCES uno_results(match_id),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    won INTEGER, -- NULL is no contest: no win or loss
    hidden INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(match_id,user_id)
);
CREATE INDEX IF NOT EXISTS uno_result_users ON uno_result_players(user_id,match_id);
CREATE TABLE IF NOT EXISTS uno_invitations (
    id TEXT PRIMARY KEY,
    table_id TEXT NOT NULL REFERENCES uno_tables(id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    receiver_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    UNIQUE(table_id, receiver_id)
);
CREATE TABLE IF NOT EXISTS uno_reactions (
    id TEXT PRIMARY KEY,
    table_id TEXT NOT NULL REFERENCES uno_tables(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS uno_recent_reactions ON uno_reactions(table_id,created_at);
-- Both namespaces reserve codes, including inserts from the legacy router.
CREATE TRIGGER IF NOT EXISTS uno_code_unique BEFORE INSERT ON uno_tables
WHEN EXISTS(SELECT 1 FROM lobbies WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'table code already reserved'); END;
CREATE TRIGGER IF NOT EXISTS domino_code_unique BEFORE INSERT ON lobbies
WHEN EXISTS(SELECT 1 FROM uno_tables WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'table code already reserved'); END;
