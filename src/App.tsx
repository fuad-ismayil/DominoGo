import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Maximize, Minimize, Users, UserPlus, Play, History, LogIn, Plus, LogOut, X, Layers, Trash2, Copy, Check, Shield, MonitorOff, AlertTriangle, SmilePlus, Send, ArrowLeft, UserMinus, Clock, Trophy } from 'lucide-react';
import { io, Socket } from 'socket.io-client';
import { DndContext, useDraggable, useDroppable, DragEndEvent, DragStartEvent, TouchSensor, MouseSensor, useSensor, useSensors, DragOverlay } from '@dnd-kit/core';
import { CSS } from '@dnd-kit/utilities';
import { DominoTile } from './components/DominoTile';
import { canPlayTile } from './game/engine';
import { initAudio, playSound } from './utils/audio';

type FloatingReaction = { id: number; emoji: string; senderName: string; x: number };
const REACTIONS = ['👍', '👎', '😤', '🎉', '😂', '🔥', '💀', '🤝'];
let reactionIdCounter = 0;

type Screen = 'ENTRY' | 'LOGIN' | 'REGISTER' | 'MAIN_MENU' | 'CREATE_LOBBY' | 'JOIN_LOBBY' | 'LOBBY' | 'GAME' | 'UPGRADE_ACCOUNT' | 'FRIENDS' | 'LEADERBOARD';

type FriendInfo = { id: string; username: string; isOnline: boolean };
type FriendRequest = { id: number; userId: string; username: string };
type GameInviteToast = { id: number; lobbyId: string; senderUsername: string };
type PlayerStats = { wins: number; losses: number; gamesPlayed: number; winRate: number; totalScore: number; avgScore: number };
type LeaderboardEntry = { rank: number; userId: string; username: string; wins: number; losses: number; gamesPlayed: number; winRate: number; totalScore: number; avgScore: number };

function DraggableTile({ id, isPlayable, children }: { id: string, isPlayable: boolean, children: React.ReactNode }) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id,
    disabled: !isPlayable
  });

  const style = {
    opacity: isDragging ? 0.3 : 1,
  };

  return (
    <div ref={setNodeRef} style={style} {...listeners} {...attributes} className="touch-none cursor-grab active:cursor-grabbing">
      {children}
    </div>
  );
}

function DroppableZone({ id, isVisible, children, scale = 1 }: { id: string, isVisible: boolean, children: React.ReactNode, scale?: number }) {
  const { isOver, setNodeRef } = useDroppable({ id });

  if (!isVisible) return null;

  const getBaseWidth = () => {
    if (typeof window === 'undefined') return 16;
    const w = window.innerWidth;
    if (w >= 768) return 36;
    if (w >= 640) return 28;
    return 16;
  };

  const getBaseHeight = () => {
    if (typeof window === 'undefined') return 32;
    const w = window.innerWidth;
    if (w >= 768) return 72;
    if (w >= 640) return 56;
    return 32;
  };

  const width = getBaseWidth() * scale;
  const height = getBaseHeight() * scale;

  return (
    <div
      ref={setNodeRef}
      style={{ width: `${width}px`, height: `${height}px` }}
      className={`transition-all duration-200 ${isOver ? 'bg-amber-500/30 border-amber-400 scale-110' : 'bg-[#243a2c]/30 border-[#5a7054]'} border-2 border-dashed rounded-md sm:rounded-lg flex items-center justify-center shrink-0`}
    >
      {children}
    </div>
  );
}

export default function App() {
  const [currentScreen, setCurrentScreen] = useState<Screen>('ENTRY');
  const [username, setUsername] = useState('');
  const [userId, setUserId] = useState('');
  const [isGuest, setIsGuest] = useState(false);
  const [inputName, setInputName] = useState('');
  const [password, setPassword] = useState('');
  const [errorMsg, setErrorMsg] = useState('');
  const [isLoading, setIsLoading] = useState(true);

  const [isFullscreen, setIsFullscreen] = useState(false);
  const [lobbyId, setLobbyId] = useState('');
  const [lobbyPlayers, setLobbyPlayers] = useState<{ id: string, username: string }[]>([]);
  const [maxPlayers, setMaxPlayers] = useState(2);
  const [isHost, setIsHost] = useState(false);
  const [socket, setSocket] = useState<Socket | null>(null);
  const [gameState, setGameState] = useState<any>(null);
  const [copied, setCopied] = useState(false);

  const handleCopyLobbyId = () => {
    navigator.clipboard.writeText(lobbyId);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  const [activeDragId, setActiveDragId] = useState<string | null>(null);
  const [baseChunkSize, setBaseChunkSize] = useState(() => {
    const w = typeof window !== 'undefined' ? window.innerWidth : 800;
    const tileW = w >= 768 ? 76 : w >= 640 ? 60 : 36; // tile + gap (reduced)
    return Math.max(6, Math.floor((w - 32) / tileW));
  });
  const [matchHistory, setMatchHistory] = useState<any[]>([]);
  const [deletingMatchId, setDeletingMatchId] = useState<string | null>(null);

  // Reactions state
  const [showReactionPicker, setShowReactionPicker] = useState(false);
  const [floatingReactions, setFloatingReactions] = useState<FloatingReaction[]>([]);
  const reactionTimeoutRef = useRef<number | null>(null);

  // Guest expiry timer
  const [guestExpiresAt, setGuestExpiresAt] = useState<string | null>(null);
  const [guestCountdown, setGuestCountdown] = useState('');

  // Friend system state
  const [friendsList, setFriendsList] = useState<FriendInfo[]>([]);
  const [incomingRequests, setIncomingRequests] = useState<FriendRequest[]>([]);
  const [outgoingRequests, setOutgoingRequests] = useState<FriendRequest[]>([]);
  const [friendInput, setFriendInput] = useState('');
  const [friendError, setFriendError] = useState('');
  const [friendSuccess, setFriendSuccess] = useState('');
  const [showInviteModal, setShowInviteModal] = useState(false);
  const [gameInviteToasts, setGameInviteToasts] = useState<GameInviteToast[]>([]);
  let gameInviteIdCounter = useRef(0);

  // Player stats & leaderboard
  const [playerStats, setPlayerStats] = useState<PlayerStats | null>(null);
  const [leaderboard, setLeaderboard] = useState<LeaderboardEntry[]>([]);

  const fetchPlayerStats = async (uid?: string) => {
    const id = uid || userId;
    if (!id) return;
    try {
      const res = await fetch(`/api/stats/${id}`);
      if (res.ok) setPlayerStats(await res.json());
    } catch (e) { }
  };

  const fetchLeaderboard = async () => {
    try {
      const res = await fetch('/api/leaderboard');
      if (res.ok) {
        const data = await res.json();
        setLeaderboard(data.leaderboard || []);
      }
    } catch (e) { }
  };

  const fetchFriends = async () => {
    const token = localStorage.getItem('domino_token');
    if (!token) return;
    try {
      const res = await fetch('/api/friends', { headers: { 'Authorization': `Bearer ${token}` } });
      if (res.ok) {
        const data = await res.json();
        setFriendsList(data.friends || []);
        setIncomingRequests(data.incoming || []);
        setOutgoingRequests(data.outgoing || []);
        // Show pending game invites as toasts
        if (data.pendingGameInvites?.length) {
          setGameInviteToasts(prev => [
            ...prev,
            ...data.pendingGameInvites.filter((gi: any) => !prev.find(p => p.id === gi.id))
          ]);
        }
      }
    } catch (e) { }
  };

  useEffect(() => {
    if (currentScreen === 'MAIN_MENU') {
      const token = localStorage.getItem('domino_token');
      if (token) {
        fetch('/api/matches/history', {
          headers: { 'Authorization': `Bearer ${token}` }
        })
          .then(res => res.json())
          .then(data => {
            if (data.matches) {
              setMatchHistory(data.matches);
            }
          })
          .catch(console.error);
      }
    }
  }, [currentScreen]);

  useEffect(() => {
    const calcChunk = () => {
      const w = window.innerWidth;
      const tileW = w >= 768 ? 76 : w >= 640 ? 60 : 36;
      setBaseChunkSize(Math.max(6, Math.floor((w - 32) / tileW)));
    };
    calcChunk();
    window.addEventListener('resize', calcChunk);
    return () => window.removeEventListener('resize', calcChunk);
  }, []);

  const sensors = useSensors(
    useSensor(MouseSensor, {
      activationConstraint: { distance: 5 },
    }),
    useSensor(TouchSensor, {
      activationConstraint: { delay: 100, tolerance: 5 },
    })
  );

  const handleDragStart = (event: DragStartEvent) => {
    setActiveDragId(event.active.id as string);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setActiveDragId(null);
    const { active, over } = event;
    if (!over) return;

    const tileStr = active.id as string;
    const [t0, t1] = tileStr.split('-').map(Number);
    const tile: [number, number] = [t0, t1];
    const end = over.id as string; // 'left' | 'right' | 'north' | 'south'

    socket?.emit('play_tile', { lobbyId, tile, end }, (res: any) => {
      if (res.error) { playSound('error'); alert(res.error); }
    });
  };

  // Fullscreen toggle logic
  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(err => {
        console.error(`Error attempting to enable fullscreen: ${err.message}`);
      });
    } else {
      if (document.exitFullscreen) {
        document.exitFullscreen();
      }
    }
  };

  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(!!document.fullscreenElement);
    };
    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, []);

  // Audio trigger effect
  const prevGameStateRef = useRef<any>(null);
  useEffect(() => {
    const prev = prevGameStateRef.current;
    const state = gameState;
    if (prev && state && currentScreen === 'GAME') {
      if (state.board?.length > prev.board?.length) playSound('place');
      else if (state.marketCount < prev.marketCount) playSound('draw');

      if (state.status === 'round_end' && prev.status === 'playing') playSound(state.lastWinner === userId ? 'win' : 'lose');
      if (state.status === 'finished' && prev.status !== 'finished') playSound(state.lastWinner === userId ? 'win' : 'lose');
    }
    prevGameStateRef.current = state;
  }, [gameState, currentScreen, userId]);

  // Initialize audio system on mount
  useEffect(() => { initAudio(); }, []);

  // Check session on load
  useEffect(() => {
    const checkSession = async () => {
      const token = localStorage.getItem('domino_token');
      if (!token) {
        setIsLoading(false);
        return;
      }

      try {
        const res = await fetch('/api/auth/me', {
          headers: { 'Authorization': `Bearer ${token}` }
        });

        if (res.ok) {
          const data = await res.json();
          setUsername(data.user.username);
          setUserId(data.user.id);
          setIsGuest(data.user.isGuest);
          if (data.user.guestExpiresAt) setGuestExpiresAt(data.user.guestExpiresAt);
          setCurrentScreen('MAIN_MENU');
          fetchPlayerStats(data.user.id);
        } else {
          localStorage.removeItem('domino_token');
        }
      } catch (err) {
        console.error('Session check failed', err);
      } finally {
        setIsLoading(false);
      }
    };

    checkSession();
  }, []);

  // Guest expiry countdown timer
  useEffect(() => {
    if (!isGuest || !guestExpiresAt) {
      setGuestCountdown('');
      return;
    }

    const tick = () => {
      const now = Date.now();
      const expires = new Date(guestExpiresAt).getTime();
      const diff = Math.max(0, expires - now);

      if (diff === 0) {
        setGuestCountdown('Expired');
        return;
      }

      const hours = Math.floor(diff / (1000 * 60 * 60));
      const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
      const seconds = Math.floor((diff % (1000 * 60)) / 1000);
      setGuestCountdown(`${hours}h ${minutes.toString().padStart(2, '0')}m ${seconds.toString().padStart(2, '0')}s`);
    };

    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, [isGuest, guestExpiresAt]);

  // Socket initialization
  useEffect(() => {
    if (!username) return;

    const token = localStorage.getItem('domino_token');
    const newSocket = io({
      auth: { token }
    });

    newSocket.on('lobby_updated', (data) => {
      setLobbyPlayers(data.players);
    });

    newSocket.on('game_started', () => {
      setCurrentScreen('GAME');
    });

    newSocket.on('game_state_update', (state) => {
      setGameState((prev: any) => {
        // Evaluate sounds outside the functional update by using the current state value
        // But since we need prev safely, we can do it here but wrap playSound in a setTimeout
        // to push it to the macrotask queue, ensuring it only runs once per actual update?
        // Actually, a better way: functional update might run multiple times, 
        // but the socket event itself only fires once. 
        // We can capture the *current* gameState from a ref or just rely on the fact that 
        // functional updates shouldn't have side effects.
        return state;
      });

      // Since closures might have stale state, we can use a ref to track previous state for audio,
      // OR we can just rely on the setGameState to update the component, and use a standard useEffect 
      // block inside the component to watch for state.board / state.marketCount changes to play audio.
      setCurrentScreen('GAME');
    });

    // Reaction listener
    newSocket.on('reaction', ({ userId: senderId, emoji }) => {
      const senderName = senderId;
      const id = ++reactionIdCounter;
      const x = 20 + Math.random() * 60;
      setFloatingReactions(prev => [...prev, { id, emoji, senderName: senderId, x }]);
      setTimeout(() => {
        setFloatingReactions(prev => prev.filter(r => r.id !== id));
      }, 2500);
    });

    // Friend system socket listeners
    newSocket.on('friend_online', ({ userId: fid }) => {
      setFriendsList(prev => prev.map(f => f.id === fid ? { ...f, isOnline: true } : f));
    });
    newSocket.on('friend_offline', ({ userId: fid }) => {
      setFriendsList(prev => prev.map(f => f.id === fid ? { ...f, isOnline: false } : f));
    });
    newSocket.on('friend_request_received', () => { fetchFriends(); });
    newSocket.on('friend_request_accepted', () => { fetchFriends(); });
    newSocket.on('friend_removed', () => { fetchFriends(); });
    newSocket.on('game_invitation', (invite: GameInviteToast) => {
      setGameInviteToasts(prev => [...prev, invite]);
    });

    // Fetch friends on connect
    fetchFriends();

    setSocket(newSocket);

    return () => {
      newSocket.disconnect();
    };
  }, [username]);

  const handleGuestLogin = async () => {
    if (!inputName.trim()) return;
    setIsLoading(true);
    setErrorMsg('');

    try {
      const res = await fetch('/api/auth/guest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prefix: inputName.trim() })
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to login as guest');

      localStorage.setItem('domino_token', data.token);
      setUsername(data.user.username);
      setUserId(data.user.id);
      setIsGuest(data.user.isGuest);
      setCurrentScreen('MAIN_MENU');
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleRegister = async () => {
    if (!inputName.trim() || !password.trim()) return;
    setIsLoading(true);
    setErrorMsg('');

    try {
      const res = await fetch('/api/auth/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: inputName.trim(), password })
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to register');

      localStorage.setItem('domino_token', data.token);
      setUsername(data.user.username);
      setUserId(data.user.id);
      setIsGuest(data.user.isGuest);
      setCurrentScreen('MAIN_MENU');
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogin = async () => {
    if (!inputName.trim() || !password.trim()) return;
    setIsLoading(true);
    setErrorMsg('');

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: inputName.trim(), password })
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to login');

      localStorage.setItem('domino_token', data.token);
      setUsername(data.user.username);
      setUserId(data.user.id);
      setIsGuest(data.user.isGuest);
      setCurrentScreen('MAIN_MENU');
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    const token = localStorage.getItem('domino_token');
    if (token) {
      try {
        await fetch('/api/auth/logout', {
          method: 'POST',
          headers: { 'Authorization': `Bearer ${token}` }
        });
      } catch (err) {
        console.error('Logout failed', err);
      }
    }
    localStorage.removeItem('domino_token');
    setUsername('');
    setUserId('');
    setIsGuest(false);
    setInputName('');
    setPassword('');
    setCurrentScreen('ENTRY');
  };

  const handleLogoutOthers = async () => {
    const token = localStorage.getItem('domino_token');
    if (token) {
      try {
        await fetch('/api/auth/logout-others', {
          method: 'POST',
          headers: { 'Authorization': `Bearer ${token}` }
        });
        alert('Successfully logged out from all other devices.');
      } catch (err) {
        console.error('Logout others failed', err);
      }
    }
  };

  const handleUpgrade = async () => {
    if (password.length < 8) return;
    setIsLoading(true);
    setErrorMsg('');

    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch('/api/auth/upgrade', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
        body: JSON.stringify({ newUsername: inputName.trim(), password })
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to upgrade account');

      setUsername(data.user.username);
      setIsGuest(false);
      setCurrentScreen('MAIN_MENU');
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCreateLobby = async (players: number) => {
    setIsLoading(true);
    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch('/api/lobbies', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
        body: JSON.stringify({ maxPlayers: players })
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error);

      setLobbyId(data.id);
      setMaxPlayers(players);
      setLobbyPlayers([{ id: userId, username }]);
      setIsHost(true);

      socket?.emit('join_lobby', data.id);
      setCurrentScreen('LOBBY');
    } catch (err) {
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleDeleteMatch = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();

    if (deletingMatchId !== id) {
      setDeletingMatchId(id);
      return;
    }

    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch(`/api/matches/${id}`, {
        method: 'DELETE',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.ok) {
        setMatchHistory(prev => prev.filter(m => m.id !== id));
        setDeletingMatchId(null);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleJoinLobby = async (id: string) => {
    if (!id.trim()) return;
    setIsLoading(true);
    setErrorMsg('');
    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch(`/api/lobbies/${id}/join`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error);

      setLobbyId(id.toUpperCase());
      setMaxPlayers(data.maxPlayers);
      setLobbyPlayers(data.players);
      setIsHost(data.creatorId === userId);

      socket?.emit('join_lobby', id.toUpperCase());
      if (data.status === 'playing' || data.status === 'round_end' || data.status === 'finished') {
        setCurrentScreen('GAME');
      } else {
        setCurrentScreen('LOBBY');
      }
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLeaveLobby = async () => {
    try {
      const token = localStorage.getItem('domino_token');
      await fetch(`/api/lobbies/${lobbyId}/leave`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      socket?.emit('leave_lobby', lobbyId);
      setCurrentScreen('MAIN_MENU');
      setLobbyId('');
      setLobbyPlayers([]);
      setIsHost(false);
    } catch (err) {
      console.error(err);
    }
  };

  const handleAddBot = async () => {
    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch(`/api/lobbies/${lobbyId}/add-bot`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error);
      setLobbyPlayers(data.players);
    } catch (err) {
      console.error(err);
    }
  };

  const handleKickPlayer = async (targetUserId: string) => {
    try {
      const token = localStorage.getItem('domino_token');
      const res = await fetch(`/api/lobbies/${lobbyId}/kick`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ targetUserId })
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error);
      setLobbyPlayers(data.players);
    } catch (err) {
      console.error(err);
    }
  };

  const handleStartGame = async () => {
    try {
      const token = localStorage.getItem('domino_token');
      await fetch(`/api/lobbies/${lobbyId}/start`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      });
    } catch (err) {
      console.error(err);
    }
  };

  if (isLoading && currentScreen === 'ENTRY' && !username) {
    return (
      <div className="min-h-screen bg-[#0c1a12] flex items-center justify-center">
        <div className="w-12 h-12 border-4 border-amber-500 border-t-transparent rounded-full animate-spin"></div>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen bg-[#0c1a12] text-[#faf6ee] font-sans overflow-hidden flex flex-col relative selection:bg-amber-500/30 text-sm sm:text-base">
      <style>{`
        @media screen and (max-width: 768px) and (orientation: portrait) {
          .portrait-warning {
            display: flex !important;
          }
        }
      `}</style>

      {/* Orientation Warning */}
      <div className="portrait-warning fixed inset-0 z-[9999] bg-[#0f2318] flex-col items-center justify-center p-8 text-center hidden">
        <div className="w-20 h-20 border-4 border-[#2d4a38] rounded-3xl flex items-center justify-center mb-6 animate-pulse">
          <div className="w-12 h-16 border-2 border-zinc-500 rounded-xl relative">
            <div className="absolute top-1 left-1/2 -translate-x-1/2 w-4 h-1 bg-zinc-500 rounded-full"></div>
            <div className="absolute bottom-1 left-1/2 -translate-x-1/2 w-2 h-2 bg-zinc-500 rounded-full"></div>
          </div>
        </div>
        <h2 className="text-2xl font-bold text-white mb-2">Rotate Your Device</h2>
        <p className="text-[#a8b5a0]">Please rotate your phone to landscape mode to play DominoGo.</p>
      </div>

      {/* Top Bar - hidden on GAME and LOBBY screens */}
      {currentScreen !== 'GAME' && currentScreen !== 'LOBBY' && (
        <div className="absolute top-0 right-0 p-1 sm:p-2 z-50 pointer-events-none">
          <div className="pointer-events-auto">
            <button
              onClick={toggleFullscreen}
              className="p-1 sm:p-2 rounded-full bg-[#1a2e22]/30 hover:bg-[#243a2c]/50 text-[#7a8f74] hover:text-white transition-colors backdrop-blur-sm border border-[#2d4a38]/30"
            >
              {isFullscreen ? <Minimize size={14} className="sm:w-5 sm:h-5" /> : <Maximize size={14} className="sm:w-5 sm:h-5" />}
            </button>
          </div>
        </div>
      )}

      {/* Main Content Area */}
      <div className="flex-1 flex items-center justify-center p-2 sm:p-4 md:p-8 min-h-0">
        <AnimatePresence mode="wait">

          {currentScreen === 'ENTRY' && (
            <motion.div
              key="entry"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-[260px] sm:max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-2 sm:p-6 rounded-xl sm:rounded-2xl border border-[#2d4a38] shadow-2xl"
            >
              <h1 className="text-base sm:text-3xl font-bold mb-0.5 sm:mb-2 text-center">Welcome</h1>
              <p className="text-[#a8b5a0] text-center mb-2 sm:mb-8 text-[8px] sm:text-sm">Enter a username to get started</p>

              {errorMsg && (
                <div className="mb-2 p-1.5 bg-red-500/10 border border-red-500/20 text-red-400 rounded-lg text-[8px] sm:text-sm text-center">
                  {errorMsg}
                </div>
              )}

              <div className="space-y-1 sm:space-y-4">
                <div>
                  <input
                    type="text"
                    placeholder="Username"
                    value={inputName}
                    onChange={(e) => setInputName(e.target.value)}
                    className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                    onKeyDown={(e) => e.key === 'Enter' && handleGuestLogin()}
                    disabled={isLoading}
                  />
                </div>

                <button
                  onClick={handleGuestLogin}
                  disabled={!inputName.trim() || isLoading}
                  className="w-full bg-amber-600 hover:bg-amber-500 disabled:bg-[#243a2c] disabled:text-[#7a8f74] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                >
                  {isLoading ? <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div> : <LogIn size={11} className="sm:w-[18px] sm:h-[18px]" />}
                  Play as Guest
                </button>

                <div className="relative py-1 sm:py-4 flex items-center">
                  <div className="flex-grow border-t border-[#2d4a38]"></div>
                  <span className="flex-shrink-0 mx-2 text-[#7a8f74] text-[8px] uppercase tracking-wider">or</span>
                  <div className="flex-grow border-t border-[#2d4a38]"></div>
                </div>

                <div className="grid grid-cols-2 gap-1.5 sm:gap-3">
                  <button
                    onClick={() => { setErrorMsg(''); setCurrentScreen('LOGIN'); }}
                    className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                  >
                    Login
                  </button>
                  <button
                    onClick={() => { setErrorMsg(''); setCurrentScreen('REGISTER'); }}
                    className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                  >
                    Register
                  </button>
                </div>
              </div>
            </motion.div>
          )}

          {currentScreen === 'REGISTER' && (
            <motion.div
              key="register"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-[260px] sm:max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-2 sm:p-6 rounded-xl sm:rounded-2xl border border-[#2d4a38] shadow-2xl"
            >
              <h1 className="text-base sm:text-3xl font-bold mb-0.5 sm:mb-2 text-center">Create Account</h1>
              <p className="text-[#a8b5a0] text-center mb-2 sm:mb-8 text-[8px] sm:text-sm">Register to save your progress</p>

              {errorMsg && (
                <div className="mb-2 p-1.5 bg-red-500/10 border border-red-500/20 text-red-400 rounded-xl text-[8px] sm:text-sm text-center">
                  {errorMsg}
                </div>
              )}

              <div className="space-y-1 sm:space-y-4">
                <input
                  type="text"
                  placeholder="Username"
                  value={inputName}
                  onChange={(e) => setInputName(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  disabled={isLoading}
                />

                <input
                  type="password"
                  placeholder="Password (min 8 chars)"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  onKeyDown={(e) => e.key === 'Enter' && handleRegister()}
                  disabled={isLoading}
                />

                <button
                  onClick={handleRegister}
                  disabled={!inputName.trim() || password.length < 8 || isLoading}
                  className="w-full bg-amber-600 hover:bg-amber-500 disabled:bg-[#243a2c] disabled:text-[#7a8f74] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                >
                  {isLoading ? <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div> : <UserPlus size={11} className="sm:w-[18px] sm:h-[18px]" />}
                  Register
                </button>

                <button
                  onClick={() => { setErrorMsg(''); setCurrentScreen('ENTRY'); }}
                  className="w-full bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all"
                  disabled={isLoading}
                >
                  Back
                </button>
              </div>
            </motion.div>
          )}

          {currentScreen === 'LOGIN' && (
            <motion.div
              key="login"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-[260px] sm:max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-2 sm:p-6 rounded-xl sm:rounded-2xl border border-[#2d4a38] shadow-2xl"
            >
              <h1 className="text-base sm:text-3xl font-bold mb-0.5 sm:mb-2 text-center">Welcome Back</h1>
              <p className="text-[#a8b5a0] text-center mb-2 sm:mb-8 text-[8px] sm:text-sm">Login to your account</p>

              {errorMsg && (
                <div className="mb-2 p-1.5 bg-red-500/10 border border-red-500/20 text-red-400 rounded-xl text-[8px] sm:text-sm text-center">
                  {errorMsg}
                </div>
              )}

              <div className="space-y-1 sm:space-y-4">
                <input
                  type="text"
                  placeholder="Username"
                  value={inputName}
                  onChange={(e) => setInputName(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  disabled={isLoading}
                />

                <input
                  type="password"
                  placeholder="Password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  onKeyDown={(e) => e.key === 'Enter' && handleLogin()}
                  disabled={isLoading}
                />

                <button
                  onClick={handleLogin}
                  disabled={!inputName.trim() || !password.trim() || isLoading}
                  className="w-full bg-amber-600 hover:bg-amber-500 disabled:bg-[#243a2c] disabled:text-[#7a8f74] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                >
                  {isLoading ? <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div> : <LogIn size={11} className="sm:w-[18px] sm:h-[18px]" />}
                  Login
                </button>

                <button
                  onClick={() => { setErrorMsg(''); setCurrentScreen('ENTRY'); }}
                  className="w-full bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-2 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all"
                  disabled={isLoading}
                >
                  Back
                </button>
              </div>
            </motion.div>
          )}

          {currentScreen === 'UPGRADE_ACCOUNT' && (
            <motion.div
              key="upgrade_account"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-[260px] sm:max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-2 sm:p-6 rounded-xl sm:rounded-2xl border border-[#2d4a38] shadow-2xl"
            >
              <h1 className="text-base sm:text-3xl font-bold mb-0.5 sm:mb-2 text-center">Save Account</h1>
              <p className="text-[#a8b5a0] text-center mb-2 sm:mb-8 text-[8px] sm:text-sm">Set a password to save your progress permanently.</p>

              {errorMsg && (
                <div className="mb-2 p-1.5 bg-red-500/10 border border-red-500/20 text-red-400 rounded-xl text-[8px] sm:text-sm text-center">
                  {errorMsg}
                </div>
              )}

              <div className="space-y-1 sm:space-y-4">
                <input
                  type="text"
                  placeholder="New Username (Optional)"
                  value={inputName}
                  onChange={(e) => setInputName(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  disabled={isLoading}
                />

                <input
                  type="password"
                  placeholder="New Password (min 8 chars)"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-lg sm:rounded-xl px-2 py-1.5 sm:px-4 sm:py-3 text-[10px] sm:text-base text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all"
                  onKeyDown={(e) => e.key === 'Enter' && handleUpgrade()}
                  disabled={isLoading}
                />

                <button
                  onClick={handleUpgrade}
                  disabled={password.length < 8 || isLoading}
                  className="w-full bg-amber-600 hover:bg-amber-500 disabled:bg-[#243a2c] disabled:text-[#7a8f74] text-white font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all flex items-center justify-center gap-1 sm:gap-2"
                >
                  {isLoading ? <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div> : <Shield size={11} className="sm:w-[18px] sm:h-[18px]" />}
                  Upgrade Account
                </button>

                <button
                  onClick={() => { setErrorMsg(''); setCurrentScreen('MAIN_MENU'); }}
                  className="w-full bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-1.5 sm:py-3 text-[10px] sm:text-base rounded-lg sm:rounded-xl transition-all"
                  disabled={isLoading}
                >
                  Cancel
                </button>
              </div>
            </motion.div>
          )}

          {currentScreen === 'MAIN_MENU' && (
            <motion.div
              key="main_menu"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="w-full max-w-5xl h-full max-h-[90vh] md:max-h-[450px] grid grid-cols-2 gap-2 md:gap-4 items-stretch overflow-y-auto p-2 md:p-0"
            >
              <div className="bg-[#1a2e22]/90 backdrop-blur-md p-3 sm:p-4 md:p-6 rounded-2xl border border-[#2d4a38] flex flex-col justify-center h-full relative">
                <div className="absolute top-2 md:top-4 left-3 md:left-4 text-[10px] sm:text-sm font-bold tracking-tighter text-amber-400 opacity-50">
                  DominoGo
                </div>
                <div className="absolute top-2 md:top-4 right-2 md:right-4 flex gap-1.5 md:gap-2">
                  {!isGuest && (
                    <button
                      onClick={handleLogoutOthers}
                      className="p-1 sm:p-1.5 md:p-2 rounded-full bg-[#0f2318]/50 hover:bg-[#243a2c] text-[#7a8f74] hover:text-amber-400 transition-colors border border-[#2d4a38]"
                      title="Log out other devices"
                    >
                      <Layers size={12} className="sm:w-[14px] sm:h-[14px] md:w-5 md:h-5" />
                    </button>
                  )}
                  <button
                    onClick={handleLogout}
                    className="p-1 sm:p-1.5 md:p-2 rounded-full bg-[#0f2318]/50 hover:bg-[#243a2c] text-[#7a8f74] hover:text-red-400 transition-colors border border-[#2d4a38]"
                    title="Logout"
                  >
                    <LogOut size={12} className="sm:w-[14px] sm:h-[14px] md:w-5 md:h-5" />
                  </button>
                </div>
                <div className="mb-2 sm:mb-4 md:mb-6 mt-6 md:mt-0">
                  <div className="flex items-center gap-2 mb-0.5 sm:mb-1">
                    <h2 className="text-base sm:text-xl font-bold truncate">Hello, {username}</h2>
                    {isGuest && <span className="shrink-0 bg-[#243a2c] text-[#a8b5a0] text-[8px] sm:text-[10px] px-1.5 sm:px-2 py-0.5 rounded-full uppercase font-bold">Guest</span>}
                  </div>
                  {isGuest ? (
                    <button
                      onClick={() => { setInputName(username); setPassword(''); setErrorMsg(''); setCurrentScreen('UPGRADE_ACCOUNT'); }}
                      className="w-full mt-1 sm:mt-2 bg-amber-500/10 border border-amber-500/30 rounded-lg sm:rounded-xl p-2 sm:p-2.5 md:p-3 text-left hover:bg-amber-500/15 transition-colors group"
                    >
                      <div className="flex items-start gap-1.5 sm:gap-2">
                        <AlertTriangle size={14} className="text-amber-400 shrink-0 mt-0.5 sm:w-4 sm:h-4" />
                        <div>
                          <p className="text-amber-300 font-semibold text-[10px] sm:text-xs md:text-sm">Your progress is at risk!</p>
                          <p className="text-[#a8b5a0] text-[8px] sm:text-[9px] md:text-xs mt-0.5 leading-relaxed">Create an account to save your game history and log in from any device.</p>
                          <span className="inline-flex items-center gap-1 mt-0.5 sm:mt-1 text-amber-400 font-semibold text-[9px] sm:text-[10px] md:text-xs group-hover:text-amber-300 transition-colors">Create Account <span>→</span></span>
                        </div>
                      </div>
                    </button>
                  ) : (
                    <p className="text-[#a8b5a0] text-[10px] sm:text-xs md:text-sm">What would you like to play today?</p>
                  )}
                  {isGuest && guestCountdown && (
                    <div className="mt-1 sm:mt-1.5 flex items-center gap-1.5 text-[8px] sm:text-[9px] md:text-[11px] text-[#7a8f74]">
                      <div className="w-1.5 h-1.5 rounded-full bg-red-400/60 animate-pulse" />
                      <span>Guest account expires in <span className="font-mono font-semibold text-amber-400/80">{guestCountdown}</span></span>
                    </div>
                  )}
                  {playerStats && playerStats.gamesPlayed > 0 && (
                    <div className="mt-1 sm:mt-2 flex items-center gap-1.5 sm:gap-2 flex-wrap">
                      <span className="bg-green-500/15 text-green-400 text-[8px] sm:text-[9px] md:text-[10px] font-bold px-1.5 sm:px-2 py-0.5 rounded-md">{playerStats.wins}W</span>
                      <span className="bg-red-500/15 text-red-400 text-[8px] sm:text-[9px] md:text-[10px] font-bold px-1.5 sm:px-2 py-0.5 rounded-md">{playerStats.losses}L</span>
                      <span className="bg-amber-500/15 text-amber-400 text-[8px] sm:text-[9px] md:text-[10px] font-bold px-1.5 sm:px-2 py-0.5 rounded-md">{playerStats.winRate}%</span>
                      <span className="text-[8px] sm:text-[9px] text-[#5a7054]">{playerStats.gamesPlayed} games</span>
                    </div>
                  )}
                </div>

                <div className="space-y-1.5 sm:space-y-2 md:space-y-3">
                  <button
                    onClick={() => setCurrentScreen('CREATE_LOBBY')}
                    className="w-full bg-amber-600 hover:bg-amber-500 text-white font-semibold py-1.5 sm:py-2 md:py-3 rounded-lg sm:rounded-xl transition-all flex items-center justify-between px-3 sm:px-4 md:px-5 group"
                  >
                    <div className="flex items-center gap-2 sm:gap-3">
                      <Plus size={14} className="sm:w-4 sm:h-4 md:w-[18px] md:h-[18px]" />
                      <span className="text-xs sm:text-sm md:text-base">Create Lobby</span>
                    </div>
                    <span className="opacity-0 group-hover:opacity-100 transition-opacity text-amber-200 text-[9px] sm:text-[10px] md:text-xs">Host a game</span>
                  </button>

                  <button
                    onClick={() => {
                      setErrorMsg('');
                      setCurrentScreen('JOIN_LOBBY');
                    }}
                    className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-1.5 sm:py-2 md:py-3 rounded-lg sm:rounded-xl transition-all flex items-center justify-between px-3 sm:px-4 md:px-5"
                  >
                    <div className="flex items-center gap-2 sm:gap-3">
                      <Users size={14} className="sm:w-4 sm:h-4 md:w-[18px] md:h-[18px]" />
                      <span className="text-xs sm:text-sm md:text-base">Join Lobby</span>
                    </div>
                  </button>

                  {!isGuest && (
                    <button
                      onClick={() => { setFriendError(''); setFriendSuccess(''); fetchFriends(); setCurrentScreen('FRIENDS'); }}
                      className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-1.5 sm:py-2 md:py-3 rounded-lg sm:rounded-xl transition-all flex items-center justify-between px-3 sm:px-4 md:px-5"
                    >
                      <div className="flex items-center gap-2 sm:gap-3">
                        <UserPlus size={14} className="sm:w-4 sm:h-4 md:w-[18px] md:h-[18px]" />
                        <span className="text-xs sm:text-sm md:text-base">Friends</span>
                      </div>
                      {(incomingRequests.length > 0 || gameInviteToasts.length > 0) && (
                        <span className="bg-amber-500 text-white text-[9px] sm:text-[10px] font-bold rounded-full w-4 h-4 sm:w-5 sm:h-5 flex items-center justify-center">
                          {incomingRequests.length + gameInviteToasts.length}
                        </span>
                      )}
                    </button>
                  )}

                  <button
                    onClick={() => { fetchLeaderboard(); setCurrentScreen('LEADERBOARD'); }}
                    className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-1.5 sm:py-2 md:py-3 rounded-lg sm:rounded-xl transition-all flex items-center justify-between px-3 sm:px-4 md:px-5"
                  >
                    <div className="flex items-center gap-2 sm:gap-3">
                      <Trophy size={14} className="sm:w-4 sm:h-4 md:w-[18px] md:h-[18px]" />
                      <span className="text-xs sm:text-sm md:text-base">Leaderboard</span>
                    </div>
                  </button>
                </div>
              </div>

              <div className="bg-[#1a2e22]/90 backdrop-blur-md p-3 sm:p-4 md:p-6 rounded-2xl border border-[#2d4a38] flex flex-col h-full min-h-0">
                <div className="flex items-center gap-2 mb-4 text-[#a8b5a0] shrink-0">
                  <History size={18} />
                  <h3 className="font-semibold uppercase tracking-wider text-sm">Match History</h3>
                </div>

                {matchHistory.length === 0 ? (
                  <div className="flex-1 flex flex-col items-center justify-center text-[#7a8f74] space-y-2 min-h-0">
                    <div className="w-16 h-16 rounded-full border border-dashed border-[#2d4a38] flex items-center justify-center mb-2">
                      <History size={24} className="opacity-50" />
                    </div>
                    <p>No matches played yet.</p>
                    <p className="text-sm">Play a game to see your history here.</p>
                  </div>
                ) : (
                  <div className="flex-1 overflow-y-auto space-y-1.5 pr-2 custom-scrollbar min-h-0">
                    {matchHistory.map((match) => {
                      const playerNames = match.players.map((p: any) => p.username).join(' vs ');
                      const isFinished = match.status === 'finished';
                      const isPlaying = match.status === 'playing' || match.status === 'round_end';
                      const dateStr = new Date(match.updated_at || match.created_at).toLocaleString(undefined, {
                        month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit'
                      });

                      return (
                        <div
                          key={match.id}
                          className={`bg-[#243a2c]/50 hover:bg-[#243a2c] border ${isFinished ? 'border-[#2d4a38]/30' : 'border-amber-500/20'} rounded-lg p-2 flex flex-col gap-1 cursor-pointer transition-colors`}
                          onClick={() => {
                            handleJoinLobby(match.id);
                          }}
                        >
                          <div className="flex items-start justify-between gap-2">
                            <div className="flex-1 min-w-0">
                              <div className="font-medium text-[#d4ddd0] truncate text-sm" title={playerNames}>
                                {playerNames || `Lobby: ${match.id}`}
                              </div>
                              <div className="text-[10px] text-[#7a8f74] mt-0.5 flex items-center gap-2">
                                <span className={`px-2 py-0.5 rounded-md ${isFinished ? 'bg-[#2d4a38] text-[#d4ddd0]' :
                                  isPlaying ? 'bg-amber-500/20 text-amber-400' :
                                    'bg-amber-500/20 text-amber-400'
                                  }`}>
                                  {isFinished ? 'Finished' : isPlaying ? 'In Progress' : 'Waiting'}
                                </span>
                                <span className="font-mono uppercase opacity-50">#{match.id}</span>
                              </div>
                            </div>

                            <div className="flex flex-col items-end gap-1">
                              <div className="flex items-center gap-2">
                                <div className="text-[10px] text-[#7a8f74] whitespace-nowrap uppercase tracking-wider font-semibold">
                                  {isFinished ? 'Ended' : 'Last Played'}
                                </div>
                                {deletingMatchId === match.id ? (
                                  <div className="flex items-center gap-1">
                                    <button
                                      onClick={(e) => handleDeleteMatch(e, match.id)}
                                      className="text-xs bg-red-500/20 text-red-400 hover:bg-red-500/30 px-2 py-0.5 rounded transition-colors"
                                    >
                                      Confirm
                                    </button>
                                    <button
                                      onClick={(e) => { e.stopPropagation(); setDeletingMatchId(null); }}
                                      className="text-xs bg-[#2d4a38] text-[#d4ddd0] hover:bg-[#3d5e48] px-2 py-0.5 rounded transition-colors"
                                    >
                                      Cancel
                                    </button>
                                  </div>
                                ) : (
                                  <button
                                    onClick={(e) => handleDeleteMatch(e, match.id)}
                                    className="text-[#7a8f74] hover:text-red-400 transition-colors p-1"
                                    title="Remove from history"
                                  >
                                    <X size={14} />
                                  </button>
                                )}
                              </div>
                              <div className="text-xs text-[#a8b5a0] whitespace-nowrap">
                                {dateStr}
                              </div>
                              {!isFinished && (
                                <div className="bg-amber-500/10 text-amber-400 p-1.5 rounded-lg mt-1">
                                  <Play size={14} />
                                </div>
                              )}
                            </div>
                          </div>

                          {match.scores && (
                            <div className="pt-1 border-t border-[#2d4a38]/50 grid grid-cols-2 gap-1">
                              {match.players.map((p: any) => {
                                const score = match.scores[p.id]?.permanent || 0;
                                const isWinner = match.winner === p.id;
                                return (
                                  <div key={p.id} className={`text-[10px] flex justify-between items-center p-1 rounded-md ${isWinner ? 'bg-amber-500/10 text-amber-400 font-bold' : 'text-[#a8b5a0]'}`}>
                                    <span className="truncate pr-2">{p.username}</span>
                                    <span>{score}</span>
                                  </div>
                                );
                              })}
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </motion.div>
          )}

          {currentScreen === 'FRIENDS' && (
            <motion.div
              key="friends"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="w-full max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-4 sm:p-6 rounded-2xl border border-[#2d4a38] flex flex-col max-h-[85vh]"
            >
              <div className="flex items-center justify-between mb-4 shrink-0">
                <div className="flex items-center gap-2">
                  <button onClick={() => setCurrentScreen('MAIN_MENU')} className="p-1.5 rounded-lg hover:bg-[#243a2c] text-[#7a8f74] hover:text-white transition-colors">
                    <ArrowLeft size={18} />
                  </button>
                  <h2 className="text-lg font-bold">Friends</h2>
                </div>
                <span className="text-[10px] text-[#7a8f74] uppercase tracking-wider">
                  {friendsList.length} friend{friendsList.length !== 1 ? 's' : ''}
                </span>
              </div>

              <div className="flex-1 overflow-y-auto space-y-4 pr-1 custom-scrollbar">
                {/* Send Friend Request */}
                <div>
                  <h3 className="text-[10px] uppercase tracking-wider text-[#7a8f74] font-bold mb-2">Add Friend</h3>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={friendInput}
                      onChange={e => { setFriendInput(e.target.value); setFriendError(''); setFriendSuccess(''); }}
                      placeholder="Enter username..."
                      className="flex-1 bg-[#0f2318] border border-[#2d4a38] rounded-lg px-3 py-2 text-sm text-white placeholder-[#5a7054] focus:border-amber-500 outline-none"
                      onKeyDown={e => {
                        if (e.key === 'Enter') {
                          e.preventDefault();
                          (async () => {
                            if (!friendInput.trim()) return;
                            setFriendError(''); setFriendSuccess('');
                            const token = localStorage.getItem('domino_token');
                            const res = await fetch('/api/friends/request', {
                              method: 'POST', headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
                              body: JSON.stringify({ username: friendInput.trim() })
                            });
                            const data = await res.json();
                            if (res.ok) { setFriendInput(''); setFriendSuccess('Request sent!'); fetchFriends(); }
                            else setFriendError(data.error || 'Failed');
                          })();
                        }
                      }}
                    />
                    <button
                      onClick={async () => {
                        if (!friendInput.trim()) return;
                        setFriendError(''); setFriendSuccess('');
                        const token = localStorage.getItem('domino_token');
                        const res = await fetch('/api/friends/request', {
                          method: 'POST', headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
                          body: JSON.stringify({ username: friendInput.trim() })
                        });
                        const data = await res.json();
                        if (res.ok) { setFriendInput(''); setFriendSuccess('Request sent!'); fetchFriends(); }
                        else setFriendError(data.error || 'Failed');
                      }}
                      className="bg-amber-600 hover:bg-amber-500 text-white px-3 rounded-lg transition-colors"
                    >
                      <Send size={16} />
                    </button>
                  </div>
                  {friendError && <p className="text-red-400 text-[10px] mt-1">{friendError}</p>}
                  {friendSuccess && <p className="text-green-400 text-[10px] mt-1">{friendSuccess}</p>}
                </div>

                {/* Incoming Requests */}
                {incomingRequests.length > 0 && (
                  <div>
                    <h3 className="text-[10px] uppercase tracking-wider text-[#7a8f74] font-bold mb-2">Incoming Requests</h3>
                    <div className="space-y-1.5">
                      {incomingRequests.map(req => (
                        <div key={req.id} className="flex items-center justify-between bg-[#243a2c]/50 rounded-lg px-3 py-2 border border-amber-500/20">
                          <span className="text-sm font-medium">{req.username}</span>
                          <div className="flex gap-1.5">
                            <button
                              onClick={async () => {
                                const token = localStorage.getItem('domino_token');
                                await fetch('/api/friends/accept', {
                                  method: 'POST', headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
                                  body: JSON.stringify({ friendshipId: req.id })
                                });
                                fetchFriends();
                              }}
                              className="bg-green-600 hover:bg-green-500 text-white text-[10px] font-bold px-2.5 py-1 rounded-md transition-colors"
                            >
                              Accept
                            </button>
                            <button
                              onClick={async () => {
                                const token = localStorage.getItem('domino_token');
                                await fetch('/api/friends/reject', {
                                  method: 'POST', headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
                                  body: JSON.stringify({ friendshipId: req.id })
                                });
                                fetchFriends();
                              }}
                              className="bg-red-600/30 hover:bg-red-600/50 text-red-300 text-[10px] font-bold px-2.5 py-1 rounded-md transition-colors"
                            >
                              Reject
                            </button>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Outgoing Requests */}
                {outgoingRequests.length > 0 && (
                  <div>
                    <h3 className="text-[10px] uppercase tracking-wider text-[#7a8f74] font-bold mb-2">Sent Requests</h3>
                    <div className="space-y-1.5">
                      {outgoingRequests.map(req => (
                        <div key={req.id} className="flex items-center justify-between bg-[#243a2c]/30 rounded-lg px-3 py-2 border border-[#2d4a38]/50">
                          <div className="flex items-center gap-2">
                            <span className="text-sm font-medium text-[#a8b5a0]">{req.username}</span>
                            <span className="flex items-center gap-1 text-[9px] text-amber-400/60 bg-amber-500/10 px-1.5 py-0.5 rounded-md">
                              <Clock size={8} /> Waiting
                            </span>
                          </div>
                          <button
                            onClick={async () => {
                              const token = localStorage.getItem('domino_token');
                              await fetch('/api/friends/cancel', {
                                method: 'POST', headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
                                body: JSON.stringify({ friendshipId: req.id })
                              });
                              fetchFriends();
                            }}
                            className="text-[#7a8f74] hover:text-red-400 transition-colors p-1"
                            title="Cancel request"
                          >
                            <X size={14} />
                          </button>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Friends List */}
                <div>
                  <h3 className="text-[10px] uppercase tracking-wider text-[#7a8f74] font-bold mb-2">Friends</h3>
                  {friendsList.length === 0 ? (
                    <div className="text-center py-6 text-[#5a7054] text-sm">
                      <UserPlus size={24} className="mx-auto mb-2 opacity-50" />
                      <p>No friends yet</p>
                      <p className="text-[10px] mt-0.5">Add friends by username above</p>
                    </div>
                  ) : (
                    <div className="space-y-1.5">
                      {friendsList.sort((a, b) => (b.isOnline ? 1 : 0) - (a.isOnline ? 1 : 0)).map(friend => (
                        <div key={friend.id} className="flex items-center justify-between bg-[#243a2c]/30 rounded-lg px-3 py-2 border border-[#2d4a38]/50">
                          <div className="flex items-center gap-2">
                            <div className={`w-2 h-2 rounded-full ${friend.isOnline ? 'bg-green-400' : 'bg-[#5a7054]'}`} />
                            <span className="text-sm font-medium">{friend.username}</span>
                            <span className={`text-[9px] ${friend.isOnline ? 'text-green-400' : 'text-[#5a7054]'}`}>
                              {friend.isOnline ? 'Online' : 'Offline'}
                            </span>
                          </div>
                          <button
                            onClick={async () => {
                              if (!confirm(`Remove ${friend.username} from friends?`)) return;
                              const token = localStorage.getItem('domino_token');
                              await fetch(`/api/friends/${friend.id}`, {
                                method: 'DELETE', headers: { 'Authorization': `Bearer ${token}` }
                              });
                              fetchFriends();
                            }}
                            className="text-[#7a8f74] hover:text-red-400 transition-colors p-1"
                            title="Remove friend"
                          >
                            <UserMinus size={14} />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </motion.div>
          )}

          {currentScreen === 'LEADERBOARD' && (
            <motion.div
              key="leaderboard"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="w-full max-w-lg bg-[#1a2e22]/90 backdrop-blur-md p-4 sm:p-6 rounded-2xl border border-[#2d4a38] flex flex-col max-h-[85vh]"
            >
              <div className="flex items-center justify-between mb-4 shrink-0">
                <div className="flex items-center gap-2">
                  <button onClick={() => setCurrentScreen('MAIN_MENU')} className="p-1.5 rounded-lg hover:bg-[#243a2c] text-[#7a8f74] hover:text-white transition-colors">
                    <ArrowLeft size={18} />
                  </button>
                  <Trophy size={18} className="text-amber-400" />
                  <h2 className="text-lg font-bold">Leaderboard</h2>
                </div>
                <span className="text-[10px] text-[#7a8f74] uppercase tracking-wider">Top 50</span>
              </div>

              <div className="flex-1 overflow-y-auto custom-scrollbar">
                {leaderboard.length === 0 ? (
                  <div className="text-center py-8 text-[#5a7054]">
                    <Trophy size={32} className="mx-auto mb-3 opacity-30" />
                    <p className="text-sm">No games played yet</p>
                    <p className="text-[10px] mt-1">Play matches to appear here!</p>
                  </div>
                ) : (
                  <table className="w-full text-[10px] sm:text-xs">
                    <thead>
                      <tr className="text-[#7a8f74] uppercase tracking-wider border-b border-[#2d4a38]">
                        <th className="text-left py-2 px-1 font-semibold">#</th>
                        <th className="text-left py-2 px-1 font-semibold">Player</th>
                        <th className="text-center py-2 px-1 font-semibold">W</th>
                        <th className="text-center py-2 px-1 font-semibold">L</th>
                        <th className="text-center py-2 px-1 font-semibold">Win%</th>
                        <th className="text-center py-2 px-1 font-semibold">Avg</th>
                        <th className="text-center py-2 px-1 font-semibold">GP</th>
                      </tr>
                    </thead>
                    <tbody>
                      {leaderboard.map(entry => {
                        const isMe = entry.userId === userId;
                        return (
                          <tr
                            key={entry.userId}
                            className={`border-b border-[#2d4a38]/30 transition-colors ${isMe ? 'bg-amber-500/10' : 'hover:bg-[#243a2c]/30'}`}
                          >
                            <td className={`py-2 px-1 font-bold ${entry.rank <= 3 ? 'text-amber-400' : 'text-[#7a8f74]'}`}>
                              {entry.rank <= 3 ? ['🥇', '🥈', '🥉'][entry.rank - 1] : entry.rank}
                            </td>
                            <td className={`py-2 px-1 font-medium truncate max-w-[120px] ${isMe ? 'text-amber-400' : 'text-white'}`}>
                              {entry.username}{isMe && <span className="text-[8px] ml-1 text-amber-400/60">(you)</span>}
                            </td>
                            <td className="text-center py-2 px-1 text-green-400 font-semibold">{entry.wins}</td>
                            <td className="text-center py-2 px-1 text-red-400/80">{entry.losses}</td>
                            <td className="text-center py-2 px-1 text-amber-300 font-semibold">{entry.winRate}%</td>
                            <td className="text-center py-2 px-1 text-[#a8b5a0]">{entry.avgScore}</td>
                            <td className="text-center py-2 px-1 text-[#7a8f74]">{entry.gamesPlayed}</td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                )}
              </div>
            </motion.div>
          )}

          {currentScreen === 'CREATE_LOBBY' && (
            <motion.div
              key="create_lobby"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-6 rounded-2xl border border-[#2d4a38]"
            >
              <h2 className="text-2xl font-bold mb-6 text-center">Create Lobby</h2>

              <div className="space-y-6">
                <div>
                  <label className="block text-sm font-medium text-[#a8b5a0] mb-3">Player Count</label>
                  <div className="grid grid-cols-3 gap-3">
                    {[2, 3, 4].map(num => (
                      <button
                        key={num}
                        onClick={() => handleCreateLobby(num)}
                        className="bg-[#243a2c] hover:bg-amber-600/20 hover:border-amber-500/50 border border-[#2d4a38] py-4 rounded-xl transition-all flex flex-col items-center gap-2"
                      >
                        <Users size={24} className={num === 2 ? "text-[#d4ddd0]" : "text-[#a8b5a0]"} />
                        <span className="font-semibold">{num} Players</span>
                      </button>
                    ))}
                  </div>
                </div>

                <button
                  onClick={() => setCurrentScreen('MAIN_MENU')}
                  className="w-full bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-3 rounded-xl transition-all"
                >
                  Cancel
                </button>
              </div>
            </motion.div>
          )}

          {currentScreen === 'JOIN_LOBBY' && (
            <motion.div
              key="join_lobby"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="w-full max-w-md bg-[#1a2e22]/90 backdrop-blur-md p-6 rounded-2xl border border-[#2d4a38]"
            >
              <h2 className="text-2xl font-bold mb-6 text-center">Join Lobby</h2>

              {errorMsg && (
                <div className="bg-red-500/10 border border-red-500/50 text-red-400 p-3 rounded-xl text-sm text-center mb-6">
                  {errorMsg}
                </div>
              )}

              <div className="space-y-4">
                <div>
                  <input
                    type="text"
                    placeholder="Enter 7-character Lobby ID"
                    className="w-full bg-[#0f2318] border border-[#2d4a38] rounded-xl px-4 py-3 text-white placeholder:text-[#5a7054] focus:outline-none focus:ring-2 focus:ring-amber-500/50 transition-all uppercase text-center tracking-widest font-mono"
                    maxLength={7}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        handleJoinLobby((e.target as HTMLInputElement).value);
                      }
                    }}
                  />
                </div>

                <button
                  onClick={() => {
                    const input = document.querySelector('input') as HTMLInputElement;
                    handleJoinLobby(input.value);
                  }}
                  className="w-full bg-amber-600 hover:bg-amber-500 text-white font-semibold py-3 rounded-xl transition-all"
                >
                  Join Game
                </button>

                <button
                  onClick={() => {
                    setErrorMsg('');
                    setCurrentScreen('MAIN_MENU');
                  }}
                  className="w-full bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-3 rounded-xl transition-all"
                >
                  Cancel
                </button>
              </div>
            </motion.div>
          )}

          {currentScreen === 'LOBBY' && (
            <motion.div
              key="lobby"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="w-full max-w-lg sm:max-w-2xl bg-[#1a2e22]/90 backdrop-blur-md p-3 sm:p-5 rounded-2xl border border-[#2d4a38] flex flex-col"
            >
              <div className="flex justify-between items-center mb-3 sm:mb-4">
                <div>
                  <h2 className="text-base sm:text-xl font-bold text-white">Waiting Room</h2>
                  <p className="text-[#7a8f74] text-[10px] sm:text-xs">Mode: 101 • {maxPlayers} Players</p>
                </div>
                <div className="bg-[#0f2318] border border-[#2d4a38] px-2.5 py-1 sm:px-3 sm:py-1.5 rounded-lg flex flex-col items-center relative group">
                  <span className="text-[8px] sm:text-[10px] text-[#7a8f74] uppercase tracking-wider font-semibold">Code</span>
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-sm sm:text-lg tracking-widest text-amber-400 font-bold">{lobbyId}</span>
                    <button
                      onClick={handleCopyLobbyId}
                      className="p-1 rounded-md hover:bg-[#243a2c] text-[#7a8f74] hover:text-amber-400 transition-colors"
                      title="Copy Lobby ID"
                    >
                      {copied ? <Check size={14} className="text-amber-500" /> : <Copy size={14} />}
                    </button>
                  </div>
                </div>
              </div>

              <div className={`grid gap-2 sm:gap-3 mb-3 sm:mb-4 ${maxPlayers <= 2 ? 'grid-cols-2' : maxPlayers === 3 ? 'grid-cols-3' : 'grid-cols-4'}`}>
                {Array.from({ length: maxPlayers }).map((_, idx) => {
                  const player = lobbyPlayers[idx];
                  return (
                    <div
                      key={idx}
                      className={`relative rounded-xl border flex flex-col items-center justify-center p-2 sm:p-3 min-h-[80px] sm:min-h-[100px] transition-all ${player
                        ? 'border-amber-500/30 bg-amber-500/5'
                        : 'border-dashed border-[#2d4a38] bg-[#1a2e22]/50'
                        }`}
                    >
                      {player ? (
                        <>
                          <div className="w-7 h-7 sm:w-9 sm:h-9 rounded-full bg-[#243a2c] flex items-center justify-center mb-1.5">
                            <Users size={12} className="text-amber-400 sm:w-4 sm:h-4" />
                          </div>
                          <span className="font-semibold text-[10px] sm:text-xs text-center truncate w-full">{player.username}</span>
                          {idx === 0 && <span className="text-[7px] sm:text-[8px] uppercase tracking-wider text-amber-400 font-bold mt-0.5">Host</span>}
                          {isHost && idx !== 0 && (
                            <button
                              onClick={() => handleKickPlayer(player.id)}
                              className="absolute top-1 right-1 p-0.5 text-[#5a7054] hover:text-red-400 transition-colors"
                              title="Kick"
                            >
                              <X size={12} />
                            </button>
                          )}
                        </>
                      ) : (
                        <>
                          <span className="text-[#5a7054] font-medium text-[10px] sm:text-xs mb-2">Empty</span>
                          <div className="flex flex-col gap-1 w-full">
                            <button onClick={() => { fetchFriends(); setShowInviteModal(true); }} className="bg-[#243a2c] hover:bg-[#2d4a38] text-[8px] sm:text-[10px] font-semibold py-1 rounded-md transition-colors w-full">
                              Invite
                            </button>
                            {isHost && (
                              <button
                                onClick={handleAddBot}
                                className="bg-amber-600/20 hover:bg-amber-600/30 text-amber-400 text-[8px] sm:text-[10px] font-semibold py-1 rounded-md transition-colors w-full"
                              >
                                Add Bot
                              </button>
                            )}
                          </div>
                        </>
                      )}
                    </div>
                  );
                })}
              </div>

              <div className="flex justify-between items-center pt-2 sm:pt-3 border-t border-[#2d4a38]">
                <button
                  onClick={handleLeaveLobby}
                  className="bg-transparent hover:bg-[#243a2c] text-[#a8b5a0] font-semibold py-1.5 px-3 sm:py-2 sm:px-5 rounded-lg transition-all text-[10px] sm:text-xs"
                >
                  Leave
                </button>

                {isHost ? (
                  <button
                    disabled={lobbyPlayers.length < maxPlayers}
                    onClick={handleStartGame}
                    className="bg-amber-600 hover:bg-amber-500 disabled:bg-[#243a2c] disabled:text-[#7a8f74] text-white font-semibold py-1.5 px-4 sm:py-2 sm:px-6 rounded-lg transition-all flex items-center gap-1 sm:gap-1.5 text-[10px] sm:text-xs"
                  >
                    <Play size={12} className="sm:w-3.5 sm:h-3.5" />
                    Start Game
                  </button>
                ) : (
                  <div className="text-[#7a8f74] text-[10px] sm:text-xs font-medium px-2">
                    Waiting for host...
                  </div>
                )}
              </div>
            </motion.div>
          )}

          {currentScreen === 'GAME' && (
            <motion.div
              key="game"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="w-full h-full flex flex-col"
            >
              {!gameState ? (
                <div className="flex-1 flex flex-col items-center justify-center">
                  <div className="w-24 h-24 border-4 border-amber-500 border-t-transparent rounded-full animate-spin mx-auto mb-6"></div>
                  <h2 className="text-2xl font-bold mb-2">Game Starting...</h2>
                  <p className="text-[#a8b5a0]">Loading domino tiles and board</p>
                </div>
              ) : (
                <DndContext sensors={sensors} onDragStart={handleDragStart} onDragEnd={handleDragEnd}>
                  {/* Floating top-right controls: Fullscreen + Exit */}
                  <div className="absolute top-2 right-2 sm:top-3 sm:right-3 z-50 flex flex-col gap-1.5 pointer-events-auto">
                    <button
                      onClick={toggleFullscreen}
                      className="p-1.5 sm:p-2 rounded-lg bg-[#1a2e22]/80 hover:bg-[#243a2c] text-[#a8b5a0] hover:text-white transition-colors backdrop-blur-sm border border-[#2d4a38]/50"
                    >
                      {isFullscreen ? <Minimize size={14} className="sm:w-4 sm:h-4" /> : <Maximize size={14} className="sm:w-4 sm:h-4" />}
                    </button>
                    <button
                      onClick={() => {
                        socket?.emit('leave_lobby', lobbyId);
                        setCurrentScreen('MAIN_MENU');
                        setLobbyId('');
                        setGameState(null);
                      }}
                      className="p-1.5 sm:p-2 rounded-lg bg-[#1a2e22]/80 hover:bg-red-900/50 text-[#a8b5a0] hover:text-red-400 transition-colors backdrop-blur-sm border border-[#2d4a38]/50"
                      title="Exit Match"
                    >
                      <X size={14} className="sm:w-4 sm:h-4" />
                    </button>
                    {/* Reaction picker button */}
                    <button
                      onClick={() => setShowReactionPicker(prev => !prev)}
                      className="p-1.5 sm:p-2 rounded-lg bg-[#1a2e22]/80 hover:bg-[#243a2c] text-[#a8b5a0] hover:text-white transition-colors backdrop-blur-sm border border-[#2d4a38]/50"
                      title="Send Reaction"
                    >
                      <SmilePlus size={14} className="sm:w-4 sm:h-4" />
                    </button>
                  </div>

                  {/* Reaction picker dropdown */}
                  <AnimatePresence>
                    {showReactionPicker && (
                      <motion.div
                        initial={{ opacity: 0, scale: 0.8, y: -10 }}
                        animate={{ opacity: 1, scale: 1, y: 0 }}
                        exit={{ opacity: 0, scale: 0.8, y: -10 }}
                        transition={{ duration: 0.15 }}
                        className="absolute top-14 right-2 sm:top-[4.5rem] sm:right-3 z-50 bg-[#1a2e22]/95 backdrop-blur-md border border-[#2d4a38] rounded-xl p-1.5 sm:p-2 shadow-2xl pointer-events-auto"
                      >
                        <div className="grid grid-cols-4 gap-1">
                          {REACTIONS.map(emoji => (
                            <button
                              key={emoji}
                              onClick={() => {
                                socket?.emit('send_reaction', { lobbyId, emoji });
                                setShowReactionPicker(false);
                              }}
                              className="w-8 h-8 sm:w-10 sm:h-10 flex items-center justify-center rounded-lg hover:bg-[#243a2c] active:scale-90 transition-all text-base sm:text-xl"
                            >
                              {emoji}
                            </button>
                          ))}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>

                  {/* Floating reactions overlay */}
                  <div className="absolute inset-0 z-[60] pointer-events-none overflow-hidden">
                    <AnimatePresence>
                      {floatingReactions.map(r => (
                        <motion.div
                          key={r.id}
                          initial={{ opacity: 1, y: '70vh', scale: 0.5 }}
                          animate={{ opacity: 0, y: '10vh', scale: 1.3 }}
                          exit={{ opacity: 0 }}
                          transition={{ duration: 2.2, ease: 'easeOut' }}
                          style={{ left: `${r.x}%` }}
                          className="absolute flex flex-col items-center"
                        >
                          <span className="text-3xl sm:text-5xl drop-shadow-lg">{r.emoji}</span>
                          <span className="text-[8px] sm:text-[10px] text-white/70 font-semibold mt-0.5 bg-black/30 px-1.5 py-0.5 rounded-full backdrop-blur-sm whitespace-nowrap">
                            {lobbyPlayers.find(lp => lp.id === r.senderName)?.username || r.senderName}
                          </span>
                        </motion.div>
                      ))}
                    </AnimatePresence>
                  </div>

                  {/* Opponent info capsules - positioned dynamically */}
                  {(() => {
                    const opponents = gameState.players.filter((p: string) => p !== userId);
                    const opponentCapsule = (p: string, posClass: string) => (
                      <div key={p} className={`absolute z-40 ${posClass}`}>
                        <div className="flex items-center gap-1.5 bg-[#1a2e22]/80 backdrop-blur-sm border border-[#2d4a38]/50 rounded-full px-2 py-0.5 sm:px-3 sm:py-1">
                          <span className={`text-[9px] sm:text-xs font-semibold truncate max-w-[50px] sm:max-w-[80px] transition-colors ${gameState.players[gameState.turnIndex] === p ? 'text-amber-400' : 'text-[#d4ddd0]'}`}>{lobbyPlayers.find(lp => lp.id === p)?.username || p}</span>
                          <span className="text-[8px] sm:text-[10px] text-[#7a8f74]">{gameState.opponentHandCounts[p]}🀰</span>
                          <span className="text-[8px] sm:text-[10px] text-amber-400 font-sans font-semibold">{gameState.scores[p]?.permanent || 0}/101</span>
                        </div>
                      </div>
                    );

                    if (opponents.length === 1) {
                      return opponentCapsule(opponents[0], 'top-2 left-1/2 -translate-x-1/2 sm:top-3');
                    } else if (opponents.length === 2) {
                      return (
                        <>
                          {opponentCapsule(opponents[0], 'top-2 left-2 sm:top-3 sm:left-4')}
                          {opponentCapsule(opponents[1], 'top-2 right-12 sm:top-3 sm:right-16')}
                        </>
                      );
                    } else if (opponents.length === 3) {
                      return (
                        <>
                          {opponentCapsule(opponents[0], 'top-2 left-2 sm:top-3 sm:left-4')}
                          {opponentCapsule(opponents[1], 'top-2 left-1/2 -translate-x-1/2 sm:top-3')}
                          {opponentCapsule(opponents[2], 'top-2 right-12 sm:top-3 sm:right-16')}
                        </>
                      );
                    }
                    return null;
                  })()}

                  {/* Left-middle market draw icon - hidden in 4-player mode */}
                  {maxPlayers < 4 && (
                    <div className="absolute left-2 top-1/2 -translate-y-1/2 z-40 sm:left-3">
                      <button
                        onClick={() => socket?.emit('draw_market', { lobbyId }, (res: any) => { if (res.error) { playSound('error'); alert(res.error); } })}
                        disabled={gameState.marketCount === 0}
                        className={`flex flex-col items-center gap-0.5 p-1.5 sm:p-2 rounded-lg backdrop-blur-sm border transition-colors ${gameState.marketCount > 0
                          ? 'bg-[#1a2e22]/80 border-[#2d4a38]/50 text-[#d4ddd0] hover:bg-[#243a2c] hover:text-white cursor-pointer'
                          : 'bg-[#1a2e22]/30 border-[#2d4a38]/20 text-[#5a7054] cursor-not-allowed'
                          }`}
                        title={gameState.marketCount > 0 ? `Draw (${gameState.marketCount})` : 'Market empty'}
                      >
                        <Layers size={14} className="sm:w-4 sm:h-4" />
                        <span className="text-[8px] sm:text-[10px] font-mono font-bold">{gameState.marketCount}</span>
                      </button>
                    </div>
                  )}

                  {/* Bottom-left: current user info */}
                  <div className="absolute bottom-2 left-2 z-40 sm:bottom-3 sm:left-3">
                    <div className="flex items-center gap-1.5 bg-[#1a2e22]/80 backdrop-blur-sm border border-[#2d4a38]/50 rounded-full px-2 py-0.5 sm:px-3 sm:py-1">
                      <span className={`text-[9px] sm:text-xs font-semibold transition-colors ${gameState.players[gameState.turnIndex] === userId ? 'text-amber-400' : 'text-[#d4ddd0]'}`}>{username}</span>
                      <span className="text-[8px] sm:text-[10px] text-amber-400 font-sans font-semibold">
                        {gameState.scores[userId]?.permanent || 0}{gameState.scores[userId]?.temporary > 0 && `+${gameState.scores[userId].temporary}`}/101
                      </span>
                    </div>
                  </div>

                  {/* Board Area - full screen, no chrome */}
                  <div className="flex-1 flex flex-col items-center justify-center px-1 py-1 overflow-hidden relative w-full">
                    {gameState.status === 'finished' && (
                      <div className="fixed inset-0 bg-[#0c1a12]/95 z-[100] flex flex-col items-center justify-center backdrop-blur-md p-2 sm:p-4">
                        <h2 className="text-base sm:text-2xl font-black text-amber-400 mb-0.5 sm:mb-1 uppercase tracking-widest">Match Over!</h2>
                        <p className="text-[#d4ddd0] mb-2 sm:mb-4 text-[10px] sm:text-sm">Winner: <span className="font-bold text-white text-xs sm:text-base ml-0.5 sm:ml-1">{lobbyPlayers.find(lp => lp.id === gameState.lastWinner)?.username || gameState.lastWinner}</span></p>

                        <div className="bg-[#1a2e22] border border-[#2d4a38] rounded-lg sm:rounded-xl p-2 sm:p-4 mb-2 sm:mb-4 min-w-[160px] sm:min-w-[250px] max-h-[35vh] overflow-y-auto shadow-2xl">
                          <h3 className="text-[8px] sm:text-xs font-bold text-[#7a8f74] uppercase tracking-wider mb-1 sm:mb-2 text-center">Final Scores</h3>
                          {gameState.players.map((p: string) => (
                            <div key={p} className="flex justify-between items-center py-1 sm:py-2 border-b border-[#2d4a38]/50 last:border-0 gap-4">
                              <span className={p === gameState.lastWinner ? 'text-amber-400 font-bold text-[10px] sm:text-sm' : 'text-[#d4ddd0] text-[10px] sm:text-sm'}>
                                {lobbyPlayers.find(lp => lp.id === p)?.username || p}
                              </span>
                              <span className="font-sans text-white text-xs sm:text-base font-semibold">
                                {gameState.scores[p]?.permanent || 0}
                              </span>
                            </div>
                          ))}
                        </div>

                        <div className="flex items-center gap-2 sm:gap-3 z-[101]">
                          {isHost && (
                            <button
                              onClick={() => socket?.emit('rematch', { lobbyId }, (res: any) => { if (res.error) alert(res.error); })}
                              className="bg-amber-600 hover:bg-amber-500 text-white font-bold py-1.5 px-4 sm:py-2.5 sm:px-6 rounded-lg sm:rounded-xl text-xs sm:text-sm transition-colors shadow-lg shadow-amber-900/20"
                            >
                              Rematch
                            </button>
                          )}
                          <button
                            onClick={() => {
                              socket?.emit('leave_lobby', lobbyId);
                              setCurrentScreen('MAIN_MENU');
                              setLobbyId('');
                              setGameState(null);
                            }}
                            className="bg-[#243a2c] hover:bg-[#2d4a38] text-white font-bold py-1.5 px-4 sm:py-2.5 sm:px-6 rounded-lg sm:rounded-xl text-xs sm:text-sm transition-colors"
                          >
                            Return to Menu
                          </button>
                        </div>
                        {!isHost && (
                          <p className="text-[#7a8f74] text-[9px] sm:text-xs mt-1.5">Host can start a rematch</p>
                        )}
                      </div>
                    )}

                    {gameState.status === 'round_end' && (
                      <div className="absolute inset-0 bg-[#0c1a12]/90 z-50 flex flex-col items-center justify-center backdrop-blur-sm p-2 sm:p-4">
                        <h2 className="text-xl sm:text-3xl font-bold text-amber-400 mb-0.5 sm:mb-1">Round Over!</h2>
                        <p className="text-[#d4ddd0] mb-2 sm:mb-4 text-xs sm:text-lg">Winner: <span className="font-bold text-white">{lobbyPlayers.find(lp => lp.id === gameState.lastWinner)?.username || gameState.lastWinner}</span></p>

                        <div className="bg-[#1a2e22] border border-[#2d4a38] rounded-xl p-2 sm:p-4 mb-2 sm:mb-4 min-w-[180px] sm:min-w-[250px] max-h-[40vh] overflow-y-auto">
                          <h3 className="text-[10px] sm:text-sm font-bold text-[#7a8f74] uppercase tracking-wider mb-1 sm:mb-2 text-center">Current Scores</h3>
                          {gameState.players.map((p: string) => (
                            <div key={p} className="flex justify-between items-center py-1 sm:py-2 border-b border-[#2d4a38]/50 last:border-0">
                              <span className={p === gameState.lastWinner ? 'text-amber-400 font-bold text-xs sm:text-base' : 'text-[#d4ddd0] text-xs sm:text-base'}>
                                {lobbyPlayers.find(lp => lp.id === p)?.username || p}
                              </span>
                              <span className="font-sans text-white font-semibold text-xs sm:text-base">
                                {gameState.scores[p]?.permanent || 0}
                                {gameState.scores[p]?.temporary > 0 && <span className="text-[#7a8f74] text-[10px] sm:text-sm ml-1 font-sans">(+{gameState.scores[p].temporary})</span>}
                              </span>
                            </div>
                          ))}
                        </div>

                        {isHost && (
                          <button
                            onClick={() => socket?.emit('next_round', { lobbyId }, (res: any) => res.error && alert(res.error))}
                            className="bg-amber-600 hover:bg-amber-500 text-white font-bold py-2 px-6 sm:py-3 sm:px-10 rounded-xl text-sm sm:text-base shadow-lg shadow-amber-900/20 z-[60] mt-2"
                          >
                            Next Round
                          </button>
                        )}
                      </div>
                    )}



                    <div className="flex-1 w-full overflow-y-auto overflow-x-hidden flex flex-col items-center justify-center min-h-[80px] sm:min-h-[160px] px-0.5 py-1">
                      {(() => {
                        const sequence: any[] = [];
                        if (gameState.players[gameState.turnIndex] === userId) {
                          sequence.push({ type: 'drop-left' });
                        }
                        gameState.board.forEach((tile: [number, number], idx: number) => {
                          sequence.push({ type: 'tile', tile, idx });
                        });
                        if (gameState.board.length > 0 && gameState.players[gameState.turnIndex] === userId) {
                          sequence.push({ type: 'drop-right' });
                        }

                        if (sequence.length === 0) {
                          return <div className="text-[#5a7054] italic font-medium mx-4">Waiting for opponent to start...</div>;
                        }

                        // Dynamic scaling and chunking - STRICT 2 ROW LIMIT
                        const totalItems = sequence.length;
                        let currentChunkSize = baseChunkSize;
                        let boardScale = 1.0;

                        // Max capacity in 2 rows + 1 corner is (currentChunkSize * 2) + 1
                        if (totalItems > (baseChunkSize * 2) + 1) {
                          // Calculate required chunk size to fit everything in 2 rows
                          // totalItems = 2 * requiredChunkSize + 1
                          const requiredChunkSize = Math.ceil((totalItems - 1) / 2);
                          boardScale = Math.max(0.4, baseChunkSize / requiredChunkSize);
                          currentChunkSize = requiredChunkSize;
                        }

                        const layout: any[] = [];
                        let i = 0;
                        let isLTR = true;

                        // Row 1
                        const row1Items = sequence.slice(i, i + currentChunkSize);
                        layout.push({ type: 'row', isLTR, items: row1Items });
                        i += currentChunkSize;

                        // Corner 1
                        if (i < sequence.length) {
                          const corner1Item = sequence[i];
                          layout.push({ type: 'corner', isLTR, item: corner1Item });
                          i += 1;
                          isLTR = !isLTR;

                          // Row 2 - take all remaining items to ensure no 3rd row
                          const row2Items = sequence.slice(i);
                          layout.push({ type: 'row', isLTR, items: row2Items });
                          i = sequence.length;
                        }

                        return (
                          <div className="flex flex-col items-stretch w-fit mx-auto" style={{ gap: `${4 * boardScale}px` }}>
                            {layout.map((block, blockIdx) => {
                              if (block.type === 'row') {
                                const displayItems = block.isLTR ? block.items : [...block.items].reverse();
                                return (
                                  <div
                                    key={`row-${blockIdx}`}
                                    className={`flex items-center flex-row ${block.isLTR ? 'justify-start' : 'justify-end'}`}
                                    style={{ gap: `${4 * boardScale}px` }}
                                  >
                                    {displayItems.map((item: any, itemIndex: number) => {
                                      if (item.type === 'drop-left') {
                                        return (
                                          <div key="left">
                                            <DroppableZone id="left" isVisible={true} scale={boardScale}>
                                              <span className="text-[#7a8f74] font-bold text-[6px] sm:text-[10px] md:text-xs tracking-tighter sm:tracking-normal">LEFT</span>
                                            </DroppableZone>
                                          </div>
                                        );
                                      }
                                      if (item.type === 'drop-right') {
                                        return (
                                          <div key="right">
                                            <DroppableZone id="right" isVisible={true} scale={boardScale}>
                                              <span className="text-[#7a8f74] font-bold text-[6px] sm:text-[10px] md:text-xs tracking-tighter sm:tracking-normal">RIGHT</span>
                                            </DroppableZone>
                                          </div>
                                        );
                                      }
                                      const isDouble = item.tile[0] === item.tile[1];
                                      const displayValue = block.isLTR ? item.tile : [item.tile[1], item.tile[0]];
                                      return (
                                        <DominoTile
                                          key={`tile-${item.idx}`}
                                          value={displayValue as [number, number]}
                                          horizontal={!isDouble}
                                          isDouble={isDouble}
                                          scale={boardScale}
                                        />
                                      );
                                    })}
                                  </div>
                                );
                              } else {
                                const item = block.item;
                                return (
                                  <div key={`corner-${blockIdx}`} className={`w-full flex ${block.isLTR ? 'justify-end' : 'justify-start'}`}>
                                    {(() => {
                                      if (item.type === 'drop-left') {
                                        return (
                                          <div key="left">
                                            <DroppableZone id="left" isVisible={true} scale={boardScale}>
                                              <span className="text-[#7a8f74] font-bold text-[6px] sm:text-[10px] md:text-xs tracking-tighter sm:tracking-normal">LEFT</span>
                                            </DroppableZone>
                                          </div>
                                        );
                                      }
                                      if (item.type === 'drop-right') {
                                        return (
                                          <div key="right">
                                            <DroppableZone id="right" isVisible={true} scale={boardScale}>
                                              <span className="text-[#7a8f74] font-bold text-[6px] sm:text-[10px] md:text-xs tracking-tighter sm:tracking-normal">RIGHT</span>
                                            </DroppableZone>
                                          </div>
                                        );
                                      }
                                      const isDouble = item.tile[0] === item.tile[1];
                                      return (
                                        <DominoTile
                                          key={`tile-${item.idx}`}
                                          value={item.tile as [number, number]}
                                          horizontal={false}
                                          isDouble={isDouble}
                                          scale={boardScale}
                                        />
                                      );
                                    })()}
                                  </div>
                                );
                              }
                            })}
                          </div>
                        );
                      })()}
                    </div>

                    {/* 5-lik: North/South drop zones when spinner exists */}
                  </div>

                  {/* Bottom: Player Hand + Pass button - no background bar */}
                  <div className="px-1 py-0 sm:px-2 sm:py-0.5 flex items-end justify-center gap-1 shrink-0 w-full relative mb-0.5">
                    <button
                      onClick={() => socket?.emit('pass_turn', { lobbyId }, (res: any) => { if (res.error) { playSound('error'); alert(res.error); } })}
                      disabled={gameState.marketCount > 0 || gameState.players[gameState.turnIndex] !== userId}
                      className="bg-[#243a2c]/70 hover:bg-[#2d4a38] text-[#a8b5a0] hover:text-white px-1.5 py-0.5 sm:px-2 sm:py-1 rounded-md text-[8px] sm:text-xs font-semibold transition-colors backdrop-blur-sm border border-[#2d4a38]/50 shrink-0 mb-0.5 disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                      Pass
                    </button>
                    <div className={`flex flex-wrap justify-center gap-0.5 overflow-y-auto max-h-[18vh] sm:max-h-[30vh] pb-0.5 transition-opacity ${gameState.players[gameState.turnIndex] === userId ? '' : 'opacity-40 pointer-events-none'}`}>
                      {(() => {
                        let requiredFirstTile: [number, number] | null = null;
                        if (gameState.board.length === 0 && gameState.roundNumber === 1) {
                          const doubles = [[1, 1], [2, 2], [3, 3], [4, 4], [5, 5], [6, 6], [0, 0]];
                          for (const d of doubles) {
                            if (gameState.hands[userId]?.some((t: [number, number]) => t[0] === d[0] && t[1] === d[1])) {
                              requiredFirstTile = d as [number, number];
                              break;
                            }
                          }
                        }

                        return gameState.hands[userId]?.map((tile: [number, number], idx: number) => {
                          const isMyTurn = gameState.players[gameState.turnIndex] === userId;
                          let canPlayLeft = false;
                          let canPlayRight = false;

                          if (isMyTurn) {
                            if (gameState.board.length === 0) {
                              if (gameState.roundNumber === 1) {
                                if (requiredFirstTile) {
                                  canPlayLeft = tile[0] === requiredFirstTile[0] && tile[1] === requiredFirstTile[1];
                                } else {
                                  canPlayLeft = tile[0] === tile[1];
                                }
                              } else {
                                canPlayLeft = true;
                              }
                            } else {
                              canPlayLeft = tile[0] === gameState.openEnds[0] || tile[1] === gameState.openEnds[0];
                              canPlayRight = tile[0] === gameState.openEnds[1] || tile[1] === gameState.openEnds[1];
                            }
                          }

                          const isPlayable = canPlayLeft || canPlayRight;
                          const tileId = `${tile[0]}-${tile[1]}`;

                          return (
                            <div key={idx} className="flex flex-col items-center">
                              <DraggableTile id={tileId} isPlayable={isPlayable}>
                                <DominoTile
                                  value={tile}
                                  horizontal={false}
                                  isPlayable={isPlayable}
                                  disabled={isMyTurn && !isPlayable}
                                />
                              </DraggableTile>
                            </div>
                          );
                        });
                      })()}
                    </div>
                  </div>
                  <DragOverlay dropAnimation={null} zIndex={1000}>
                    {activeDragId ? (
                      <DominoTile
                        value={activeDragId.split('-').map(Number) as [number, number]}
                        horizontal={false}
                        isPlayable={true}
                        className="shadow-2xl scale-110 rotate-3 z-[1000] relative"
                      />
                    ) : null}
                  </DragOverlay>
                </DndContext>
              )}
            </motion.div>
          )}

        </AnimatePresence>

        {/* Invite Friend Modal */}
        {showInviteModal && (
          <div className="fixed inset-0 bg-black/60 z-[90] flex items-center justify-center p-4" onClick={() => setShowInviteModal(false)}>
            <motion.div
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              className="bg-[#1a2e22] border border-[#2d4a38] rounded-2xl p-4 sm:p-6 max-w-sm w-full max-h-[70vh] flex flex-col"
              onClick={e => e.stopPropagation()}
            >
              <div className="flex items-center justify-between mb-4 shrink-0">
                <h3 className="text-base font-bold">Invite Friend</h3>
                <button onClick={() => setShowInviteModal(false)} className="text-[#7a8f74] hover:text-white transition-colors p-1">
                  <X size={18} />
                </button>
              </div>

              <div className="flex-1 overflow-y-auto space-y-1.5">
                {friendsList.length === 0 ? (
                  <div className="text-center py-6 text-[#5a7054] text-sm">
                    <Users size={24} className="mx-auto mb-2 opacity-50" />
                    <p>No friends to invite</p>
                    <p className="text-[10px] mt-0.5">Add friends from the Friends menu</p>
                  </div>
                ) : (
                  friendsList.sort((a, b) => (b.isOnline ? 1 : 0) - (a.isOnline ? 1 : 0)).map(friend => {
                    const isInLobby = lobbyPlayers.some(p => p.id === friend.id);
                    return (
                      <div key={friend.id} className="flex items-center justify-between bg-[#243a2c]/30 rounded-lg px-3 py-2.5 border border-[#2d4a38]/50">
                        <div className="flex items-center gap-2">
                          <div className={`w-2 h-2 rounded-full ${friend.isOnline ? 'bg-green-400' : 'bg-[#5a7054]'}`} />
                          <span className="text-sm font-medium">{friend.username}</span>
                          <span className={`text-[9px] ${friend.isOnline ? 'text-green-400' : 'text-[#5a7054]'}`}>
                            {friend.isOnline ? 'Online' : 'Offline'}
                          </span>
                        </div>
                        {isInLobby ? (
                          <span className="text-[9px] text-green-400 bg-green-500/10 px-2 py-0.5 rounded-md">In Lobby</span>
                        ) : (
                          <button
                            onClick={() => {
                              socket?.emit('invite_to_lobby', { lobbyId, friendUserId: friend.id }, (res: any) => {
                                if (res.error) alert(res.error);
                                else setShowInviteModal(false);
                              });
                            }}
                            className="bg-amber-600 hover:bg-amber-500 text-white text-[10px] font-bold px-3 py-1 rounded-md transition-colors"
                          >
                            Invite
                          </button>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </motion.div>
          </div>
        )}

        {/* Game Invitation Toasts */}
        {gameInviteToasts.length > 0 && (
          <div className="fixed top-4 left-1/2 -translate-x-1/2 z-[110] flex flex-col gap-2 w-[90vw] max-w-sm">
            {gameInviteToasts.map(toast => (
              <motion.div
                key={toast.id}
                initial={{ opacity: 0, y: -20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -20, scale: 0.95 }}
                className="bg-[#1a2e22] border border-amber-500/30 rounded-xl p-3 shadow-2xl shadow-black/50"
              >
                <p className="text-sm text-white mb-2"><span className="font-bold text-amber-400">{toast.senderUsername}</span> invited you to a game!</p>
                <div className="flex gap-2">
                  <button
                    onClick={() => {
                      socket?.emit('accept_game_invite', { inviteId: toast.id }, (res: any) => {
                        if (res.error) { alert(res.error); return; }
                        setGameInviteToasts(prev => prev.filter(t => t.id !== toast.id));
                        setLobbyId(res.lobbyId);
                        setLobbyPlayers(res.players);
                        setMaxPlayers(res.maxPlayers);
                        setIsHost(res.creatorId === userId);
                        socket?.emit('join_lobby', res.lobbyId);
                        setCurrentScreen('LOBBY');
                      });
                    }}
                    className="flex-1 bg-green-600 hover:bg-green-500 text-white text-xs font-bold py-1.5 rounded-lg transition-colors"
                  >
                    Accept
                  </button>
                  <button
                    onClick={() => {
                      socket?.emit('reject_game_invite', { inviteId: toast.id }, () => { });
                      setGameInviteToasts(prev => prev.filter(t => t.id !== toast.id));
                    }}
                    className="flex-1 bg-red-600/30 hover:bg-red-600/50 text-red-300 text-xs font-bold py-1.5 rounded-lg transition-colors"
                  >
                    Reject
                  </button>
                </div>
              </motion.div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

