use std::sync::atomic::{AtomicBool, Ordering};
use tracing::warn;

static SOUND_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_sound_enabled(enabled: bool) {
    SOUND_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_sound_enabled() -> bool {
    SOUND_ENABLED.load(Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(psz_sound: *const u16, hmod: usize, fdw_sound: u32) -> i32;
    }

    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;

    static HIGHLIGHT_WAV: &[u8] = include_bytes!("../assets/highlight.wav");
    static LAST_PLAY_TIME: AtomicU64 = AtomicU64::new(0);

    pub fn play_hover() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let last = LAST_PLAY_TIME.load(Ordering::Relaxed);
        // Debounce to max 40Hz (25ms) to prevent audio jitter on rapid coordinate updates
        if now.saturating_sub(last) < 25 {
            return;
        }
        LAST_PLAY_TIME.store(now, Ordering::Relaxed);

        unsafe {
            let res = PlaySoundW(
                HIGHLIGHT_WAV.as_ptr() as *const u16,
                0,
                SND_ASYNC | SND_MEMORY | SND_NODEFAULT,
            );
            if res == 0 {
                super::warn!("PlaySoundW failed to play hover audio");
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub fn play_hover() {}
}

/// Plays the wedge hover sound using native system audio if sound effects are enabled.
pub fn play_hover_sound() {
    if !is_sound_enabled() {
        return;
    }
    platform::play_hover();
}
