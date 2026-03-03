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
`);

try {
  db.prepare("ALTER TABLE sessions ADD COLUMN expires_at DATETIME").run();
  db.prepare("UPDATE sessions SET expires_at = datetime('now', '+30 days') WHERE expires_at IS NULL").run();
} catch (e) { }

try {
  db.prepare("ALTER TABLE lobbies ADD COLUMN game_mode TEXT NOT NULL DEFAULT '101'").run();
} catch (e) { }

// Prune expired sessions
db.prepare("DELETE FROM sessions WHERE expires_at < datetime('now')").run();

const app = express();
const PORT = 3000;
let io: Server;

app.use(express.json());

// Helper functions
function getSanitizedState(state: any, userId: string) {
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
    market: [] // hide market
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
        clientSocket.emit('game_state_update', getSanitizedState(state, clientUserId));
      }
    }
  }
}

function updateGameState(lobbyId: string, state: any) {
  db.prepare('UPDATE game_states SET state_json = ?, updated_at = CURRENT_TIMESTAMP WHERE lobby_id = ?').run(JSON.stringify(state), lobbyId);
  if (state.status === 'finished') {
    db.prepare('UPDATE lobbies SET status = ? WHERE id = ?').run('finished', lobbyId);
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
  res.json({ user: { ...((req as any).user), isGuest: !!(req as any).user.is_guest } });
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
    checkAndRunAITurn(lobbyId, state);
  }

  res.json({ success: true });
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

    socket.on('join_lobby', (lobbyId) => {
      socket.join(`lobby_${lobbyId}`);
      const gameStateRow = db.prepare('SELECT state_json FROM game_states WHERE lobby_id = ?').get(lobbyId) as any;
      if (gameStateRow) {
        const state = JSON.parse(gameStateRow.state_json);
        socket.emit('game_state_update', getSanitizedState(state, userId));
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
        checkAndRunAITurn(lobbyId, state);
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
