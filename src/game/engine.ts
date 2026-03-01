export type Tile = [number, number];
export type GameMode = '101' | '5-lik';

export interface PlayerScore {
  temporary: number;
  permanent: number;
}

export interface GameState {
  mode: GameMode;
  players: string[];
  hands: Record<string, Tile[]>;
  market: Tile[];
  board: Tile[]; // Ordered sequence of played tiles
  openEnds: [number | null, number | null]; // [left, right]
  scores: Record<string, PlayerScore>;
  turnIndex: number;
  status: 'playing' | 'round_end' | 'finished';
  roundNumber: number;
  lastWinner: string | null;
  consecutivePasses: number;
}

/**
 * Generates a standard double-six domino set and shuffles it.
 */
export function generateTiles(): Tile[] {
  const tiles: Tile[] = [];
  for (let i = 0; i <= 6; i++) {
    for (let j = i; j <= 6; j++) {
      tiles.push([i, j]);
    }
  }
  // Fisher-Yates shuffle
  for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
  }
  return tiles;
}

/**
 * Applies the 101 mode scoring rules (Temporary vs Permanent).
 */
function apply101Scoring(scores: Record<string, PlayerScore>, winnerId: string, roundPoints: number, players: string[]): Record<string, PlayerScore> {
  const newScores = { ...scores };
  
  // Reset temporary scores for losers
  for (const p of players) {
    if (p !== winnerId) {
      newScores[p] = { ...newScores[p], temporary: 0 };
    }
  }

  // Update winner
  const winnerScore = newScores[winnerId];
  
  if (winnerScore.permanent > 0) {
    newScores[winnerId] = {
      temporary: 0,
      permanent: winnerScore.permanent + roundPoints
    };
  } else {
    const newTotal = winnerScore.temporary + roundPoints;
    if (newTotal > 12) {
      newScores[winnerId] = {
        temporary: 0,
        permanent: newTotal
      };
    } else {
      newScores[winnerId] = {
        ...winnerScore,
        temporary: newTotal
      };
    }
  }
  
  return newScores;
}

/**
 * Starts a new round, dealing tiles and determining the starting player.
 */
export function startRound(state: GameState): GameState {
  const players = state.players;
  let hands: Record<string, Tile[]> = {};
  let market: Tile[] = [];
  let startingPlayerIndex = 0;
  let validDistribution = false;

  // Keep dealing until we meet the starting tile requirements for the first round
  while (!validDistribution) {
    const tiles = generateTiles();
    hands = {};
    players.forEach(p => hands[p] = []);
    
    let tileIndex = 0;
    for (let i = 0; i < 7; i++) {
      for (const p of players) {
        hands[p].push(tiles[tileIndex++]);
      }
    }
    
    market = tiles.slice(tileIndex);
    
    if (state.roundNumber === 1) {
      if (state.mode === '101') {
        const doubles = [[1,1], [2,2], [3,3], [4,4], [5,5], [6,6], [0,0]];
        let found = false;
        for (const d of doubles) {
          for (let i = 0; i < players.length; i++) {
            if (hands[players[i]].some(t => (t[0] === d[0] && t[1] === d[1]))) {
              startingPlayerIndex = i;
              found = true;
              break;
            }
          }
          if (found) break;
        }
        if (found) validDistribution = true;
      } else if (state.mode === '5-lik') {
        let found = false;
        for (let i = 0; i < players.length; i++) {
          if (hands[players[i]].some(t => (t[0] === 2 && t[1] === 3) || (t[0] === 3 && t[1] === 2))) {
            startingPlayerIndex = i;
            found = true;
            break;
          }
        }
        if (found) validDistribution = true;
      }
    } else {
      // Subsequent rounds: winner of previous round starts
      startingPlayerIndex = players.indexOf(state.lastWinner || players[0]);
      if (startingPlayerIndex === -1) startingPlayerIndex = 0;
      validDistribution = true;
    }
  }

  return {
    ...state,
    hands,
    market,
    board: [],
    openEnds: [null, null],
    turnIndex: startingPlayerIndex,
    status: 'playing',
    consecutivePasses: 0
  };
}

/**
 * Creates a new game state.
 */
export function createGame(players: string[], mode: GameMode): GameState {
  const initialScores: Record<string, PlayerScore> = {};
  players.forEach(p => initialScores[p] = { temporary: 0, permanent: 0 });
  
  const initialState: GameState = {
    mode,
    players,
    hands: {},
    market: [],
    board: [],
    openEnds: [null, null],
    scores: initialScores,
    turnIndex: 0,
    status: 'playing',
    roundNumber: 1,
    lastWinner: null,
    consecutivePasses: 0
  };
  
  return startRound(initialState);
}

/**
 * Checks if a tile can be played on the given open ends.
 */
export function canPlayTile(tile: Tile, openEnds: [number | null, number | null]): boolean {
  if (openEnds[0] === null && openEnds[1] === null) return true;
  return tile[0] === openEnds[0] || tile[1] === openEnds[0] || 
         tile[0] === openEnds[1] || tile[1] === openEnds[1];
}

/**
 * Plays a tile from a player's hand to the board.
 */
export function playTile(state: GameState, playerId: string, tile: Tile, end: 'left' | 'right'): GameState {
  if (state.status !== 'playing') throw new Error("Game is not in playing state");
  if (state.players[state.turnIndex] !== playerId) throw new Error("Not your turn");
  
  const hand = state.hands[playerId];
  const tileIndex = hand.findIndex(t => (t[0] === tile[0] && t[1] === tile[1]) || (t[0] === tile[1] && t[1] === tile[0]));
  if (tileIndex === -1) throw new Error("Tile not in hand");

  const actualTile = hand[tileIndex];
  let newOpenEnds: [number | null, number | null] = [...state.openEnds];
  let orientedTile: Tile = [actualTile[0], actualTile[1]];
  
  // First move validation (101 must start with double, 5-lik must start with 2-3)
  if (state.board.length === 0 && state.roundNumber === 1) {
    if (state.mode === '101') {
      const doubles = [[1,1], [2,2], [3,3], [4,4], [5,5], [6,6], [0,0]];
      let requiredDouble: Tile | null = null;
      
      for (const d of doubles) {
        if (hand.some(t => t[0] === d[0] && t[1] === d[1])) {
          requiredDouble = d as Tile;
          break;
        }
      }
      
      if (requiredDouble) {
        if (actualTile[0] !== requiredDouble[0] || actualTile[1] !== requiredDouble[1]) {
          throw new Error(`First round of 101 must start with your lowest double (${requiredDouble[0]}-${requiredDouble[1]})`);
        }
      } else if (actualTile[0] !== actualTile[1]) {
        throw new Error("First round of 101 must start with a double");
      }
    }
    if (state.mode === '5-lik' && !((actualTile[0] === 2 && actualTile[1] === 3) || (actualTile[0] === 3 && actualTile[1] === 2))) {
      throw new Error("First round of 5-lik must start with 2-3");
    }
  }

  if (state.board.length === 0) {
    newOpenEnds = [actualTile[0], actualTile[1]];
  } else {
    if (end === 'left') {
      if (actualTile[0] === state.openEnds[0]) {
        newOpenEnds[0] = actualTile[1];
        orientedTile = [actualTile[1], actualTile[0]];
      } else if (actualTile[1] === state.openEnds[0]) {
        newOpenEnds[0] = actualTile[0];
        orientedTile = [actualTile[0], actualTile[1]];
      } else {
        throw new Error("Invalid move on left end");
      }
    } else {
      if (actualTile[0] === state.openEnds[1]) {
        newOpenEnds[1] = actualTile[1];
        orientedTile = [actualTile[0], actualTile[1]];
      } else if (actualTile[1] === state.openEnds[1]) {
        newOpenEnds[1] = actualTile[0];
        orientedTile = [actualTile[1], actualTile[0]];
      } else {
        throw new Error("Invalid move on right end");
      }
    }
  }

  const newHand = [...hand];
  newHand.splice(tileIndex, 1);
  
  const newHands = { ...state.hands, [playerId]: newHand };
  const newBoard = state.board.length === 0 ? [orientedTile] : (end === 'left' ? [orientedTile, ...state.board] : [...state.board, orientedTile]);
  let newScores = { ...state.scores };
  
  // 5-lik mid-round scoring
  if (state.mode === '5-lik') {
    const sum = (newOpenEnds[0] || 0) + (newOpenEnds[1] || 0);
    if (sum % 5 === 0 && sum > 0) {
      newScores[playerId] = {
        ...newScores[playerId],
        permanent: newScores[playerId].permanent + sum
      };
    }
  }

  let newStatus: 'playing' | 'round_end' | 'finished' = state.status;
  let lastWinner = state.lastWinner;
  
  // Check for round win (Domino!)
  if (newHand.length === 0) {
    newStatus = 'round_end';
    lastWinner = playerId;
    
    let roundPoints = 0;
    for (const p of state.players) {
      if (p !== playerId) {
        const pHand = newHands[p];
        if (pHand.length === 1 && pHand[0][0] === 0 && pHand[0][1] === 0) {
          roundPoints += 10;
        } else {
          roundPoints += pHand.reduce((acc, t) => acc + t[0] + t[1], 0);
        }
      }
    }
    
    if (state.mode === '101') {
      newScores = apply101Scoring(newScores, playerId, roundPoints, state.players);
    } else {
      newScores[playerId].permanent += roundPoints;
    }
  }

  return {
    ...state,
    hands: newHands,
    board: newBoard,
    openEnds: newOpenEnds,
    scores: newScores,
    turnIndex: newStatus === 'round_end' ? state.turnIndex : (state.turnIndex + 1) % state.players.length,
    status: newStatus,
    lastWinner,
    consecutivePasses: 0
  };
}

/**
 * Draws tiles from the market based on player count rules.
 */
export function drawFromMarket(state: GameState, playerId: string): GameState {
  if (state.status !== 'playing') throw new Error("Game is not in playing state");
  if (state.players[state.turnIndex] !== playerId) throw new Error("Not your turn");
  if (state.market.length === 0) throw new Error("Market is empty");
  
  const drawCount = state.players.length === 2 ? 2 : (state.players.length === 3 ? 1 : 0);
  if (drawCount === 0) throw new Error("No market draws allowed in 4-player game");
  
  const actualDrawCount = Math.min(drawCount, state.market.length);
  const drawnTiles = state.market.slice(0, actualDrawCount);
  const newMarket = state.market.slice(actualDrawCount);
  
  const newHands = {
    ...state.hands,
    [playerId]: [...state.hands[playerId], ...drawnTiles]
  };
  
  return {
    ...state,
    hands: newHands,
    market: newMarket
  };
}

/**
 * Passes the turn. If all players pass consecutively, the round ends (blocked game).
 */
export function passTurn(state: GameState, playerId: string): GameState {
  if (state.status !== 'playing') throw new Error("Game is not in playing state");
  if (state.players[state.turnIndex] !== playerId) throw new Error("Not your turn");
  
  // Note: In a strict engine, we might validate that the player actually has no valid moves 
  // and the market is empty. For flexibility, we assume the caller validated this.

  const newConsecutivePasses = state.consecutivePasses + 1;
  let newStatus: 'playing' | 'round_end' | 'finished' = state.status;
  let lastWinner = state.lastWinner;
  let newScores = { ...state.scores };

  // Blocked game condition
  if (newConsecutivePasses >= state.players.length) {
    newStatus = 'round_end';
    
    // Find player with lowest hand sum
    let minSum = Infinity;
    let winner = state.players[0];
    const sums: Record<string, number> = {};
    
    for (const p of state.players) {
      const pHand = state.hands[p];
      let sum = 0;
      if (pHand.length === 1 && pHand[0][0] === 0 && pHand[0][1] === 0) {
        sum = 10;
      } else {
        sum = pHand.reduce((acc, t) => acc + t[0] + t[1], 0);
      }
      sums[p] = sum;
      if (sum < minSum) {
        minSum = sum;
        winner = p;
      }
    }
    
    lastWinner = winner;
    let roundPoints = 0;
    for (const p of state.players) {
      if (p !== winner) roundPoints += sums[p];
    }
    
    if (state.mode === '101') {
      newScores = apply101Scoring(newScores, winner, roundPoints, state.players);
    } else {
      newScores[winner].permanent += roundPoints;
    }
  }

  return {
    ...state,
    turnIndex: newStatus === 'round_end' ? state.turnIndex : (state.turnIndex + 1) % state.players.length,
    consecutivePasses: newConsecutivePasses,
    status: newStatus,
    lastWinner,
    scores: newScores
  };
}

/**
 * Checks if the game has reached the winning score (101).
 */
export function checkGameOver(state: GameState): GameState {
  for (const p of state.players) {
    if (state.scores[p].permanent >= 101) {
      return { ...state, status: 'finished' };
    }
  }
  return state;
}

/**
 * Transitions from round_end to the next round, or finishes the game if someone won.
 */
export function nextRound(state: GameState): GameState {
  if (state.status !== 'round_end') throw new Error("Round has not ended yet");
  
  const checkedState = checkGameOver(state);
  if (checkedState.status === 'finished') return checkedState;
  
  return startRound({
    ...state,
    roundNumber: state.roundNumber + 1
  });
}
