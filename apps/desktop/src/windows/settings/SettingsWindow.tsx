import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Check,
  FolderOpen,
  ChevronLeft,
  ChevronRight,
  Trash2,
} from "lucide-react";
import {
  GearSix,
  HandGrabbing,
  CircleDashed,
  FolderOpen as PhFolderOpen,
  Cpu,
  ClockCounterClockwise,
  Info as PhInfo,
  ArrowsClockwise,
  Pulse as PhPulse,
} from "@phosphor-icons/react";
import {
  WindowFrame,
  ToggleSwitch,
  SegmentedControl,
  WheelButton,
  SettingRow,
  SettingSection,
} from "../../ui/WheelUI";

type SettingsNavId =
  | "general"
  | "trigger"
  | "wheel_ui"
  | "status"
  | "output_general"
  | "output_images"
  | "output_audio"
  | "output_video"
  | "engines"
  | "history"
  | "about";

interface NavChild {
  id: SettingsNavId;
  label: string;
}

interface NavItem {
  id: SettingsNavId;
  label: string;
  icon: React.ReactNode;
  group: "core" | "features";
  /** Nested pages, revealed while this item or one of them is active. */
  children?: NavChild[];
}

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<SettingsNavId>("general");
  const [navHistory, setNavHistory] = useState<SettingsNavId[]>(["general"]);
  const [navHistoryIndex, setNavHistoryIndex] = useState<number>(0);
  const [settings, setSettings] = useState<any>(null);
  const [historyCount, setHistoryCount] = useState<number>(0);
  const [historyList, setHistoryList] = useState<any[]>([]);
  const [ffmpegStatus, setFfmpegStatus] = useState<any>(null);
  const [isSaved, setIsSaved] = useState<boolean>(false);
  const [ffmpegError, setFfmpegError] = useState<string | null>(null);
  const [ffmpegDownload, setFfmpegDownload] = useState<{
    state: "idle" | "downloading" | "done" | "error";
    percent: number;
  }>({ state: "idle", percent: 0 });

  // Lets the FFmpeg HUD prompt drop the user straight on the fix.
  useEffect(() => {
    const unlisten = listen<string>("settings-navigate", ({ payload }) => {
      const page = payload as SettingsNavId;
      setNavHistory([page]);
      setNavHistoryIndex(0);
      setActiveTab(page);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Auto-updater state
  const [appVersion, setAppVersion] = useState<string>("v0.1.0-pre-alpha.1");
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "available" | "downloading" | "ready" | "up_to_date" | "error"
  >("idle");
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  // `getCurrentWebviewWindow()` returns a fresh object on every call, so using it
  // directly as an effect dependency re-ran this effect on every render. Resolve
  // it once so mounting is genuinely mount-only.
  const appWindow = useMemo(() => getCurrentWebviewWindow(), []);

  useEffect(() => {
    appWindow.unminimize().catch(() => {});
    appWindow.show().catch(() => {});
    appWindow.setFocus().catch(() => {});
  }, [appWindow]);

  async function refreshFfmpegStatus() {
    try {
      setFfmpegStatus(await invoke<any>("get_ffmpeg_report"));
    } catch (e) {
      console.error("Failed to read ffmpeg status", e);
    }
  }

  async function handleDownloadFfmpeg() {
    setFfmpegError(null);
    setFfmpegDownload({ state: "downloading", percent: 0 });
    try {
      await invoke("download_ffmpeg");
    } catch (e) {
      setFfmpegError(String(e));
      setFfmpegDownload({ state: "error", percent: 0 });
    }
  }

  async function handleLocateFfmpeg() {
    setFfmpegError(null);
    try {
      const picked = await open({ multiple: false, directory: false });
      if (typeof picked !== "string") return;

      const result = await invoke<any>("validate_ffmpeg_path", { path: picked });
      if (!result?.ok) {
        setFfmpegError(result?.reason ?? "That file is not a working FFmpeg.");
        return;
      }

      // Persist through save_settings so the backend republishes the override.
      const current = await invoke<any>("get_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          ffmpeg: { ...current.ffmpeg, custom_path: picked },
        },
      });
      await refreshFfmpegStatus();
    } catch (e) {
      setFfmpegError(String(e));
    }
  }

  async function handleClearFfmpegPath() {
    setFfmpegError(null);
    const current = await invoke<any>("get_settings");
    await invoke("save_settings", {
      settings: {
        ...current,
        ffmpeg: { ...current.ffmpeg, custom_path: null },
      },
    });
    await refreshFfmpegStatus();
  }

  // FFmpeg download runs in Rust and reports back over events.
  useEffect(() => {
    const unlistenProgress = listen<any>("ffmpeg-download-progress", (e) => {
      const pct = Math.round(e.payload?.percent ?? 0);
      setFfmpegDownload({ state: "downloading", percent: pct });
    });

    const unlistenDone = listen<any>("ffmpeg-download-finished", async (e) => {
      if (e.payload?.ok) {
        setFfmpegDownload({ state: "done", percent: 100 });
        setFfmpegError(null);
      } else {
        setFfmpegDownload({ state: "error", percent: 0 });
        setFfmpegError(e.payload?.error ?? "The FFmpeg download failed.");
      }
      await refreshFfmpegStatus();
    });

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenDone.then((fn) => fn());
    };
  }, []);

  // Load initial settings and backend statuses
  useEffect(() => {
    async function loadData() {
      try {
        const s = await invoke<any>("get_settings");
        setSettings(s);
      } catch (e) {
        console.error("Failed to load settings", e);
      }

      try {
        const hist = await invoke<any[]>("get_history", { limit: 50 });
        setHistoryList(hist);
        setHistoryCount(hist.length);
      } catch (e) {
        console.error("Failed to load history count", e);
      }

      try {
        const ff = await invoke<any>("get_ffmpeg_report");
        setFfmpegStatus(ff);
      } catch (e) {
        console.error("Failed to check ffmpeg status", e);
      }

      try {
        const ver = await invoke<string>("get_app_version");
        if (ver) setAppVersion(ver);
      } catch (e) {
        console.error("Failed to get app version", e);
      }
    }
    loadData();
  }, []);

  // Listen for background auto-updater events
  useEffect(() => {
    let unlistenDownloaded: (() => void) | null = null;
    let unlistenProgress: (() => void) | null = null;

    listen<{ version: string }>("update-downloaded", (event) => {
      setUpdateStatus("ready");
      setUpdateVersion(event.payload.version);
    }).then((un) => {
      unlistenDownloaded = un;
    });

    listen<{ chunk_length: number; content_length: number | null }>("update-download-progress", () => {
      setUpdateStatus("downloading");
    }).then((un) => {
      unlistenProgress = un;
    });

    return () => {
      if (unlistenDownloaded) unlistenDownloaded();
      if (unlistenProgress) unlistenProgress();
    };
  }, []);

  const handleCheckUpdate = async () => {
    setUpdateStatus("checking");
    setUpdateError(null);
    try {
      const res = await invoke<{
        update_available: boolean;
        current_version: string;
        latest_version: string | null;
        body: string | null;
      }>("check_for_update");

      if (res.update_available && res.latest_version) {
        setUpdateStatus("available");
        setUpdateVersion(res.latest_version);
      } else {
        setUpdateStatus("up_to_date");
      }
    } catch (e: any) {
      console.warn("Update check error:", e);
      setUpdateStatus("error");
      setUpdateError(typeof e === "string" ? e : (e?.message || "Failed to check for updates"));
    }
  };

  const handleDownloadAndInstall = async () => {
    setUpdateStatus("downloading");
    setUpdateError(null);
    try {
      const ver = await invoke<string>("download_and_install_update");
      setUpdateStatus("ready");
      setUpdateVersion(ver);
    } catch (e: any) {
      console.warn("Update install error:", e);
      setUpdateStatus("error");
      setUpdateError(typeof e === "string" ? e : (e?.message || "Failed to download update"));
    }
  };

  const handleRestart = () => {
    invoke("restart_app").catch(console.error);
  };

  const handleSelectTab = (tabId: SettingsNavId) => {
    if (tabId === activeTab) return;
    const newHist = navHistory.slice(0, navHistoryIndex + 1);
    newHist.push(tabId);
    setNavHistory(newHist);
    setNavHistoryIndex(newHist.length - 1);
    setActiveTab(tabId);
  };

  const handleNavBack = () => {
    if (navHistoryIndex > 0) {
      const prevIdx = navHistoryIndex - 1;
      setNavHistoryIndex(prevIdx);
      setActiveTab(navHistory[prevIdx]);
    }
  };

  const handleNavForward = () => {
    if (navHistoryIndex < navHistory.length - 1) {
      const nextIdx = navHistoryIndex + 1;
      setNavHistoryIndex(nextIdx);
      setActiveTab(navHistory[nextIdx]);
    }
  };

  /** Persist a status HUD anchor change and immediately re-place the live window. */
  const handleStatusPosition = async (key: "status_vertical" | "status_horizontal", val: string) => {
    if (!settings) return;
    const updated = {
      ...settings,
      wheel_ui: { ...settings.wheel_ui, [key]: val },
    };
    await handleSaveSettings(updated);
    try {
      await invoke("apply_status_position", {
        vertical: updated.wheel_ui.status_vertical ?? "bottom",
        horizontal: updated.wheel_ui.status_horizontal ?? "center",
      });
    } catch (e) {
      console.error("Failed to reposition status window", e);
    }
  };

  const handleSaveSettings = async (newSettings: any) => {
    setSettings(newSettings);
    try {
      await invoke("save_settings", { settings: newSettings });
      setIsSaved(true);
      setTimeout(() => setIsSaved(false), 2000);
    } catch (e) {
      console.error("Failed to save settings", e);
    }
  };

  const handleClearHistory = async () => {
    try {
      await invoke("clear_history");
      setHistoryList([]);
      setHistoryCount(0);
    } catch (e) {
      console.error("Failed to clear history", e);
    }
  };

  const handleDeleteHistoryItem = async (id: string) => {
    try {
      await invoke("delete_history_item", { id });
      setHistoryList((prev) => prev.filter((item) => item.id !== id));
      setHistoryCount((prev) => Math.max(0, prev - 1));
    } catch (e) {
      console.error("Failed to delete history item", e);
    }
  };



  const navItems: NavItem[] = [
    {
      id: "general",
      label: "General",
      icon: <GearSix size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "trigger",
      label: "Trigger",
      icon: <HandGrabbing size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "wheel_ui",
      label: "The Wheel",
      icon: <CircleDashed size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "status",
      label: "Progress Window",
      icon: <PhPulse size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "output_general",
      label: "Output",
      icon: <PhFolderOpen size={18} weight="bold" />,
      group: "core",
      children: [
        { id: "output_general", label: "General" },
        { id: "output_images", label: "Images" },
        { id: "output_audio", label: "Audio" },
        { id: "output_video", label: "Video" },
      ],
    },
    {
      id: "engines",
      label: "FFmpeg",
      icon: <Cpu size={18} weight="bold" />,
      group: "features",
    },
    {
      id: "history",
      label: "Recent Files",
      icon: <ClockCounterClockwise size={18} weight="bold" />,
      group: "features",
    },
    {
      id: "about",
      label: "About",
      icon: <PhInfo size={18} weight="bold" />,
      group: "features",
    },
  ];

if (!settings) {
    return (
      <WindowFrame title="Settings" bodyClassName="items-center justify-center text-text-muted">
        Loading…
      </WindowFrame>
    );
  }

  return (
    <WindowFrame
      title="Settings"
      bodyClassName="flex"
      titleBarExtras={
        <>
          {/* Page history arrows live in the title bar, so they only ever occupy
              the one slot every window reserves for header controls. */}
          <div className="flex items-center gap-0.5">
          <button
            type="button"
            disabled={navHistoryIndex <= 0}
            onClick={handleNavBack}
            className="w-5 h-5 flex items-center justify-center rounded text-text-muted hover:text-text disabled:opacity-20 transition-colors cursor-default"
            title="Back"
          >
            <ChevronLeft size={13} strokeWidth={2.5} />
          </button>
          <button
            type="button"
            disabled={navHistoryIndex >= navHistory.length - 1}
            onClick={handleNavForward}
            className="w-5 h-5 flex items-center justify-center rounded text-text-muted hover:text-text disabled:opacity-20 transition-colors cursor-default"
            title="Forward"
          >
            <ChevronRight size={13} strokeWidth={2.5} />
          </button>
        </div>

        {isSaved && (
            <span className="flex items-center gap-1.5 text-xs text-accent font-medium">
              <Check size={12} strokeWidth={2.5} />
              Saved
            </span>
          )}
        </>
      }
    >
      {/* Main Body: Left Sidebar + Right Settings Panel */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Navigation Sidebar */}
        <aside className="w-[224px] border-r border-line-subtle bg-surface-window flex flex-col p-2 shrink-0 overflow-y-auto">
          {/* Navigation Items */}
          <nav className="flex flex-col gap-[2px]">
            {navItems.map((item, idx) => {
              const prevItem = navItems[idx - 1];
              const showDivider = prevItem && prevItem.group === "core" && item.group === "features";

              const childActive = item.children?.some((c) => c.id === activeTab) ?? false;
              // A parent reads as active while it or any of its pages is open.
              const isActive = activeTab === item.id || childActive;

              return (
                <div key={item.id}>
                  {showDivider && (
                    <div className="my-[6px] border-t border-line-subtle" />
                  )}
                  <button
                    onClick={() =>
                      handleSelectTab(item.children?.[0]?.id ?? item.id)
                    }
                    className={`w-full flex items-center gap-[10px] px-2.5 py-[6px] rounded-[6px] text-[13px] transition-all cursor-default text-left select-none ${
                      isActive
                        ? "bg-accent/10 text-text font-medium"
                        : "text-text-muted hover:text-text hover:bg-overlay"
                    }`}
                  >
                    <span className={`flex-shrink-0 w-5 h-5 flex items-center justify-center ${isActive ? "text-accent" : "text-text-muted"}`}>
                      {item.icon}
                    </span>
                    <span className={isActive ? "text-text font-medium" : "text-text-secondary"}>{item.label}</span>
                  </button>

                  {item.children && childActive && (
                    <div className="mt-[2px] mb-[2px] ml-[26px] pl-[10px] border-l border-line-subtle flex flex-col gap-[1px]">
                      {item.children.map((child) => {
                        const childIsActive = activeTab === child.id;
                        return (
                          <button
                            key={child.id}
                            onClick={() => handleSelectTab(child.id)}
                            className={`w-full text-left px-2 py-[5px] rounded-[6px] text-[12px] transition-all cursor-default select-none ${
                              childIsActive
                                ? "bg-accent/10 text-text font-medium"
                                : "text-text-muted hover:text-text hover:bg-overlay"
                            }`}
                          >
                            {child.label}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
          </nav>
        </aside>

        {/* Right content panel — same dark shade as sidebar */}
        <main className="flex-1 bg-surface-window px-4 py-5 overflow-y-auto">
          {/* TAB 1: GENERAL */}
          {activeTab === "general" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Start Lime at Login"
                  description="Open Lime when Windows starts"
                >
                  <ToggleSwitch
                    checked={settings.general.launch_at_login}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, launch_at_login: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Minimize to Tray"
                  description="Closing a window leaves Lime running in the background"
                >
                  <ToggleSwitch
                    checked={settings.general.minimize_to_tray}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, minimize_to_tray: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>

              <SettingSection title="Updates">
                <SettingRow
                  title="Install updates automatically"
                  description="Check for new versions and install them"
                >
                  <ToggleSwitch
                    checked={settings.general.auto_update ?? true}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, auto_update: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 2: TRIGGER & DRAG */}
          {activeTab === "trigger" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Key to hold"
                  description="Hold this while dragging files to open the wheel"
                >
                  <SegmentedControl
                    value={settings.trigger.modifier}
                    options={[
                      { label: "Shift", value: "shift" },
                      { label: "Ctrl", value: "ctrl" },
                      { label: "Alt", value: "alt" },
                      { label: "None", value: "none", title: "Always active on drag" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, modifier: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Drag distance"
                  description="How far you must drag before the wheel opens. Higher values stop it opening by accident."
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-text-muted w-12 text-right">
                      {settings.trigger.movement_threshold_px} px
                    </span>
                    <input
                      type="range"
                      min="2"
                      max="20"
                      value={settings.trigger.movement_threshold_px}
                      onChange={(e) => {
                        const updated = {
                          ...settings,
                          trigger: {
                            ...settings.trigger,
                            movement_threshold_px: Number(e.target.value),
                          },
                        };
                        handleSaveSettings(updated);
                      }}
                      className="w-28 accent-accent cursor-default bg-surface-field rounded-full h-1"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Wait before giving up"
                  description="How long to keep trying to hand a file to another app, in milliseconds"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-text-muted w-16 text-right">
                      {settings.trigger.confirm_timeout_ms} ms
                    </span>
                    <input
                      type="range"
                      min="100"
                      max="800"
                      step="50"
                      value={settings.trigger.confirm_timeout_ms}
                      onChange={(e) => {
                        const updated = {
                          ...settings,
                          trigger: {
                            ...settings.trigger,
                            confirm_timeout_ms: Number(e.target.value),
                          },
                        };
                        handleSaveSettings(updated);
                      }}
                      className="w-28 accent-accent cursor-default bg-surface-field rounded-full h-1"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Open on any drag"
                  description="Open the wheel even when you are not holding the key"
                >
                  <ToggleSwitch
                    checked={settings.trigger.always_show}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, always_show: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Pause"
                  description="Stop the wheel opening. Handy when it gets in the way."
                >
                  <ToggleSwitch
                    checked={settings.trigger.paused}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, paused: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 3: RADIAL WHEEL UI */}
          {activeTab === "wheel_ui" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Wheel size"
                  description="How large the wheel appears on screen"
                >
                  <SegmentedControl
                    value={settings.wheel_ui.size ?? "medium"}
                    options={[
                      { label: "Small", value: "small" },
                      { label: "Medium", value: "medium" },
                      { label: "Large", value: "large" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, size: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Hide actions that do not apply"
                  description="Grey out tools that cannot handle the file you dragged"
                >
                  <ToggleSwitch
                    checked={settings.wheel_ui.context_filter_enabled}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, context_filter_enabled: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Play a sound when hovering"
                  description="A click as you move across the wheel"
                >
                  <ToggleSwitch
                    checked={settings.wheel_ui.sound_enabled}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, sound_enabled: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Reduce motion"
                  description="Show the wheel without animations"
                >
                  <ToggleSwitch
                    checked={settings.wheel_ui.reduced_motion}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, reduced_motion: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 4: STATUS & PROGRESS */}
          {activeTab === "status" && (
            <div className="w-full">
              <SettingSection first title="Progress Window">
                <SettingRow
                  title="Detail"
                  description="Full shows the file names. Short shows one line."
                >
                  <SegmentedControl
                    value={settings.wheel_ui.hud_style ?? "standard"}
                    options={[
                      { label: "Standard", value: "standard" },
                      { label: "Compact", value: "compact" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, hud_style: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>

              <SettingSection title="Position">
                <SettingRow
                  title="Vertical"
                  description="Vertical positioning of the progress window"
                >
                  <SegmentedControl
                    value={settings.wheel_ui.status_vertical ?? "bottom"}
                    options={[
                      { label: "Top", value: "top" },
                      { label: "Middle", value: "middle" },
                      { label: "Bottom", value: "bottom" },
                    ]}
                    onChange={(val) => {
                      handleStatusPosition("status_vertical", val);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Horizontal"
                  description="Horizontal positioning of the progress window"
                >
                  <SegmentedControl
                    value={settings.wheel_ui.status_horizontal ?? "center"}
                    options={[
                      { label: "Left", value: "left" },
                      { label: "Center", value: "center" },
                      { label: "Right", value: "right" },
                    ]}
                    onChange={(val) => {
                      handleStatusPosition("status_horizontal", val);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 5a: OUTPUT & FILES > GENERAL */}
          {activeTab === "output_general" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Where to save"
                  description="Where finished files go"
                >
                  <SegmentedControl
                    value={settings.output.policy}
                    options={[
                      { label: "Next to Source", value: "next_to_source" },
                      { label: "Fixed Folder", value: "fixed_folder" },
                      { label: "Ask Each Time", value: "ask_each_time" },
                      { label: "Clipboard", value: "clipboard" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, policy: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                {settings.output.policy === "fixed_folder" && (
                  <SettingRow
                    title="Folder"
                    description="The folder finished files go in"
                  >
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        value={settings.output.fixed_folder || ""}
                        onChange={(e) => {
                          const updated = {
                            ...settings,
                            output: {
                              ...settings.output,
                              fixed_folder: e.target.value.trim() || null,
                            },
                          };
                          handleSaveSettings(updated);
                        }}
                        placeholder="C:\Users\...\Pictures\Lime"
                        className="w-48 px-2.5 py-1 text-xs bg-surface-field border border-line-subtle focus:border-line-strong rounded-md text-text font-mono outline-none cursor-text"
                      />
                      <WheelButton
                        variant="secondary"
                        onClick={async () => {
                          try {
                            const folder = await invoke<string | null>("pick_folder");
                            if (folder) {
                              const updated = {
                                ...settings,
                                output: {
                                  ...settings.output,
                                  fixed_folder: folder,
                                },
                              };
                              handleSaveSettings(updated);
                            }
                          } catch (e) {
                            console.error("Failed to pick folder", e);
                          }
                        }}
                      >
                        Browse...
                      </WheelButton>
                    </div>
                  </SettingRow>
                )}

                <SettingRow
                  title="Add to file names"
                  description="Text added before the extension, such as _converted. Leave blank for no change."
                >
                  <input
                    type="text"
                    value={settings.output.suffix || ""}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, suffix: e.target.value },
                      };
                      handleSaveSettings(updated);
                    }}
                    placeholder="_converted"
                    className="w-32 px-2.5 py-1 text-xs bg-surface-field border border-line-subtle focus:border-line-strong rounded-md text-text font-mono outline-none text-right cursor-text"
                  />
                </SettingRow>
              </SettingSection>

              <SettingSection title="Original Files">
                <SettingRow
                  title="Move originals to the Recycle Bin"
                  description="After a file is converted, send the original to the Recycle Bin instead of leaving it"
                >
                  <ToggleSwitch
                    checked={settings.output.recycle_source}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, recycle_source: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Replace files that already exist"
                  description="Overwrite a finished file if one is already there. Off means the same number is added instead."
                >
                  <ToggleSwitch
                    checked={settings.output.overwrite_source}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, overwrite_source: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 5b: OUTPUT & FILES > IMAGES */}
          {activeTab === "output_images" && (
            <div className="w-full">
              <SettingSection first title="Photo Details">
                <SettingRow
                  title="Keep photo details"
                  description="Carry over the camera, date and colour profile from the original image"
                >
                  <ToggleSwitch
                    checked={settings.output.metadata.images}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: {
                          ...settings.output,
                          metadata: { ...settings.output.metadata, images: checked },
                        },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>

              <p className="text-[11px] text-text-muted leading-relaxed mt-3">
                Works for JPEG, PNG and WebP. BMP, GIF, ICO, TIFF and AVIF cannot
                store this, so those always lose it.
              </p>
            </div>
          )}

          {/* TAB 5c: OUTPUT & FILES > AUDIO */}
          {activeTab === "output_audio" && (
            <div className="w-full">
              <SettingSection first title="Track Details">
                <SettingRow
                  title="Keep track details"
                  description="Carry over the title, artist, album and cover art from the original track"
                >
                  <ToggleSwitch
                    checked={settings.output.metadata.audio}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: {
                          ...settings.output,
                          metadata: { ...settings.output.metadata, audio: checked },
                        },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>

              <p className="text-[11px] text-text-muted leading-relaxed mt-3">
                Cover art carries over to M4A, MP4, FLAC and WMA. Ogg, Opus and WAV
                cannot hold a picture, so it is left out.
              </p>
            </div>
          )}

          {/* TAB 5d: OUTPUT & FILES > VIDEO */}
          {activeTab === "output_video" && (
            <div className="w-full">
              <SettingSection first title="Video Details">
                <SettingRow
                  title="Keep video details"
                  description="Carry over the title and chapter markers from the original video"
                >
                  <ToggleSwitch
                    checked={settings.output.metadata.video}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: {
                          ...settings.output,
                          metadata: { ...settings.output.metadata, video: checked },
                        },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>
            </div>
          )}


          {/* TAB 6: ENGINES */}
          {activeTab === "engines" && (
            <div className="w-full">
              <SettingSection
                first
                title="FFmpeg"
                description="Needed to turn video and audio into other formats, and to make high quality GIFs. Converting images does not need it."
              >
                <SettingRow
                  title="Status"
                  description={
                    ffmpegStatus?.installed
                      ? ffmpegStatus.version || "Ready"
                      : "Not found on this computer"
                  }
                >
                  <span
                    className={`text-xs font-medium px-2.5 py-1 rounded-md ${
                      ffmpegStatus?.installed
                        ? "text-positive bg-positive-soft"
                        : "text-caution bg-caution-soft"
                    }`}
                  >
                    {ffmpegStatus?.installed ? "Ready" : "Missing"}
                  </span>
                </SettingRow>

                <div className="flex items-center gap-2 pl-1">
                  <button
                    onClick={() => void handleDownloadFfmpeg()}
                    disabled={ffmpegDownload.state === "downloading"}
                    className="text-xs font-medium px-3 py-1.5 rounded-md bg-accent/15 text-accent hover:bg-accent/25 disabled:opacity-40 transition-colors"
                  >
                    {ffmpegDownload.state === "downloading"
                      ? `Downloading ${ffmpegDownload.percent}%`
                      : "Download"}
                  </button>

                  <button
                    onClick={() => void handleLocateFfmpeg()}
                    className="text-xs font-medium px-3 py-1.5 rounded-md bg-overlay-faint hover:bg-overlay transition-colors"
                  >
                    Choose a file
                  </button>

                  <button
                    onClick={() => void handleClearFfmpegPath()}
                    disabled={!ffmpegStatus?.custom_path}
                    className="text-xs font-medium px-3 py-1.5 rounded-md bg-overlay-faint hover:bg-overlay disabled:opacity-30 transition-colors"
                  >
                    Forget that file
                  </button>

                  <button
                    onClick={() => void invoke("open_onboarding")}
                    className="text-xs font-medium px-3 py-1.5 rounded-md bg-overlay-faint hover:bg-overlay transition-colors"
                  >
                    Run setup again
                  </button>
                </div>

                {ffmpegDownload.state === "downloading" && (
                  <div className="h-1 w-full rounded-full bg-overlay overflow-hidden">
                    <div
                      className="h-full bg-accent transition-[width] duration-200"
                      style={{ width: `${ffmpegDownload.percent}%` }}
                    />
                  </div>
                )}

                {ffmpegError && (
                  <p className="text-xs text-negative leading-relaxed">{ffmpegError}</p>
                )}

                {ffmpegStatus?.custom_path_stale && (
                  <p className="text-xs text-caution leading-relaxed">
                    The file you chose is no longer there, so Lime is looking
                    somewhere else.
                  </p>
                )}

                {ffmpegStatus?.installed && ffmpegStatus?.path && (
                  <p className="text-[11px] text-text/35 font-mono truncate">
                    Using: {ffmpegStatus.path}
                  </p>
                )}

                {ffmpegStatus?.managed_dir && (
                  <p className="text-[11px] text-text/35 font-mono truncate">
                    Downloads go to: {ffmpegStatus.managed_dir}
                  </p>
                )}
              </SettingSection>
            </div>
          )}

          {/* TAB 7: HISTORY */}
          {activeTab === "history" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Saved history"
                  description={`${historyCount} finished ${historyCount === 1 ? "job" : "jobs"} recorded on this computer`}
                >
                  <WheelButton
                    variant="danger"
                    disabled={historyCount === 0}
                    onClick={handleClearHistory}
                  >
                    Clear
                  </WheelButton>
                </SettingRow>
              </SettingSection>

              {historyList.length > 0 && (
                <SettingSection title="Last Jobs">
                  <div className="space-y-1.5 max-h-72 overflow-y-auto pr-1">
                    {historyList.map((item) => {
                      const outPath = item.outputs?.[0] || item.inputs?.[0] || "";
                      const fileName = outPath.split(/[/\\]/).pop() || outPath;
                      const timeStr = item.created_at
                        ? new Date(item.created_at).toLocaleTimeString()
                        : "";
                      return (
                        <div
                          key={item.id}
                          className="p-2.5 bg-surface-field border border-line-subtle rounded-lg flex items-center justify-between gap-3 text-xs"
                        >
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <span
                                className="font-medium text-text truncate"
                                title={outPath}
                              >
                                {fileName}
                              </span>
                              <span className="text-[10px] text-text-muted bg-overlay px-1.5 py-0.5 rounded font-mono shrink-0">
                                {item.action_id}
                              </span>
                            </div>
                            <div className="text-[11px] text-text-muted mt-0.5 truncate">
                              {timeStr} • {outPath}
                            </div>
                          </div>

                          <div className="flex items-center gap-1 shrink-0">
                            {outPath && (
                              <button
                                onClick={() =>
                                  invoke("open_in_folder", { path: outPath })
                                }
                                className="p-1.5 text-text-muted hover:text-text bg-overlay-faint hover:bg-overlay rounded-md transition-colors cursor-default"
                                title="Show in File Explorer"
                              >
                                <FolderOpen size={13} />
                              </button>
                            )}
                            <button
                              onClick={() => handleDeleteHistoryItem(item.id)}
                              className="p-1.5 text-text-muted hover:text-negative bg-overlay-faint hover:bg-negative-soft rounded-md transition-colors cursor-default"
                              title="Remove from history"
                            >
                              <Trash2 size={13} />
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </SettingSection>
              )}
            </div>
          )}

          {/* TAB 8: ABOUT */}
          {activeTab === "about" && (
            <div className="w-full">
              <SettingSection first>
                <div className="flex flex-col items-center justify-center p-6 bg-surface-field border border-line-subtle rounded-lg mb-4 text-center">
                  <img
                    src="/logo.svg"
                    alt="Lime"
                    className="w-[100px] object-cover select-none pointer-events-none"
                  />
                  <h2 className="text-base font-semibold text-text tracking-wide">Lime</h2>
                  <p className="text-xs text-text-muted mt-1">
                    A wheel of tools for files on Windows
                  </p>
                </div>

                <div className="p-4 bg-surface-field border border-line-subtle rounded-lg space-y-3">
                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <span className="text-text-muted">Version</span>
                      <div className="font-mono text-text mt-0.5 flex items-center gap-2">
                        <span>{appVersion}</span>
                        {updateStatus === "up_to_date" && (
                          <span className="text-[11px] text-positive font-sans flex items-center gap-1">
                            <Check size={12} className="inline" /> Up to date
                          </span>
                        )}
                        {updateStatus === "ready" && (
                          <span className="text-[11px] text-accent font-sans font-medium">
                            • v{updateVersion} installed & ready
                          </span>
                        )}
                        {updateStatus === "available" && (
                          <span className="text-[11px] text-caution font-sans font-medium">
                            • v{updateVersion} available
                          </span>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center gap-2">
                      {updateStatus === "ready" ? (
                        <WheelButton variant="primary" onClick={handleRestart}>
                          <ArrowsClockwise size={12} className="inline mr-1" />
                          Restart Lime
                        </WheelButton>
                      ) : updateStatus === "available" ? (
                        <WheelButton variant="primary" onClick={handleDownloadAndInstall}>
                          Install v{updateVersion}
                        </WheelButton>
                      ) : (
                        <WheelButton
                          variant="secondary"
                          onClick={handleCheckUpdate}
                          disabled={updateStatus === "checking" || updateStatus === "downloading"}
                        >
                          <ArrowsClockwise
                            size={12}
                            className={`inline mr-1 ${updateStatus === "checking" || updateStatus === "downloading" ? "animate-spin" : ""}`}
                          />
                          {updateStatus === "checking"
                            ? "Checking…"
                            : updateStatus === "downloading"
                            ? "Downloading…"
                            : "Check for Updates"}
                        </WheelButton>
                      )}
                    </div>
                  </div>

                  {updateStatus === "error" && (
                    <div className="text-[11px] text-negative bg-negative-soft border border-negative/20 rounded px-2.5 py-1.5">
                      {updateError || "Could not check for updates."}
                    </div>
                  )}

                  <div className="h-px bg-overlay-faint" />

                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <div className="text-text-muted">Which versions to offer</div>
                      <div className="text-text-secondary text-[11px] mt-0.5">
                        Pre-release means you get alpha and beta builds early
                      </div>
                    </div>
                    <SegmentedControl
                      value={settings?.general?.include_prereleases ?? true ? "prerelease" : "stable"}
                      options={[
                        { label: "Stable", value: "stable" },
                        { label: "Pre-release", value: "prerelease" },
                      ]}
                      onChange={(val) => {
                        if (settings) {
                          const updated = {
                            ...settings,
                            general: { ...settings.general, include_prereleases: val === "prerelease" },
                          };
                          handleSaveSettings(updated);
                        }
                      }}
                    />
                  </div>

                  <div className="h-px bg-overlay-faint" />

                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <div className="text-text-muted">Where Lime keeps its files</div>
                      <div className="font-mono text-text-muted text-[11px] truncate max-w-xs mt-0.5">
                        %LOCALAPPDATA%\Lime\
                      </div>
                    </div>
                    <WheelButton
                      variant="secondary"
                      onClick={() => invoke("open_data_folder")}
                    >
                      <FolderOpen size={12} className="inline mr-1" />
                      Open folder
                    </WheelButton>
                  </div>
                </div>
              </SettingSection>
            </div>
          )}
        </main>
      </div>
    </WindowFrame>
  );
}

