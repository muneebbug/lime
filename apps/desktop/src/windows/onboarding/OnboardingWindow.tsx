import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import { Check, Download, Loader2, FolderOpen, ArrowLeft, ArrowRight } from "lucide-react";
import { WindowFrame, LimeLogo } from "../../ui/WheelUI";

/**
 * First-run setup.
 *
 * Each step is a plain question with a plain answer. Step content lives in
 * `STEPS`, so adding one means adding an entry rather than restructuring the
 * component. Nothing here is required: FFmpeg only matters for video, audio and
 * GIF conversion, and every step can be skipped from the footer or the title bar.
 */



/**
 * Sizes are sent to the backend as the lowercase wire names, matching the
 * `WheelSize` enum. Capitalised names are accepted on read, but writing the
 * wrong case here means the choice is discarded and the wheel stays medium.
 */
const WHEEL_SIZES = [
  { value: "small", label: "Small", preview: 30 },
  { value: "medium", label: "Medium", preview: 40 },
  { value: "large", label: "Large", preview: 52 },
];

export function OnboardingWindow() {
  const [stepIndex, setStepIndex] = useState(0);
  const [settings, setSettings] = useState<any>(null);
  const [ffmpeg, setFfmpeg] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [download, setDownload] = useState<{
    state: "idle" | "downloading" | "error";
    percent: number;
  }>({ state: "idle", percent: 0 });

  const refreshFfmpeg = useCallback(async () => {
    try {
      setFfmpeg(await invoke<any>("get_ffmpeg_report"));
    } catch (e) {
      console.error("failed to read ffmpeg status", e);
    }
  }, []);

  useEffect(() => {
    // Rust already shows and focuses the window; re-asserting it here on every
    // mount would fight the user for focus.
    invoke<any>("get_settings").then(setSettings).catch(console.error);
    refreshFfmpeg();
  }, [refreshFfmpeg]);

  useEffect(() => {
    const onProgress = listen<any>("ffmpeg-download-progress", (e) =>
      setDownload({ state: "downloading", percent: Math.round(e.payload?.percent ?? 0) })
    );
    const onDone = listen<any>("ffmpeg-download-finished", async (e) => {
      if (e.payload?.ok) {
        setDownload({ state: "idle", percent: 100 });
        setError(null);
      } else {
        setDownload({ state: "error", percent: 0 });
        setError(e.payload?.error ?? "The download did not finish.");
      }
      refreshFfmpeg();
    });
    return () => {
      onProgress.then((f) => f());
      onDone.then((f) => f());
    };
  }, [refreshFfmpeg]);

  /** Save one change through the existing settings command. */
  const patch = useCallback(async (mutate: (draft: any) => void) => {
    try {
      const current = await invoke<any>("get_settings");
      const draft = structuredClone(current);
      mutate(draft);
      await invoke("save_settings", { settings: draft });
      setSettings(draft);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** Save what the user picked, remember setup was seen, then close. */
  const finish = useCallback(async () => {
    await patch((d) => {
      d.general.onboarding_completed = true;
      d.ffmpeg.declined_download = false;
    });
    await invoke("hide_onboarding").catch(console.error);
  }, [patch]);

  /** The title-bar X: record that setup was seen and hide. Never destroys. */
  const dismiss = useCallback(() => {
    void invoke("hide_onboarding").catch(console.error);
  }, []);

  const steps = [
    {
      id: "welcome",
      title: "Welcome to Lime",
      blurb: "Lime puts your tools on a wheel. Pick what you want to run, drop a file on it, and it runs.",
      content: (
        <div className="text-center">
          <p className="text-[13px] text-text-muted leading-relaxed">
            Three quick questions, then you are done. You can change any of them
            later in Settings.
          </p>
        </div>
      ),
    },
    {
      id: "size",
      title: "How big should the wheel be?",
      blurb: "Bigger leaves more room for longer action names.",
      content: (() => {
        const current = String(settings?.wheel_ui?.size ?? "medium").toLowerCase();
        return (
          <div className="grid grid-cols-3 gap-3">
            {WHEEL_SIZES.map((o) => {
              const selected = current === o.value;
              return (
                <button
                  key={o.value}
                  onClick={() =>
                    patch((d) => {
                      d.wheel_ui.size = o.value;
                    })
                  }
                  aria-pressed={selected}
                  className={`flex flex-col items-center justify-center gap-3 rounded-[10px] py-6 transition-colors border ${
                    selected
                      ? "border-accent/50 bg-accent/[0.08] text-accent"
                      : "border-line-subtle bg-overlay-faint text-text-secondary hover:bg-overlay"
                  }`}
                >
                  <span
                    className={`rounded-full border-2 ${
                      selected ? "border-accent" : "border-line-strong"
                    }`}
                    style={{ width: o.preview, height: o.preview }}
                  />
                  <span className="text-[13px] font-medium">{o.label}</span>
                </button>
              );
            })}
          </div>
        );
      })(),
    },
    {
      id: "trigger",
      title: "How do you open the wheel?",
      blurb: "Choose what brings it up.",
      content: (() => {
        const current = settings?.trigger?.always_show ?? false;
        const modifier = settings?.trigger?.modifier ?? "Shift";
        const options = [
          { value: true, label: "Always open", hint: "It follows the mouse" },
          { value: false, label: `Hold ${modifier}`, hint: "Only while the key is down" },
        ];
        return (
          <div className="grid grid-cols-2 gap-3">
            {options.map((o) => {
              const selected = current === o.value;
              return (
                <button
                  key={String(o.value)}
                  onClick={() =>
                    patch((d) => {
                      d.trigger.always_show = o.value;
                    })
                  }
                  aria-pressed={selected}
                  className={`flex flex-col items-center justify-center gap-1.5 rounded-[10px] px-4 py-6 transition-colors border ${
                    selected
                      ? "border-accent/50 bg-accent/[0.08] text-accent"
                      : "border-line-subtle bg-overlay-faint text-text-secondary hover:bg-overlay"
                  }`}
                >
                  <span className="text-[13px] font-medium">{o.label}</span>
                  <span
                    className={`text-[11px] ${selected ? "text-accent/70" : "text-text-muted"}`}
                  >
                    {o.hint}
                  </span>
                </button>
              );
            })}
          </div>
        );
      })(),
    },
    {
      id: "ffmpeg",
      title: "Convert video, audio and GIFs?",
      blurb: "These need FFmpeg, a separate program. Images do not.",
      content: (
        <div className="space-y-2.5">
          {ffmpeg?.installed ? (
            <div className="flex items-center justify-center gap-2 rounded-[10px] border border-positive/25 bg-positive-soft px-3 py-3 text-[13px] text-positive">
              <Check size={15} strokeWidth={2.5} className="shrink-0" />
              <span className="truncate">
                FFmpeg is ready{ffmpeg.version ? ` — ${ffmpeg.version}` : ""}
              </span>
            </div>
          ) : (
            <>
              <button
                onClick={async () => {
                  setError(null);
                  setDownload({ state: "downloading", percent: 0 });
                  await invoke("download_ffmpeg");
                }}
                disabled={download.state === "downloading"}
                className="w-full flex items-center justify-center gap-2 rounded-[10px] border border-accent/40 bg-accent/10 text-accent hover:bg-accent/15 disabled:opacity-50 transition-colors py-2.5 text-[13px] font-medium"
              >
                {download.state === "downloading" ? (
                  <>
                    <Loader2 size={15} className="animate-spin" />
                    Downloading {download.percent}%
                  </>
                ) : (
                  <>
                    <Download size={15} />
                    Download FFmpeg
                  </>
                )}
              </button>

              {download.state === "downloading" && (
                <div className="h-1 w-full rounded-full bg-line-subtle overflow-hidden">
                  <div
                    className="h-full bg-accent transition-[width] duration-200"
                    style={{ width: `${download.percent}%` }}
                  />
                </div>
              )}

              <button
                onClick={async () => {
                  const picked = await open({ multiple: false, directory: false });
                  if (typeof picked !== "string") return;
                  const result = await invoke<any>("validate_ffmpeg_path", {
                    path: picked,
                  });
                  if (!result?.ok) {
                    setError(result?.reason ?? "That file is not FFmpeg.");
                    return;
                  }
                  await patch((d) => {
                    d.ffmpeg.custom_path = picked;
                  });
                  refreshFfmpeg();
                }}
                className="w-full flex items-center justify-center gap-2 rounded-[10px] border border-line-subtle bg-overlay-faint hover:bg-overlay transition-colors py-2.5 text-[13px] font-medium text-text-secondary"
              >
                <FolderOpen size={15} />
                I already have FFmpeg
              </button>

              <p className="text-center text-[11px] text-text-muted pt-0.5">
                If FFmpeg is already installed, Lime finds it on its own.
              </p>
            </>
          )}

          {error && (
            <p className="text-center text-[12px] text-negative leading-relaxed">{error}</p>
          )}
        </div>
      ),
    },
    {
      id: "done",
      title: "You are set",
      blurb: "Open the wheel from the tray icon in the corner of your screen.",
      content: (
        <button
          onClick={finish}
          className="w-full rounded-[10px] border border-accent/40 bg-accent/10 text-accent hover:bg-accent/15 transition-colors py-2.5 text-[13px] font-medium"
        >
          Done
        </button>
      ),
    },
  ];

  const step = steps[stepIndex];
  const isFirst = stepIndex === 0;
  const isLast = stepIndex === steps.length - 1;

  return (
    <WindowFrame title="Onboarding" onClose={dismiss} bodyClassName="flex flex-col">
      <div className="flex-1 flex items-center overflow-y-auto">
        <div className="w-full max-w-[460px] mx-auto px-10 py-10 text-center">
          <motion.div
            key={step.id}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.2 }}
          >
            {step.id === "welcome" && (
              <div className="flex justify-center">
                <LimeLogo className="w-[124px]" />
              </div>
            )}

            <h1 className="text-[23px] font-semibold tracking-tight mb-2 text-balance text-text">
              {step.title}
            </h1>
            <p className="text-[13px] text-text-muted leading-relaxed mb-7 text-pretty">
              {step.blurb}
            </p>

            {step.content}
          </motion.div>
        </div>
      </div>

      {/* Back and Next sit against the window edges; the dots stay centred. */}
      <div className="shrink-0 border-t border-line-subtle px-5 py-3.5 flex items-center gap-3">
        <button
          onClick={() => setStepIndex((i) => Math.max(0, i - 1))}
          disabled={isFirst}
          className="flex items-center gap-1.5 text-[12px] text-text-muted hover:text-text disabled:opacity-25 disabled:hover:text-text-muted transition-colors px-1 py-1"
        >
          <ArrowLeft size={13} />
          Back
        </button>

        {!isFirst && !isLast && (
          <button
            onClick={finish}
            className="text-[12px] text-text-muted hover:text-text-secondary transition-colors px-1 py-1"
          >
            Skip
          </button>
        )}

        <div className="flex-1 flex items-center justify-center gap-1.5">
          {steps.map((s, i) => (
            <button
              key={s.id}
              onClick={() => setStepIndex(i)}
              aria-label={`Step ${i + 1} of ${steps.length}: ${s.title}`}
              aria-current={i === stepIndex}
              title={s.title}
              className={`h-1.5 rounded-full transition-all ${
                i === stepIndex ? "w-4 bg-accent" : "w-1.5 bg-overlay-hover hover:bg-overlay-strong"
              }`}
            />
          ))}
        </div>

        <div className="w-[68px] flex justify-end">
          {!isLast && (
            <button
              onClick={() => setStepIndex((i) => Math.min(steps.length - 1, i + 1))}
              className="flex items-center gap-1.5 rounded-[7px] bg-overlay hover:bg-overlay-dim transition-colors px-3.5 py-1.5 text-[12px] font-medium text-text"
            >
              Next
              <ArrowRight size={13} />
            </button>
          )}
        </div>
      </div>
    </WindowFrame>
  );
}