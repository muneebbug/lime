import { invoke } from "@tauri-apps/api/core";

/**
 * Triggers native system-level hover sound via Rust WinMM backend.
 * Zero latency, immune to WebView2 autoplay and focus restrictions.
 */
export function playHoverSound() {
  try {
    invoke("play_hover_sound").catch(() => {});
  } catch {}
}
