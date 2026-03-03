// Native Web Audio API Synthesizer for instant, zero-dependency sound effects

let audioCtx: AudioContext | null = null;
let isAudioInitialized = false;

export function initAudio() {
    if (typeof window === 'undefined') return;
    if (!audioCtx) {
        audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
    }
    if (audioCtx.state === 'suspended') {
        audioCtx.resume();
    }
    isAudioInitialized = true;
}

// Master volume control (0.0 to 1.0)
const MASTER_VOL = 0.3;

type SoundType = 'place' | 'draw' | 'win' | 'lose' | 'turn' | 'error';

export function playSound(type: SoundType) {
    if (!isAudioInitialized || !audioCtx) return;

    const t = audioCtx.currentTime;
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();

    osc.connect(gain);
    gain.connect(audioCtx.destination);

    switch (type) {
        case 'place':
            // Short, high-pitched pop (like wood on felt)
            osc.type = 'triangle';
            osc.frequency.setValueAtTime(300, t);
            osc.frequency.exponentialRampToValueAtTime(100, t + 0.05);
            gain.gain.setValueAtTime(MASTER_VOL * 0.8, t);
            gain.gain.exponentialRampToValueAtTime(0.01, t + 0.05);
            osc.start(t);
            osc.stop(t + 0.05);
            triggerHaptic(15); // Light tap
            break;

        case 'draw':
            // Soft, low thud (sliding a tile)
            osc.type = 'sine';
            osc.frequency.setValueAtTime(150, t);
            osc.frequency.linearRampToValueAtTime(80, t + 0.1);
            gain.gain.setValueAtTime(MASTER_VOL * 0.5, t);
            gain.gain.linearRampToValueAtTime(0.01, t + 0.1);
            osc.start(t);
            osc.stop(t + 0.1);
            triggerHaptic(20);
            break;

        case 'turn':
            // Bright, pleasant ding
            osc.type = 'sine';
            osc.frequency.setValueAtTime(523.25, t); // C5
            osc.frequency.exponentialRampToValueAtTime(1046.50, t + 0.1); // C6 sweep
            gain.gain.setValueAtTime(0, t);
            gain.gain.linearRampToValueAtTime(MASTER_VOL * 0.4, t + 0.02);
            gain.gain.exponentialRampToValueAtTime(0.01, t + 0.3);
            osc.start(t);
            osc.stop(t + 0.3);
            triggerHaptic([10, 30, 10]); // double tap
            break;

        case 'win':
            // Cheerful major arpeggio
            const notes = [523.25, 659.25, 783.99, 1046.50]; // C E G C
            notes.forEach((freq, i) => {
                const o = audioCtx!.createOscillator();
                const g = audioCtx!.createGain();
                o.type = 'sine';
                o.connect(g);
                g.connect(audioCtx!.destination);
                const st = t + (i * 0.1);
                o.frequency.value = freq;
                g.gain.setValueAtTime(0, st);
                g.gain.linearRampToValueAtTime(MASTER_VOL * 0.5, st + 0.02);
                g.gain.exponentialRampToValueAtTime(0.01, st + 0.3);
                o.start(st);
                o.stop(st + 0.3);
            });
            triggerHaptic([30, 50, 30, 50, 50]); // celebration rumble
            break;

        case 'lose':
            // Low descending tone 
            osc.type = 'sawtooth';
            osc.frequency.setValueAtTime(300, t);
            osc.frequency.exponentialRampToValueAtTime(100, t + 0.5);
            gain.gain.setValueAtTime(MASTER_VOL * 0.4, t);
            gain.gain.exponentialRampToValueAtTime(0.01, t + 0.5);
            osc.start(t);
            osc.stop(t + 0.5);
            triggerHaptic(100); // long buzz
            break;

        case 'error':
            // Sharp distinct buzz
            osc.type = 'square';
            osc.frequency.setValueAtTime(150, t);
            gain.gain.setValueAtTime(MASTER_VOL * 0.3, t);
            gain.gain.exponentialRampToValueAtTime(0.01, t + 0.1);
            osc.start(t);
            osc.stop(t + 0.1);
            triggerHaptic(30);
            break;
    }
}

// Wrapper to prevent crashes in unsupported browsers
function triggerHaptic(pattern: number | number[]) {
    if (typeof navigator !== 'undefined' && navigator.vibrate) {
        try {
            navigator.vibrate(pattern);
        } catch (e) {
            // Ignore security/policy errors
        }
    }
}
