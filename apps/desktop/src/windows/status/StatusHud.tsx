import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "motion/react";
import { Loader2, CheckCircle2, AlertCircle } from "lucide-react";

type HudState = "loading" | "success" | "error";

interface HudInfo {
  id: string;
  type: HudState;
  message: string;
  subtext?: string;
}

interface JobStartedEvent {
  action_id: string;
  files: string[];
}

/** Human-readable summary of an action id, e.g. "convert.png" -> "Converting to PNG". */
function describeAction(actionId: string): string {
  if (actionId.startsWith("convert.")) {
    return `Converting to ${actionId.replace("convert.", "").toUpperCase()}`;
  }
  return actionId.replace("tool.", "").replace(/(^|\s)\S/g, (c) => c.toUpperCase());
}

/** Success and error states linger so the result stays readable, then self-dismiss. */
const AUTO_DISMISS_MS: Record<Exclude<HudState, "loading">, number> = {
  success: 2200,
  error: 6000,
};

/** Matches the card's exit transition, so the fade is never cut off. */
const EXIT_ANIM_MS = 180;

/**
 * Detached status/progress HUD.
 *
 * Lives in its own always-on-top webview so conversion state stays visible after
 * the radial wheel closes. Position is owned by the Rust side, driven by the
 * `status_vertical` / `status_horizontal` settings.
 */
export function StatusHud() {
  const [hud, setHud] = useState<HudInfo | null>(null);
  const [compact, setCompact] = useState(false);
  const dismissRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearDismiss = useCallback(() => {
    if (dismissRef.current) {
      clearTimeout(dismissRef.current);
      dismissRef.current = null;
    }
  }, []);

  const dismiss = useCallback(() => {
    clearDismiss();
    setHud(null);
    // Let the exit fade finish before the window disappears.
    setTimeout(() => invoke("hide_status").catch(console.error), EXIT_ANIM_MS);
  }, [clearDismiss]);

  const settle = useCallback(
    (info: HudInfo, delay: number) => {
      clearDismiss();
      dismissRef.current = setTimeout(() => {
        setHud((cur) => (cur?.id === info.id ? null : cur));
        setTimeout(() => invoke("hide_status").catch(console.error), EXIT_ANIM_MS);
      }, delay);
    },
    [clearDismiss],
  );

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    // Compact hides the file names, so the style has to be known before the first job.
    invoke<any>("get_settings")
      .then((s) => setCompact(s?.wheel_ui?.hud_style === "compact"))
      .catch(console.error);

    listen<any>("settings-updated", ({ payload }) => {
      setCompact(payload?.wheel_ui?.hud_style === "compact");
    }).then((u) => unlisteners.push(u));

    listen<JobStartedEvent>("job-started", ({ payload }) => {
      // A previous job's dismiss timer must not hide this job's HUD.
      clearDismiss();
      const info: HudInfo = {
        id: `${Date.now()}`,
        type: "loading",
        message: `${describeAction(payload.action_id)}...`,
        subtext: payload.files.join(", "),
      };
      setHud(info);
      invoke("show_status").catch(console.error);
    }).then((u) => unlisteners.push(u));

    listen<{ outputs: string[] }>("job-completed", ({ payload }) => {
      const names = payload.outputs
        .map((p) => p.split(/[/\\]/).pop())
        .filter(Boolean)
        .join(", ");
      const info: HudInfo = {
        id: `${Date.now()}`,
        type: "success",
        message: "Complete",
        subtext: names || "File converted successfully",
      };
      setHud(info);
      settle(info, AUTO_DISMISS_MS.success);
    }).then((u) => unlisteners.push(u));

    listen<{ error: string }>("job-failed", ({ payload }) => {
      const info: HudInfo = {
        id: `${Date.now()}`,
        type: "error",
        message: "Conversion failed",
        subtext: payload.error,
      };
      setHud(info);
      settle(info, AUTO_DISMISS_MS.error);
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((fn) => fn());
      clearDismiss();
    };
  }, [settle, clearDismiss]);

return (
    <div className="w-screen h-screen flex items-center justify-center p-2">
      <AnimatePresence>
        {hud && (
          <motion.div
            // Deliberately stable: a per-state key would make AnimatePresence keep
            // the outgoing and incoming cards mounted together, and the flex row
            // would lay them out side by side at half width.
            key="hud"
            // Opacity and a vertical offset only — animating `scale` would make the
            // card visibly change width between states.
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 6 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className={`w-full flex items-center rounded-2xl bg-neutral-950/92 border border-white/15 backdrop-blur-2xl shadow-2xl cursor-default select-none ${
              compact ? "flex-row gap-2 px-4 py-2.5" : "flex-col px-4 py-3 text-center"
            }`}
            onClick={dismiss}
          >
            {hud.type === "loading" && (
              <Loader2
                className={`w-5 h-5 shrink-0 text-[#cbe71f] animate-spin ${compact ? "" : "mb-1.5"}`}
              />
            )}
            {hud.type === "success" && (
              <CheckCircle2
                className={`w-5 h-5 shrink-0 text-[#cbe71f] ${compact ? "" : "mb-1.5"}`}
              />
            )}
            {hud.type === "error" && (
              <AlertCircle
                className={`w-5 h-5 shrink-0 text-rose-400 ${compact ? "" : "mb-1.5"}`}
              />
            )}

            <div
              className={
                compact
                  ? "text-[13px] font-semibold text-white tracking-wide leading-tight truncate"
                  : "text-[13px] font-semibold text-white tracking-wide leading-tight"
              }
            >
              {hud.message}
            </div>
            {!compact && hud.subtext && (
              <div className="text-[11px] text-neutral-300 mt-1 line-clamp-2 leading-snug font-sans">
                {hud.subtext}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}