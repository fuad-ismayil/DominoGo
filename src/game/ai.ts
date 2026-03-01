import { GameState, Tile } from './engine';

export type AIAction = 
  | { type: 'play', tile: Tile, end: 'left' | 'right' }
  | { type: 'draw' }
  | { type: 'pass' };

export function getAIMove(state: GameState, aiId: string): AIAction {
  const hand = state.hands[aiId];
  const openEnds = state.openEnds;
  
  // First move logic
  if (state.board.length === 0 && state.roundNumber === 1) {
    if (state.mode === '101') {
      const doubles = [[1,1], [2,2], [3,3], [4,4], [5,5], [6,6], [0,0]];
      for (const d of doubles) {
        if (hand.some(t => t[0] === d[0] && t[1] === d[1])) {
          return { type: 'play', tile: d as Tile, end: 'left' };
        }
      }
      // If no double, maybe draw?
    } else if (state.mode === '5-lik') {
      const startTile = hand.find(t => (t[0] === 2 && t[1] === 3) || (t[0] === 3 && t[1] === 2));
      if (startTile) return { type: 'play', tile: startTile, end: 'left' };
    }
  }

  // Find all playable moves
  const validMoves: { tile: Tile, end: 'left' | 'right', score: number }[] = [];
  
  for (const tile of hand) {
    if (state.board.length === 0) {
       validMoves.push({ tile, end: 'left', score: tile[0] + tile[1] });
    } else {
       if (tile[0] === openEnds[0] || tile[1] === openEnds[0]) {
         validMoves.push({ tile, end: 'left', score: tile[0] + tile[1] });
       }
       // If it can be played on both sides, we add both options
       if (tile[0] === openEnds[1] || tile[1] === openEnds[1]) {
         validMoves.push({ tile, end: 'right', score: tile[0] + tile[1] });
       }
    }
  }

  if (validMoves.length > 0) {
    // Basic strategy: play the tile with the highest pip count to minimize points if round is lost
    validMoves.sort((a, b) => b.score - a.score);
    return { type: 'play', tile: validMoves[0].tile, end: validMoves[0].end };
  }

  // If no valid moves, try to draw
  const drawCount = state.players.length === 2 ? 2 : (state.players.length === 3 ? 1 : 0);
  if (drawCount > 0 && state.market.length > 0) {
    return { type: 'draw' };
  }

  // Otherwise pass
  return { type: 'pass' };
}
