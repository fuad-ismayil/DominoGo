import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Maximize, Minimize, Users, UserPlus, Play, History, LogIn, Plus, LogOut, X, Layers, Trash2, Copy, Check, Shield, MonitorOff, AlertTriangle } from 'lucide-react';
import { io, Socket } from 'socket.io-client';
import { DndContext, useDraggable, useDroppable, DragEndEvent, DragStartEvent, TouchSensor, MouseSensor, useSensor, useSensors, DragOverlay } from '@dnd-kit/core';
import { CSS } from '@dnd-kit/utilities';
import { DominoTile } from './components/DominoTile';
import { canPlayTile } from './game/engine';

type Screen = 'ENTRY' | 'LOGIN' | 'REGISTER' | 'MAIN_MENU' | 'CREATE_LOBBY' | 'JOIN_LOBBY' | 'LOBBY' | 'GAME' | 'UPGRADE_ACCOUNT';

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
      if (res.error) alert(res.error);
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
          setCurrentScreen('MAIN_MENU');
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
      setGameState(state);
      setCurrentScreen('GAME');
    });

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
              className="w-full max-w-5xl h-full max-h-[450px] grid grid-cols-2 gap-4 items-stretch"
            >
              <div className="bg-[#1a2e22]/90 backdrop-blur-md p-4 sm:p-6 rounded-2xl border border-[#2d4a38] flex flex-col justify-center h-full relative">
                <div className="absolute top-4 left-4 text-xs sm:text-sm font-bold tracking-tighter text-amber-400 opacity-50">
                  DominoGo
                </div>
                <div className="absolute top-4 right-4 flex gap-2">
                  {!isGuest && (
                    <button
                      onClick={handleLogoutOthers}
                      className="p-1.5 sm:p-2 rounded-full bg-[#0f2318]/50 hover:bg-[#243a2c] text-[#7a8f74] hover:text-amber-400 transition-colors border border-[#2d4a38]"
                      title="Log out other devices"
                    >
                      <Layers size={14} className="sm:w-5 sm:h-5" />
                    </button>
                  )}
                  <button
                    onClick={handleLogout}
                    className="p-1.5 sm:p-2 rounded-full bg-[#0f2318]/50 hover:bg-[#243a2c] text-[#7a8f74] hover:text-red-400 transition-colors border border-[#2d4a38]"
                    title="Logout"
                  >
                    <LogOut size={14} className="sm:w-5 sm:h-5" />
                  </button>
                </div>
                <div className="mb-4 sm:mb-6">
                  <div className="flex items-center gap-2 mb-1">
                    <h2 className="text-xl font-bold truncate">Hello, {username}</h2>
                    {isGuest && <span className="shrink-0 bg-[#243a2c] text-[#a8b5a0] text-[10px] px-2 py-0.5 rounded-full uppercase font-bold">Guest</span>}
                  </div>
                  {isGuest ? (
                    <button
                      onClick={() => { setInputName(username); setPassword(''); setErrorMsg(''); setCurrentScreen('UPGRADE_ACCOUNT'); }}
                      className="w-full mt-2 bg-amber-500/10 border border-amber-500/30 rounded-xl p-2.5 sm:p-3 text-left hover:bg-amber-500/15 transition-colors group"
                    >
                      <div className="flex items-start gap-2">
                        <AlertTriangle size={16} className="text-amber-400 shrink-0 mt-0.5" />
                        <div>
                          <p className="text-amber-300 font-semibold text-xs sm:text-sm">Your progress is at risk!</p>
                          <p className="text-[#a8b5a0] text-[9px] sm:text-xs mt-0.5 leading-relaxed">Create an account to save your game history, pick a clean username, and log in from any device.</p>
                          <span className="inline-flex items-center gap-1 mt-1 text-amber-400 font-semibold text-[10px] sm:text-xs group-hover:text-amber-300 transition-colors">Create Account <span>→</span></span>
                        </div>
                      </div>
                    </button>
                  ) : (
                    <p className="text-[#a8b5a0] text-xs sm:text-sm">What would you like to play today?</p>
                  )}
                </div>

                <div className="space-y-2 sm:space-y-3">
                  <button
                    onClick={() => setCurrentScreen('CREATE_LOBBY')}
                    className="w-full bg-amber-600 hover:bg-amber-500 text-white font-semibold py-2.5 sm:py-3 rounded-xl transition-all flex items-center justify-between px-4 sm:px-5 group"
                  >
                    <div className="flex items-center gap-2 sm:gap-3">
                      <Plus size={16} className="sm:w-[18px] sm:h-[18px]" />
                      <span className="text-sm sm:text-base">Create Lobby</span>
                    </div>
                    <span className="opacity-0 group-hover:opacity-100 transition-opacity text-amber-200 text-[10px] sm:text-xs">Host a game</span>
                  </button>

                  <button
                    onClick={() => {
                      setErrorMsg('');
                      setCurrentScreen('JOIN_LOBBY');
                    }}
                    className="w-full bg-[#243a2c] hover:bg-[#2d4a38] text-white font-semibold py-2.5 sm:py-3 rounded-xl transition-all flex items-center justify-between px-4 sm:px-5"
                  >
                    <div className="flex items-center gap-2 sm:gap-3">
                      <Users size={16} className="sm:w-[18px] sm:h-[18px]" />
                      <span className="text-sm sm:text-base">Join Lobby</span>
                    </div>
                  </button>
                </div>
              </div>

              <div className="bg-[#1a2e22]/90 backdrop-blur-md p-4 sm:p-6 rounded-2xl border border-[#2d4a38] flex flex-col h-full min-h-0">
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
                            <button className="bg-[#243a2c] hover:bg-[#2d4a38] text-[8px] sm:text-[10px] font-semibold py-1 rounded-md transition-colors w-full">
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
                        onClick={() => socket?.emit('draw_market', { lobbyId }, (res: any) => res.error && alert(res.error))}
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
                      <div className="absolute inset-0 bg-[#0c1a12]/90 z-20 flex flex-col items-center justify-center backdrop-blur-md p-4">
                        <h2 className="text-2xl sm:text-4xl font-black text-amber-400 mb-1 sm:mb-2 uppercase tracking-widest">Match Over!</h2>
                        <p className="text-[#d4ddd0] mb-4 sm:mb-8 text-base sm:text-xl">Ultimate Winner: <span className="font-bold text-white text-lg sm:text-2xl ml-1 sm:ml-2">{lobbyPlayers.find(lp => lp.id === gameState.lastWinner)?.username || gameState.lastWinner}</span></p>

                        <div className="bg-[#1a2e22] border border-[#2d4a38] rounded-xl p-3 sm:p-6 mb-4 sm:mb-8 min-w-[220px] sm:min-w-[300px] shadow-2xl">
                          <h3 className="text-xs sm:text-sm font-bold text-[#7a8f74] uppercase tracking-wider mb-2 sm:mb-4 text-center">Final Scores</h3>
                          {gameState.players.map((p: string) => (
                            <div key={p} className="flex justify-between items-center py-2 sm:py-3 border-b border-[#2d4a38]/50 last:border-0">
                              <span className={p === gameState.lastWinner ? 'text-amber-400 font-bold text-sm sm:text-lg' : 'text-[#d4ddd0] text-sm sm:text-lg'}>
                                {lobbyPlayers.find(lp => lp.id === p)?.username || p}
                              </span>
                              <span className="font-sans text-white text-base sm:text-xl font-semibold">
                                {gameState.scores[p]?.permanent || 0}
                              </span>
                            </div>
                          ))}
                        </div>

                        <button
                          onClick={() => {
                            socket?.emit('leave_lobby', lobbyId);
                            setCurrentScreen('MAIN_MENU');
                            setLobbyId('');
                            setGameState(null);
                          }}
                          className="bg-[#243a2c] hover:bg-[#2d4a38] text-white font-bold py-3 px-8 rounded-xl transition-colors"
                        >
                          Return to Menu
                        </button>
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
                      onClick={() => socket?.emit('pass_turn', { lobbyId }, (res: any) => res.error && alert(res.error))}
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
      </div>
    </div>
  );
}

