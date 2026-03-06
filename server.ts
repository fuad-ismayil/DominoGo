import express from 'express';
import { createServer as createViteServer } from 'vite';
import Database from 'better-sqlite3';
import crypto from 'crypto';
import path from 'path';
import { fileURLToPath } from 'url';
import http from 'http';
import { Server } from 'socket.io';
import { createGame, playTile, drawFromMarket, passTurn, nextRound, checkGameOver, GameState } from './src/game/engine';
import { getAIMove } from './src/game/ai';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const db = new Database('database.sqlite');
db.pragma('journal_mode = WAL');

// Initialize database
db.exec(`
  CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    is_guest BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS lobbies (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    max_players INTEGER NOT NULL,
    status TEXT DEFAULT 'waiting',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (creator_id) REFERENCES users (id)
  );

  CREATE TABLE IF NOT EXISTS lobby_players (
    lobby_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (lobby_id, user_id),
    FOREIGN KEY (lobby_id) REFERENCES lobbies (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS game_states (
    lobby_id TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (lobby_id) REFERENCES lobbies (id) ON DELETE CASCADE
  );

  INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_1', 'AI Bot 1', 1);
  INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_2', 'AI Bot 2', 1);
  INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_3', 'AI Bot 3', 1);

  CREATE TABLE IF NOT EXISTS friendships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id TEXT NOT NULL,
    receiver_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(sender_id, receiver_id)
  );

  CREATE TABLE IF NOT EXISTS game_invitations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lobby_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    receiver_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (lobby_id) REFERENCES lobbies(id) ON DELETE CASCADE,
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS player_stats (
    user_id TEXT PRIMARY KEY,
    wins INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    total_score INTEGER NOT NULL DEFAULT 0,
    games_played INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
  );
`);

try {
  db.prepare("ALTER TABLE sessions ADD COLUMN expires_at DATETIME").run();
  db.prepare("UPDATE sessions SET expires_at = datetime('now', '+30 days') WHERE expires_at IS NULL").run();
} catch (e) { }

try {
  db.prepare("ALTER TABLE lobbies ADD COLUMN game_mode TEXT NOT NULL DEFAULT '101'").run();
} catch (e) { }

// Add last_active_at column for guest expiry tracking
try {
  db.prepare("ALTER TABLE users ADD COLUMN last_active_at DATETIME").run();
  db.prepare("UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE last_active_at IS NULL").run();
} catch (e) { }

// Prune expired sessions
db.prepare("DELETE FROM sessions WHERE expires_at < datetime('now')").run();

// Clean up stale guest accounts (inactive for 24+ hours)
function cleanupStaleGuests() {
  try {
    // Delete sessions for stale guests
    db.prepare(`
      DELETE FROM sessions WHERE user_id IN (
        SELECT id FROM users 
        WHERE is_guest = 1 
        AND id NOT LIKE 'ai_%'
        AND last_active_at < datetime('now', '-24 hours')
      )
    `).run();

    // Delete lobby_players entries for stale guests
    db.prepare(`
      DELETE FROM lobby_players WHERE user_id IN (
        SELECT id FROM users 
        WHERE is_guest = 1 
        AND id NOT LIKE 'ai_%'
        AND last_active_at < datetime('now', '-24 hours')
      )
    `).run();

    // Delete the guest users themselves
    const result = db.prepare(`
      DELETE FROM users 
      WHERE is_guest = 1 
      AND id NOT LIKE 'ai_%'
      AND last_active_at < datetime('now', '-24 hours')
    `).run();

    if (result.changes > 0) {
      console.log(`Cleaned up ${result.changes} stale guest account(s)`);
    }
  } catch (e) {
    console.error('Guest cleanup error:', e);
  }
}

// Run cleanup on startup and every 30 minutes
cleanupStaleGuests();
setInterval(cleanupStaleGuests, 30 * 60 * 1000);

const app = express();
const PORT = 3000;
let io: Server;

app.use(express.json());

// Track online users: userId -> Set of socketIds
const onlineUsers = new Map<string, Set<string>>();

// Turn timer system: 30s per human turn
const TURN_TIMER_MS = 30000;
const turnTimers = new Map<string, NodeJS.Timeout>(); // lobbyId -> timeout
const turnDeadlines = new Map<string, number>(); // lobbyId -> epoch ms deadline

// Stop game voting system
const stopVotes = new Map<string, Set<string>>(); // lobbyId -> set of human player userIds who voted

function isUserOnline(userId: string): boolean {
  const sockets = onlineUsers.get(userId);
  return !!sockets && sockets.size > 0;
}

function getUserFriendIds(userId: string): string[] {
  const rows = db.prepare(`
    SELECT CASE WHEN sender_id = ? THEN receiver_id ELSE sender_id END as friend_id
    FROM friendships WHERE status = 'accepted' AND (sender_id = ? OR receiver_id = ?)
  `).all(userId, userId, userId) as any[];
  return rows.map(r => r.friend_id);
}

// Helper functions
function clearTurnTimer(lobbyId: string) {
  const existing = turnTimers.get(lobbyId);
  if (existing) {
    clearTimeout(existing);
    turnTimers.delete(lobbyId);
  }
  turnDeadlines.delete(lobbyId);
}

function startTurnTimer(lobbyId: string, state: any) {
  clearTurnTimer(lobbyId);

  // Don't start timers for non-playing states
  if (state.status !== 'playing') return;

  const currentPlayer = state.players[state.turnIndex];

  // Don't start timer for AI — AI has its own 1.5s delay
  if (currentPlayer.startsWith('ai_')) return;

  // Don't start timer if only 1 human player (solo vs AI) — no need to rush
  const humanPlayers = state.players.filter((p: string) => !p.startsWith('ai_'));
  if (humanPlayers.length <= 1) return;

  const deadline = Date.now() + TURN_TIMER_MS;
  turnDeadlines.set(lobbyId, deadline);

  const timer = setTimeout(() => {
    try {
      const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
      if (!row) return;
      let currentState = JSON.parse(row.state_json);

      // Verify it's still this player's turn and game is still playing
      if (currentState.status !== 'playing' || currentState.players[currentState.turnIndex] !== currentPlayer) return;

      // Auto-draw if market available, otherwise auto-pass
      const drawCount = currentState.players.length === 2 ? 2 : (currentState.players.length === 3 ? 1 : 0);
      if (drawCount > 0 && currentState.market.length > 0) {
        currentState = drawFromMarket(currentState, currentPlayer);
      } else {
        currentState = passTurn(currentState, currentPlayer);
        if (currentState.status === 'round_end') {
          currentState = checkGameOver(currentState);
        }
      }

      updateGameState(lobbyId, currentState);
      broadcastGameState(lobbyId, currentState);
      startTurnTimer(lobbyId, currentState);
      checkAndRunAITurn(lobbyId, currentState);
    } catch (e) {
      console.error('Turn timer auto-play error:', e);
    }
  }, TURN_TIMER_MS);

  turnTimers.set(lobbyId, timer);
}

function getSanitizedState(state: any, userId: string, lobbyId?: string) {
  const opponentHandCounts: Record<string, number> = {};
  for (const p of state.players) {
    if (p !== userId) {
      opponentHandCounts[p] = state.hands[p] ? state.hands[p].length : 0;
    }
  }

  return {
    ...state,
    hands: { [userId]: state.hands[userId] || [] },
    opponentHandCounts,
    marketCount: state.market.length,
    market: [], // hide market
    turnDeadline: lobbyId ? (turnDeadlines.get(lobbyId) || null) : null
  };
}

function broadcastGameState(lobbyId: string, state: any) {
  if (!io) return;
  const room = io.sockets.adapter.rooms.get(`lobby_${lobbyId}`);
  if (room) {
    for (const socketId of room) {
      const clientSocket = io.sockets.sockets.get(socketId);
      if (clientSocket) {
        const clientUserId = clientSocket.data.userId;
        clientSocket.emit('game_state_update', getSanitizedState(state, clientUserId, lobbyId));
      }
    }
  }
}

function updateGameState(lobbyId: string, state: any) {
  db.prepare('UPDATE game_states SET state_json = ?, updated_at = CURRENT_TIMESTAMP WHERE lobby_id = ?').run(JSON.stringify(state), lobbyId);
  if (state.status === 'finished' || state.status === 'stopped') {
    db.prepare('UPDATE lobbies SET status = ? WHERE id = ?').run(state.status, lobbyId);
    if (state.status === 'finished') recordMatchStats(state);
    clearTurnTimer(lobbyId);
  }
}

function recordMatchStats(state: any) {
  try {
    const winner = state.lastWinner;
    const players: string[] = state.players || [];

    for (const playerId of players) {
      if (playerId.startsWith('ai_')) continue; // Skip AI bots

      const isWinner = playerId === winner;
      const score = state.scores?.[playerId]?.permanent || 0;

      // Upsert player_stats
      db.prepare(`
        INSERT INTO player_stats (user_id, wins, losses, total_score, games_played)
        VALUES (?, ?, ?, ?, 1)
        ON CONFLICT(user_id) DO UPDATE SET
          wins = wins + ?,
          losses = losses + ?,
          total_score = total_score + ?,
          games_played = games_played + 1
      `).run(
        playerId,
        isWinner ? 1 : 0,
        isWinner ? 0 : 1,
        score,
        isWinner ? 1 : 0,
        isWinner ? 0 : 1,
        score
      );
    }
  } catch (e) {
    console.error('Failed to record match stats:', e);
  }
}

function checkAndRunAITurn(lobbyId: string, state: any) {
  if (state.status !== 'playing') return;
  const currentPlayer = state.players[state.turnIndex];

  if (currentPlayer.startsWith('ai_')) {
    setTimeout(() => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return;
        let currentState = JSON.parse(row.state_json);

        // Ensure it's still the AI's turn
        if (currentState.status !== 'playing' || currentState.players[currentState.turnIndex] !== currentPlayer) return;

        const action = getAIMove(currentState, currentPlayer);

        if (action.type === 'play') {
          currentState = playTile(currentState, currentPlayer, action.tile, action.end);
        } else if (action.type === 'draw') {
          currentState = drawFromMarket(currentState, currentPlayer);
        } else if (action.type === 'pass') {
          currentState = passTurn(currentState, currentPlayer);
        }

        if (currentState.status === 'round_end') {
          currentState = checkGameOver(currentState);
        }

        updateGameState(lobbyId, currentState);
        broadcastGameState(lobbyId, currentState);

        // Recursively check if the next player is also an AI (or if the AI drew and still has its turn)
        checkAndRunAITurn(lobbyId, currentState);
      } catch (e) {
        console.error("AI Error", e);
      }
    }, 1500); // 1.5s delay for realism
  }
}
const hashPassword = (password: string) => {
  return crypto.createHash('sha256').update(password).digest('hex');
};

const generateSessionToken = () => {
  return crypto.randomBytes(32).toString('hex');
};

// Authentication Middleware
const authenticate = (req: express.Request, res: express.Response, next: express.NextFunction) => {
  const authHeader = req.headers.authorization;
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const token = authHeader.split(' ')[1];
  const session = db.prepare("SELECT * FROM sessions WHERE id = ? AND (expires_at IS NULL OR expires_at > datetime('now'))").get(token) as any;

  if (!session) {
    db.prepare('DELETE FROM sessions WHERE id = ?').run(token);
    return res.status(401).json({ error: 'Invalid or expired session' });
  }

  const user = db.prepare('SELECT id, username, is_guest FROM users WHERE id = ?').get(session.user_id) as any;
  if (!user) {
    return res.status(401).json({ error: 'User not found' });
  }

  (req as any).user = user;
  (req as any).sessionToken = token;
  next();
};

// API Routes
app.post('/api/auth/register', (req, res) => {
  const { username, password } = req.body;

  if (!username || !password) {
    return res.status(400).json({ error: 'Username and password are required' });
  }

  if (password.length < 8) {
    return res.status(400).json({ error: 'Password must be at least 8 characters long' });
  }

  try {
    const userId = crypto.randomUUID();
    const passwordHash = hashPassword(password);

    db.prepare('INSERT INTO users (id, username, password_hash, is_guest) VALUES (?, ?, ?, 0)').run(userId, username, passwordHash);

    const token = generateSessionToken();
    db.prepare("INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))").run(token, userId);

    res.json({ token, user: { id: userId, username, isGuest: false } });
  } catch (error: any) {
    if (error.code === 'SQLITE_CONSTRAINT_UNIQUE') {
      return res.status(409).json({ error: 'Username already exists' });
    }
    res.status(500).json({ error: 'Internal server error' });
  }
});

app.post('/api/auth/login', (req, res) => {
  const { username, password } = req.body;

  if (!username || !password) {
    return res.status(400).json({ error: 'Username and password are required' });
  }

  const user = db.prepare('SELECT * FROM users WHERE username = ?').get(username) as any;

  if (!user || user.password_hash !== hashPassword(password)) {
    return res.status(401).json({ error: 'Invalid username or password' });
  }

  const token = generateSessionToken();
  db.prepare("INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))").run(token, user.id);

  res.json({ token, user: { id: user.id, username: user.username, isGuest: !!user.is_guest } });
});

app.post('/api/auth/guest', (req, res) => {
  const { prefix } = req.body;

  if (!prefix || typeof prefix !== 'string' || prefix.trim() === '') {
    return res.status(400).json({ error: 'Valid prefix is required' });
  }

  const cleanPrefix = prefix.trim().toLowerCase().replace(/[^a-z0-9]/g, '');
  if (!cleanPrefix) {
    return res.status(400).json({ error: 'Prefix must contain alphanumeric characters' });
  }

  let username = '';
  let attempts = 0;
  let success = false;
  let userId = crypto.randomUUID();

  while (!success && attempts < 10) {
    const randomChars = Math.random().toString(36).substring(2, 7).toUpperCase();
    username = `${cleanPrefix}-${randomChars}`;

    try {
      db.prepare('INSERT INTO users (id, username, is_guest) VALUES (?, ?, 1)').run(userId, username);
      success = true;
    } catch (error: any) {
      if (error.code !== 'SQLITE_CONSTRAINT_UNIQUE') {
        return res.status(500).json({ error: 'Internal server error' });
      }
    }
    attempts++;
  }

  if (!success) {
    return res.status(500).json({ error: 'Failed to generate unique guest username' });
  }

  const token = generateSessionToken();
  db.prepare("INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))").run(token, userId);

  res.json({ token, user: { id: userId, username, isGuest: true } });
});

app.get('/api/matches/history', authenticate, (req, res) => {
  const userId = (req as any).user.id;
  try {
    const matches = db.prepare(`
      SELECT l.id, l.status, strftime('%Y-%m-%dT%H:%M:%SZ', l.created_at) as created_at, 
             strftime('%Y-%m-%dT%H:%M:%SZ', gs.updated_at) as updated_at, gs.state_json,
             (SELECT COUNT(*) FROM lobby_players WHERE lobby_id = l.id) as player_count
      FROM lobbies l
      JOIN lobby_players lp ON l.id = lp.lobby_id
      LEFT JOIN game_states gs ON l.id = gs.lobby_id
      WHERE lp.user_id = ?
      ORDER BY COALESCE(gs.updated_at, l.created_at) DESC
      LIMIT 20
    `).all(userId);

    const formattedMatches = matches.map((m: any) => {
      let scores = null;
      let winner = null;
      let gameStatus = m.status;
      if (m.state_json) {
        try {
          const state = JSON.parse(m.state_json);
          scores = state.scores;
          if (state.status) {
            gameStatus = state.status;
          }
          if (state.status === 'finished') {
            // Find player with max score >= 101
            for (const p of state.players) {
              if (state.scores[p] && state.scores[p].permanent >= 101) {
                winner = p;
                break;
              }
            }
          }
        } catch (e) { }
      }

      const players = db.prepare(`
        SELECT u.id, u.username 
        FROM lobby_players lp 
        JOIN users u ON lp.user_id = u.id 
        WHERE lp.lobby_id = ?
        ORDER BY lp.joined_at ASC
      `).all(m.id);

      return {
        id: m.id,
        status: gameStatus,
        created_at: m.created_at,
        updated_at: m.updated_at,
        player_count: m.player_count,
        players,
        scores,
        winner
      };
    });

    res.json({ matches: formattedMatches });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

app.delete('/api/matches/:id', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;

  try {
    db.prepare('DELETE FROM lobby_players WHERE lobby_id = ? AND user_id = ?').run(lobbyId, userId);

    // Check if lobby is empty
    const players = db.prepare('SELECT * FROM lobby_players WHERE lobby_id = ?').all(lobbyId) as any[];
    if (players.length === 0) {
      db.prepare('DELETE FROM lobbies WHERE id = ?').run(lobbyId);
      db.prepare('DELETE FROM game_states WHERE lobby_id = ?').run(lobbyId);
    }

    res.json({ success: true });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

app.get('/api/auth/me', authenticate, (req, res) => {
  const user = (req as any).user;

  // Update last_active_at to reset the 24h guest expiry timer
  db.prepare('UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE id = ?').run(user.id);

  // Calculate guest expiry time (24h from now, since we just reset the timer)
  let guestExpiresAt = null;
  if (user.is_guest && !user.id.startsWith('ai_')) {
    const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000);
    guestExpiresAt = expiresAt.toISOString();
  }

  res.json({ user: { ...user, isGuest: !!user.is_guest, guestExpiresAt } });
});

app.post('/api/auth/logout', authenticate, (req, res) => {
  const token = (req as any).sessionToken;
  db.prepare('DELETE FROM sessions WHERE id = ?').run(token);
  res.json({ success: true });
});

app.post('/api/auth/logout-others', authenticate, (req, res) => {
  const token = (req as any).sessionToken;
  const userId = (req as any).user.id;
  db.prepare('DELETE FROM sessions WHERE user_id = ? AND id != ?').run(userId, token);
  res.json({ success: true });
});

app.post('/api/auth/upgrade', authenticate, (req, res) => {
  const user = (req as any).user;
  if (!user.is_guest) {
    return res.status(400).json({ error: 'Account is already registered' });
  }

  const { newUsername, password } = req.body;
  if (!password) {
    return res.status(400).json({ error: 'Password is required' });
  }
  if (password.length < 8) {
    return res.status(400).json({ error: 'Password must be at least 8 characters long' });
  }

  const finalUsername = newUsername && newUsername.trim() !== '' ? newUsername.trim() : user.username;

  try {
    const passwordHash = hashPassword(password);
    db.prepare('UPDATE users SET username = ?, password_hash = ?, is_guest = 0 WHERE id = ?').run(finalUsername, passwordHash, user.id);
    res.json({ success: true, user: { id: user.id, username: finalUsername, isGuest: false } });
  } catch (error: any) {
    if (error.code === 'SQLITE_CONSTRAINT_UNIQUE') {
      return res.status(409).json({ error: 'Username already exists' });
    }
    res.status(500).json({ error: 'Internal server error' });
  }
});

// Lobby Routes
app.post('/api/lobbies', authenticate, (req, res) => {
  const { maxPlayers } = req.body;
  const userId = (req as any).user.id;
  const lobbyId = Math.random().toString(36).substring(2, 9).toUpperCase();

  try {
    db.prepare('INSERT INTO lobbies (id, creator_id, max_players) VALUES (?, ?, ?)').run(lobbyId, userId, maxPlayers);
    db.prepare('INSERT INTO lobby_players (lobby_id, user_id) VALUES (?, ?)').run(lobbyId, userId);
    res.json({ id: lobbyId });
  } catch (err) {
    res.status(500).json({ error: 'Failed to create lobby' });
  }
});

app.post('/api/lobbies/:id/join', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;

  const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
  if (!lobby) return res.status(404).json({ error: 'Lobby not found' });

  const players = db.prepare('SELECT * FROM lobby_players WHERE lobby_id = ?').all(lobbyId) as any[];
  const isPlayerInLobby = players.find(p => p.user_id === userId);

  if (lobby.status !== 'waiting' && !isPlayerInLobby) {
    return res.status(400).json({ error: 'Game already started' });
  }

  if (players.length >= lobby.max_players && !isPlayerInLobby) {
    return res.status(400).json({ error: 'Lobby is full' });
  }

  try {
    db.prepare('INSERT OR IGNORE INTO lobby_players (lobby_id, user_id) VALUES (?, ?)').run(lobbyId, userId);

    const updatedPlayers = db.prepare(`
      SELECT u.username, u.id 
      FROM lobby_players lp 
      JOIN users u ON lp.user_id = u.id 
      WHERE lp.lobby_id = ?
      ORDER BY lp.joined_at ASC
    `).all(lobbyId);

    if (io) {
      io.to(`lobby_${lobbyId}`).emit('lobby_updated', { players: updatedPlayers });
    }

    let gameStatus = lobby.status;
    const gameStateRow = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
    if (gameStateRow) {
      try {
        const state = JSON.parse(gameStateRow.state_json);
        if (state.status) {
          gameStatus = state.status;
        }
      } catch (e) { }
    }

    res.json({ success: true, players: updatedPlayers, maxPlayers: lobby.max_players, creatorId: lobby.creator_id, status: gameStatus });
  } catch (err) {
    res.status(500).json({ error: 'Failed to join lobby' });
  }
});

app.post('/api/lobbies/:id/leave', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;

  db.prepare('DELETE FROM lobby_players WHERE lobby_id = ? AND user_id = ?').run(lobbyId, userId);

  const players = db.prepare(`
    SELECT u.username, u.id 
    FROM lobby_players lp 
    JOIN users u ON lp.user_id = u.id 
    WHERE lp.lobby_id = ?
    ORDER BY lp.joined_at ASC
  `).all(lobbyId);

  if (players.length === 0) {
    db.prepare('DELETE FROM lobbies WHERE id = ?').run(lobbyId);
  } else {
    if (io) {
      io.to(`lobby_${lobbyId}`).emit('lobby_updated', { players });
    }
  }

  res.json({ success: true });
});

app.post('/api/lobbies/:id/add-bot', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;

  const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
  if (!lobby) return res.status(404).json({ error: 'Lobby not found' });
  if (lobby.creator_id !== userId) return res.status(403).json({ error: 'Only creator can add bots' });
  if (lobby.status !== 'waiting') return res.status(400).json({ error: 'Game already started' });

  const players = db.prepare('SELECT * FROM lobby_players WHERE lobby_id = ?').all(lobbyId) as any[];
  if (players.length >= lobby.max_players) {
    return res.status(400).json({ error: 'Lobby is full' });
  }

  try {
    // Find next available bot ID
    let botNum = 1;
    while (players.find(p => p.user_id === `ai_${botNum}`)) {
      botNum++;
    }
    const botId = `ai_${botNum}`;

    db.prepare('INSERT INTO lobby_players (lobby_id, user_id) VALUES (?, ?)').run(lobbyId, botId);

    const updatedPlayers = db.prepare(`
      SELECT u.username, u.id 
      FROM lobby_players lp 
      JOIN users u ON lp.user_id = u.id 
      WHERE lp.lobby_id = ?
      ORDER BY lp.joined_at ASC
    `).all(lobbyId);

    if (io) {
      io.to(`lobby_${lobbyId}`).emit('lobby_updated', { players: updatedPlayers });
    }

    res.json({ success: true, players: updatedPlayers });
  } catch (err) {
    res.status(500).json({ error: 'Failed to add bot' });
  }
});

app.post('/api/lobbies/:id/kick', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;
  const { targetUserId } = req.body;

  const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
  if (!lobby) return res.status(404).json({ error: 'Lobby not found' });
  if (lobby.creator_id !== userId) return res.status(403).json({ error: 'Only creator can kick players' });
  if (lobby.status !== 'waiting') return res.status(400).json({ error: 'Game already started' });
  if (targetUserId === userId) return res.status(400).json({ error: 'Cannot kick yourself' });

  db.prepare('DELETE FROM lobby_players WHERE lobby_id = ? AND user_id = ?').run(lobbyId, targetUserId);

  const updatedPlayers = db.prepare(`
    SELECT u.username, u.id 
    FROM lobby_players lp 
    JOIN users u ON lp.user_id = u.id 
    WHERE lp.lobby_id = ?
    ORDER BY lp.joined_at ASC
  `).all(lobbyId);

  if (io) {
    io.to(`lobby_${lobbyId}`).emit('lobby_updated', { players: updatedPlayers });
  }

  res.json({ success: true, players: updatedPlayers });
});
app.post('/api/lobbies/:id/start', authenticate, (req, res) => {
  const lobbyId = req.params.id.toUpperCase();
  const userId = (req as any).user.id;

  const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
  if (!lobby) return res.status(404).json({ error: 'Lobby not found' });
  if (lobby.creator_id !== userId) return res.status(403).json({ error: 'Only creator can start' });

  const players = db.prepare('SELECT user_id FROM lobby_players WHERE lobby_id = ? ORDER BY joined_at ASC').all(lobbyId) as any[];
  const playerIds = players.map(p => p.user_id);

  const state = createGame(playerIds, '101');

  db.prepare('INSERT OR REPLACE INTO game_states (lobby_id, state_json) VALUES (?, ?)').run(lobbyId, JSON.stringify(state));
  db.prepare('UPDATE lobbies SET status = ? WHERE id = ?').run('playing', lobbyId);

  if (io) {
    io.to(`lobby_${lobbyId}`).emit('game_started');
    broadcastGameState(lobbyId, state);
    startTurnTimer(lobbyId, state);
    checkAndRunAITurn(lobbyId, state);
  }

  res.json({ success: true });
});

// ===== FRIEND SYSTEM API =====

app.post('/api/friends/request', authenticate, (req, res) => {
  const user = (req as any).user;
  if (user.is_guest) return res.status(403).json({ error: 'Create an account to use the friend system' });

  const { username } = req.body;
  if (!username) return res.status(400).json({ error: 'Username is required' });

  const target = db.prepare('SELECT id, is_guest FROM users WHERE username = ?').get(username) as any;
  if (!target) return res.status(404).json({ error: 'User not found' });
  if (target.is_guest) return res.status(400).json({ error: 'Cannot add guest users as friends' });
  if (target.id === user.id) return res.status(400).json({ error: 'Cannot add yourself' });

  // Check if friendship already exists (in either direction)
  const existing = db.prepare(
    'SELECT * FROM friendships WHERE (sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?)'
  ).get(user.id, target.id, target.id, user.id) as any;

  if (existing) {
    if (existing.status === 'accepted') return res.status(400).json({ error: 'Already friends' });
    return res.status(400).json({ error: 'Friend request already pending' });
  }

  db.prepare('INSERT INTO friendships (sender_id, receiver_id) VALUES (?, ?)').run(user.id, target.id);

  // Notify target in real-time if online
  if (io) {
    io.to(`user_${target.id}`).emit('friend_request_received', { id: user.id, username: user.username });
  }

  res.json({ success: true });
});

app.post('/api/friends/accept', authenticate, (req, res) => {
  const user = (req as any).user;
  const { friendshipId } = req.body;

  const friendship = db.prepare('SELECT * FROM friendships WHERE id = ? AND receiver_id = ? AND status = ?').get(friendshipId, user.id, 'pending') as any;
  if (!friendship) return res.status(404).json({ error: 'Request not found' });

  db.prepare('UPDATE friendships SET status = ? WHERE id = ?').run('accepted', friendshipId);

  // Notify sender in real-time
  if (io) {
    io.to(`user_${friendship.sender_id}`).emit('friend_request_accepted', { id: user.id, username: user.username });
  }

  res.json({ success: true });
});

app.post('/api/friends/reject', authenticate, (req, res) => {
  const user = (req as any).user;
  const { friendshipId } = req.body;

  const friendship = db.prepare('SELECT * FROM friendships WHERE id = ? AND receiver_id = ? AND status = ?').get(friendshipId, user.id, 'pending') as any;
  if (!friendship) return res.status(404).json({ error: 'Request not found' });

  db.prepare('DELETE FROM friendships WHERE id = ?').run(friendshipId);
  res.json({ success: true });
});

app.post('/api/friends/cancel', authenticate, (req, res) => {
  const user = (req as any).user;
  const { friendshipId } = req.body;

  const friendship = db.prepare('SELECT * FROM friendships WHERE id = ? AND sender_id = ? AND status = ?').get(friendshipId, user.id, 'pending') as any;
  if (!friendship) return res.status(404).json({ error: 'Request not found' });

  db.prepare('DELETE FROM friendships WHERE id = ?').run(friendshipId);
  res.json({ success: true });
});

app.delete('/api/friends/:targetUserId', authenticate, (req, res) => {
  const user = (req as any).user;
  const targetId = req.params.targetUserId;

  const result = db.prepare(
    'DELETE FROM friendships WHERE status = ? AND ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?))'
  ).run('accepted', user.id, targetId, targetId, user.id);

  if (result.changes === 0) return res.status(404).json({ error: 'Friendship not found' });

  // Notify the other user
  if (io) {
    io.to(`user_${targetId}`).emit('friend_removed', { userId: user.id });
  }

  res.json({ success: true });
});

app.get('/api/friends', authenticate, (req, res) => {
  const user = (req as any).user;
  if (user.is_guest) return res.json({ friends: [], incoming: [], outgoing: [], pendingGameInvites: [] });

  // Accepted friends
  const friendRows = db.prepare(`
    SELECT f.id as friendshipId,
      CASE WHEN f.sender_id = ? THEN f.receiver_id ELSE f.sender_id END as friend_id,
      CASE WHEN f.sender_id = ? THEN u2.username ELSE u1.username END as username
    FROM friendships f
    JOIN users u1 ON f.sender_id = u1.id
    JOIN users u2 ON f.receiver_id = u2.id
    WHERE f.status = 'accepted' AND (f.sender_id = ? OR f.receiver_id = ?)
  `).all(user.id, user.id, user.id, user.id) as any[];

  const friends = friendRows.map(r => ({
    id: r.friend_id,
    username: r.username,
    isOnline: isUserOnline(r.friend_id)
  }));

  // Incoming pending requests
  const incoming = db.prepare(`
    SELECT f.id, f.sender_id as userId, u.username
    FROM friendships f JOIN users u ON f.sender_id = u.id
    WHERE f.receiver_id = ? AND f.status = 'pending'
  `).all(user.id) as any[];

  // Outgoing pending requests
  const outgoing = db.prepare(`
    SELECT f.id, f.receiver_id as userId, u.username
    FROM friendships f JOIN users u ON f.receiver_id = u.id
    WHERE f.sender_id = ? AND f.status = 'pending'
  `).all(user.id) as any[];

  // Pending game invitations for this user
  const pendingGameInvites = db.prepare(`
    SELECT gi.id, gi.lobby_id as lobbyId, u.username as senderUsername
    FROM game_invitations gi JOIN users u ON gi.sender_id = u.id
    WHERE gi.receiver_id = ? AND gi.status = 'pending'
    AND gi.lobby_id IN (SELECT id FROM lobbies WHERE status IN ('waiting', 'playing'))
  `).all(user.id) as any[];

  res.json({ friends, incoming, outgoing, pendingGameInvites });
});

// ===== PLAYER STATS & LEADERBOARD =====

app.get('/api/stats/:userId', (req, res) => {
  const stats = db.prepare(`
    SELECT ps.*, u.username FROM player_stats ps
    JOIN users u ON ps.user_id = u.id
    WHERE ps.user_id = ?
  `).get(req.params.userId) as any;

  if (!stats) {
    const user = db.prepare('SELECT username FROM users WHERE id = ?').get(req.params.userId) as any;
    return res.json({
      username: user?.username || 'Unknown',
      wins: 0, losses: 0, totalScore: 0, gamesPlayed: 0, winRate: 0, avgScore: 0
    });
  }

  res.json({
    username: stats.username,
    wins: stats.wins,
    losses: stats.losses,
    totalScore: stats.total_score,
    gamesPlayed: stats.games_played,
    winRate: stats.games_played > 0 ? Math.round((stats.wins / stats.games_played) * 100) : 0,
    avgScore: stats.games_played > 0 ? Math.round(stats.total_score / stats.games_played) : 0
  });
});

app.get('/api/leaderboard', (req, res) => {
  const rows = db.prepare(`
    SELECT ps.user_id, ps.wins, ps.losses, ps.total_score, ps.games_played, u.username
    FROM player_stats ps
    JOIN users u ON ps.user_id = u.id
    WHERE u.is_guest = 0 AND ps.user_id NOT LIKE 'ai_%' AND ps.games_played >= 1
    ORDER BY ps.wins DESC, 
      CASE WHEN ps.games_played > 0 THEN CAST(ps.wins AS FLOAT) / ps.games_played ELSE 0 END DESC,
      ps.total_score DESC
    LIMIT 50
  `).all() as any[];

  const leaderboard = rows.map((r, idx) => ({
    rank: idx + 1,
    userId: r.user_id,
    username: r.username,
    wins: r.wins,
    losses: r.losses,
    gamesPlayed: r.games_played,
    winRate: r.games_played > 0 ? Math.round((r.wins / r.games_played) * 100) : 0,
    totalScore: r.total_score,
    avgScore: r.games_played > 0 ? Math.round(r.total_score / r.games_played) : 0
  }));

  res.json({ leaderboard });
});

async function startServer() {
  const httpServer = http.createServer(app);
  io = new Server(httpServer, {
    cors: { origin: '*' }
  });

  io.use((socket, next) => {
    const token = socket.handshake.auth.token;
    if (!token) return next(new Error('Authentication error'));
    const session = db.prepare('SELECT * FROM sessions WHERE id = ?').get(token) as any;
    if (!session) return next(new Error('Authentication error'));
    socket.data.userId = session.user_id;
    next();
  });

  io.on('connection', (socket) => {
    const userId = socket.data.userId;

    // ===== Online presence tracking =====
    socket.join(`user_${userId}`);
    if (!onlineUsers.has(userId)) {
      onlineUsers.set(userId, new Set());
    }
    onlineUsers.get(userId)!.add(socket.id);

    // If this is the user's first socket, broadcast online to friends
    if (onlineUsers.get(userId)!.size === 1) {
      const friendIds = getUserFriendIds(userId);
      for (const fid of friendIds) {
        io.to(`user_${fid}`).emit('friend_online', { userId });
      }
    }

    socket.on('disconnect', () => {
      const sockets = onlineUsers.get(userId);
      if (sockets) {
        sockets.delete(socket.id);
        if (sockets.size === 0) {
          onlineUsers.delete(userId);
          // Broadcast offline to friends
          const friendIds = getUserFriendIds(userId);
          for (const fid of friendIds) {
            io.to(`user_${fid}`).emit('friend_offline', { userId });
          }
        }
      }
    });

    socket.on('join_lobby', (lobbyId) => {
      socket.join(`lobby_${lobbyId}`);
      const gameStateRow = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
      if (gameStateRow) {
        const state = JSON.parse(gameStateRow.state_json);
        socket.emit('game_state_update', getSanitizedState(state, userId, lobbyId));
      }
    });

    socket.on('leave_lobby', (lobbyId) => {
      socket.leave(`lobby_${lobbyId}`);
    });

    socket.on('play_tile', ({ lobbyId, tile, end }, callback) => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return callback && callback({ error: 'Game not found' });
        let state = JSON.parse(row.state_json);

        state = playTile(state, userId, tile, end);
        if (state.status === 'round_end') {
          state = checkGameOver(state);
        }

        updateGameState(lobbyId, state);

        broadcastGameState(lobbyId, state);
        startTurnTimer(lobbyId, state);
        checkAndRunAITurn(lobbyId, state);
        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('draw_market', ({ lobbyId }, callback) => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return callback && callback({ error: 'Game not found' });
        let state = JSON.parse(row.state_json);

        state = drawFromMarket(state, userId);

        updateGameState(lobbyId, state);

        broadcastGameState(lobbyId, state);
        startTurnTimer(lobbyId, state);
        checkAndRunAITurn(lobbyId, state);
        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('pass_turn', ({ lobbyId }, callback) => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return callback && callback({ error: 'Game not found' });
        let state = JSON.parse(row.state_json);

        state = passTurn(state, userId);
        if (state.status === 'round_end') {
          state = checkGameOver(state);
        }

        updateGameState(lobbyId, state);

        broadcastGameState(lobbyId, state);
        startTurnTimer(lobbyId, state);
        checkAndRunAITurn(lobbyId, state);
        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('next_round', ({ lobbyId }, callback) => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return callback && callback({ error: 'Game not found' });
        let state = JSON.parse(row.state_json);

        if (state.status !== 'round_end') return callback && callback({ error: 'Round not ended' });

        state = nextRound(state);

        updateGameState(lobbyId, state);

        broadcastGameState(lobbyId, state);
        startTurnTimer(lobbyId, state);
        checkAndRunAITurn(lobbyId, state);
        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    // Quick reactions — broadcast emoji to all players in the lobby
    const ALLOWED_REACTIONS = ['👍', '👎', '😤', '🎉', '😂', '🔥', '💀', '🤝'];
    socket.on('send_reaction', ({ lobbyId, emoji }) => {
      if (!ALLOWED_REACTIONS.includes(emoji)) return;
      io.to(`lobby_${lobbyId}`).emit('reaction', { userId, emoji });
    });

    // Stop game voting system
    socket.on('stop_game_request', ({ lobbyId }, callback) => {
      try {
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (!row) return callback && callback({ error: 'Game not found' });
        const state = JSON.parse(row.state_json);
        if (state.status !== 'playing' && state.status !== 'round_end') {
          return callback && callback({ error: 'Game is not in progress' });
        }

        // Only human players can vote
        if (userId.startsWith('ai_')) return callback && callback({ error: 'AI cannot vote' });

        if (!stopVotes.has(lobbyId)) stopVotes.set(lobbyId, new Set());
        const votes = stopVotes.get(lobbyId)!;
        votes.add(userId);

        // Count required votes: all human players
        const humanPlayers = state.players.filter((p: string) => !p.startsWith('ai_'));
        const requiredVotes = humanPlayers.length;
        const currentVotes = votes.size;

        if (currentVotes >= requiredVotes) {
          // All humans confirmed — stop the game
          state.status = 'stopped';
          updateGameState(lobbyId, state);
          broadcastGameState(lobbyId, state);
          clearTurnTimer(lobbyId);
          stopVotes.delete(lobbyId);

          io.to(`lobby_${lobbyId}`).emit('stop_vote_update', { votes: currentVotes, required: requiredVotes, stopped: true, voters: Array.from(votes) });
        } else {
          // Notify all players of progress
          io.to(`lobby_${lobbyId}`).emit('stop_vote_update', { votes: currentVotes, required: requiredVotes, stopped: false, voters: Array.from(votes) });
        }

        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('cancel_stop_vote', ({ lobbyId }, callback) => {
      const votes = stopVotes.get(lobbyId);
      if (votes) {
        votes.delete(userId);
        if (votes.size === 0) stopVotes.delete(lobbyId);

        // Re-count required
        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (row) {
          const state = JSON.parse(row.state_json);
          const humanPlayers = state.players.filter((p: string) => !p.startsWith('ai_'));
          io.to(`lobby_${lobbyId}`).emit('stop_vote_update', { votes: votes.size, required: humanPlayers.length, stopped: false, voters: Array.from(votes) });
        }
      }
      if (callback) callback({ success: true });
    });

    // Rematch — host can restart game with same players
    socket.on('rematch', ({ lobbyId }, callback) => {
      try {
        const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
        if (!lobby) return callback && callback({ error: 'Lobby not found' });
        if (lobby.creator_id !== userId) return callback && callback({ error: 'Only the host can start a rematch' });

        const row = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
        if (row) {
          const oldState = JSON.parse(row.state_json);
          if (oldState.status !== 'finished' && oldState.status !== 'stopped') return callback && callback({ error: 'Game is not finished yet' });
        }

        const players = db.prepare('SELECT user_id FROM lobby_players WHERE lobby_id = ? ORDER BY joined_at ASC').all(lobbyId) as any[];
        const playerIds = players.map((p: any) => p.user_id);

        const state = createGame(playerIds, '101');

        db.prepare('UPDATE game_states SET state_json = ?, updated_at = CURRENT_TIMESTAMP WHERE lobby_id = ?').run(JSON.stringify(state), lobbyId);
        db.prepare('UPDATE lobbies SET status = ? WHERE id = ?').run('playing', lobbyId);

        io.to(`lobby_${lobbyId}`).emit('game_started');
        broadcastGameState(lobbyId, state);
        startTurnTimer(lobbyId, state);
        checkAndRunAITurn(lobbyId, state);

        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    // ===== Game Invitation Socket Events =====
    socket.on('invite_to_lobby', ({ lobbyId, friendUserId }, callback) => {
      try {
        // Verify they're friends
        const friendship = db.prepare(
          "SELECT * FROM friendships WHERE status = 'accepted' AND ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?))"
        ).get(userId, friendUserId, friendUserId, userId) as any;
        if (!friendship) return callback && callback({ error: 'Not friends' });

        // Verify lobby exists
        const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(lobbyId) as any;
        if (!lobby) return callback && callback({ error: 'Lobby not found' });

        // Check for existing pending invite
        const existingInvite = db.prepare(
          "SELECT * FROM game_invitations WHERE lobby_id = ? AND receiver_id = ? AND status = 'pending'"
        ).get(lobbyId, friendUserId) as any;
        if (existingInvite) return callback && callback({ error: 'Already invited' });

        // Create the invitation
        const senderUser = db.prepare('SELECT username FROM users WHERE id = ?').get(userId) as any;
        db.prepare('INSERT INTO game_invitations (lobby_id, sender_id, receiver_id) VALUES (?, ?, ?)').run(lobbyId, userId, friendUserId);

        // Notify the friend in real-time
        const invite = db.prepare('SELECT last_insert_rowid() as id').get() as any;
        io.to(`user_${friendUserId}`).emit('game_invitation', {
          id: invite.id,
          lobbyId,
          senderUsername: senderUser.username
        });

        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('accept_game_invite', ({ inviteId }, callback) => {
      try {
        const invite = db.prepare(
          "SELECT * FROM game_invitations WHERE id = ? AND receiver_id = ? AND status = 'pending'"
        ).get(inviteId, userId) as any;
        if (!invite) return callback && callback({ error: 'Invitation not found' });

        db.prepare("UPDATE game_invitations SET status = 'accepted' WHERE id = ?").run(inviteId);

        // Auto-join the lobby
        const lobby = db.prepare('SELECT * FROM lobbies WHERE id = ?').get(invite.lobby_id) as any;
        if (!lobby) return callback && callback({ error: 'Lobby no longer exists' });

        const players = db.prepare('SELECT * FROM lobby_players WHERE lobby_id = ?').all(invite.lobby_id) as any[];
        if (players.length >= lobby.max_players && !players.find((p: any) => p.user_id === userId)) {
          return callback && callback({ error: 'Lobby is full' });
        }

        db.prepare('INSERT OR IGNORE INTO lobby_players (lobby_id, user_id) VALUES (?, ?)').run(invite.lobby_id, userId);

        const updatedPlayers = db.prepare(`
          SELECT u.username, u.id FROM lobby_players lp
          JOIN users u ON lp.user_id = u.id WHERE lp.lobby_id = ? ORDER BY lp.joined_at ASC
        `).all(invite.lobby_id) as any[];

        io.to(`lobby_${invite.lobby_id}`).emit('lobby_updated', { players: updatedPlayers });

        // Notify the sender
        io.to(`user_${invite.sender_id}`).emit('game_invite_accepted', { userId, lobbyId: invite.lobby_id });

        if (callback) callback({
          success: true,
          lobbyId: invite.lobby_id,
          players: updatedPlayers,
          maxPlayers: lobby.max_players,
          creatorId: lobby.creator_id
        });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

    socket.on('reject_game_invite', ({ inviteId }, callback) => {
      try {
        const invite = db.prepare(
          "SELECT * FROM game_invitations WHERE id = ? AND receiver_id = ? AND status = 'pending'"
        ).get(inviteId, userId) as any;
        if (!invite) return callback && callback({ error: 'Invitation not found' });

        db.prepare("UPDATE game_invitations SET status = 'rejected' WHERE id = ?").run(inviteId);

        if (callback) callback({ success: true });
      } catch (err: any) {
        if (callback) callback({ error: err.message });
      }
    });

  });

  // Vite middleware for development
  if (process.env.NODE_ENV !== 'production') {
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    app.use(express.static(path.join(__dirname, 'dist')));
    app.get('*', (req, res) => {
      res.sendFile(path.join(__dirname, 'dist', 'index.html'));
    });
  }

  httpServer.listen(PORT, '0.0.0.0', () => {
    console.log(`Server running on http://0.0.0.0:${PORT}`);
  });
}

startServer();
