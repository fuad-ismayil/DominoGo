// Real Sound Effects using HTML5 Audio

let isAudioInitialized = false;

// Preload audio elements
const audioCache: Record<string, HTMLAudioElement> = {};

export function initAudio() {
    if (typeof window === 'undefined' || isAudioInitialized) return;

    // Preload all sounds
    const types: SoundType[] = ['place', 'draw', 'win', 'lose', 'turn', 'error'];
    types.forEach(type => {
        const audio = new Audio(`/sounds/${type}.mp3`);
        audio.preload = 'auto';
        audioCache[type] = audio;
    });

    isAudioInitialized = true;
}

// Master volume control (0.0 to 1.0)
const MASTER_VOL = 0.5;

type SoundType = 'place' | 'draw' | 'win' | 'lose' | 'turn' | 'error';

export function playSound(type: SoundType) {
    if (!isAudioInitialized || !audioCache[type]) return;

    try {
        // Clone the node to allow overlapping sounds (polyphony)
        const sound = audioCache[type].cloneNode() as HTMLAudioElement;
        sound.volume = MASTER_VOL;

        // Play and swallow any AutoPlay policy errors gracefully
        sound.play().catch(e => {
            // Browser might block autoplay if user hasn't interacted with document
            console.debug('Audio play prevented:', e);
        });

        // Trigger corresponding haptics
        switch (type) {
            case 'place': triggerHaptic(15); break;
            case 'draw': triggerHaptic(20); break;
            case 'turn': triggerHaptic([10, 30, 10]); break;
            case 'win': triggerHaptic([30, 50, 30, 50, 50]); break;
            case 'lose': triggerHaptic(100); break;
            case 'error': triggerHaptic(30); break;
        }
    } catch (e) {
        console.error('Failed to play sound', e);
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
