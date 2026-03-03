# DominoGo — Future Features & Updates

A realistic look at what can be improved, what's feasible to add, and what limitations exist.

---

## ✅ Short-Term (Low Effort, High Impact)

### Sound Effects & Haptics
- Tile place, tile draw, turn notification, round win/loss sounds
- Vibration feedback on mobile when it's your turn
- **Effort:** Small — just audio files + `new Audio().play()` calls

### Tile Animations
- Smooth slide-in when tiles are placed on the board
- "Domino!" celebration animation when a player wins a round
- **Effort:** Low — Framer Motion is already in the stack

### Chat / Quick Reactions
- Emoji reactions or preset messages during a game (👍, 😤, 🎉)
- No free-text chat needed — keeps moderation simple
- **Effort:** Low — a new Socket.io event + small UI overlay

### Rematch Button
- After a match ends, allow the host to start a new match with the same players without returning to lobby
- **Effort:** Low — reuse `createGame` + emit to existing socket room

---

## 🔧 Medium-Term (Moderate Effort)

### Spectator Mode
- Allow users to watch ongoing games without participating
- Server already sanitizes hands per-user, so spectators would just see the board + hand counts
- **Effort:** Medium — new join type, UI adjustments

### Player Statistics & Leaderboard
- Track wins, losses, win rate, average score per match
- Global leaderboard page
- **Limitation:** SQLite is fine for small scale, but aggregation queries will slow down with thousands of users. Would need indexing or a stats cache table.

### Smarter AI Bots
- Current AI (`ai.ts`) uses a simple "play highest pip tile" strategy
- Could add difficulty levels:
  - **Easy:** Random valid move
  - **Medium:** Current strategy (highest pip)
  - **Hard:** Track opponent hand estimates, block opponent moves, manage doubles strategically
- **Limitation:** True minimax is computationally expensive for dominos due to hidden information. A heuristic-based approach is more practical.

### Turn Timer
- Add a countdown timer per turn (e.g., 30 seconds)
- Auto-pass or auto-draw if the timer expires
- **Effort:** Medium — server-side `setTimeout` + client countdown display

---

## 🚀 Long-Term (High Effort)

### Board Layout: Snake / Pan & Zoom
- Current board caps at 2 rows and shrinks tiles to fit
- A proper infinite canvas with pan/zoom would let tiles stay readable as the board grows
- **Limitation:** Implementing smooth touch-based pan/zoom alongside the existing drag-and-drop (dnd-kit) system is complex — both fight for touch event control. May need a custom gesture handler or switching to a canvas-based renderer.

### PWA / Installable App
- Add a `manifest.json` and service worker so users can "install" DominoGo on their home screen
- Offline support is limited — the game requires a live server connection via WebSocket
- **Realistic scope:** Install prompt + cached static assets, but gameplay remains online-only

### Push Notifications
- "It's your turn!" notifications when the app is backgrounded
- **Limitation:** Requires a push notification service (Firebase FCM or similar), service worker setup, and user permission flow. Adds significant complexity.

### Private Rooms with Passwords
- Let hosts set a password on their lobby so only invited players can join
- **Effort:** Moderate — add a `password` column to lobbies, hash-check on join

### Matchmaking / Quick Play
- Auto-match players who want to play without sharing lobby codes
- **Limitation:** Needs a matchmaking queue system. With a small player base, wait times could be frustratingly long. Only viable if the game reaches a certain active user threshold.

---

## ⚠️ Known Limitations & Constraints

| Area | Limitation |
|---|---|
| **Database** | SQLite is single-file, no replication. Fine for hundreds of concurrent users, but not thousands. Migration to PostgreSQL would be needed for serious scale. |
| **Server** | Single Node.js process, single machine. No horizontal scaling without moving game state to Redis or a shared store. |
| **Deployment** | Currently deployed on Render free tier which sleeps after inactivity. Cold starts take 30-50 seconds. Upgrade to paid tier or use a platform like Railway/Fly.io for always-on. |
| **Security** | Passwords are hashed with SHA-256 (no salt). Should upgrade to bcrypt or argon2 for production-grade security. |
| **Real-time** | Socket.io works well but has no reconnection recovery for game state. If a player's connection drops mid-turn, they must refresh and rejoin — the game state is preserved server-side, but UI state resets. |
| **Mobile** | Landscape-only enforcement works but is jarring. No native app wrapper means no control over orientation lock. |
| **Monolith** | Everything lives in one 1500-line `App.tsx` and one 700-line `server.ts`. As features grow, these should be split into proper component/route modules. |

---

## 🚫 Not Planned / Out of Scope

- **Video/voice chat** — Too heavy, use Discord/WhatsApp alongside
- **Betting/real money** — Legal and compliance nightmare
- **Custom tile skins / shop** — Premature without a large user base
- **Offline multiplayer** — Requires P2P networking (WebRTC), fundamentally different architecture
