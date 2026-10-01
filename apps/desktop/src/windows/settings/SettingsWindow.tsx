import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
} from "@phosphor-icons/react";
import {
  CaptionButtons,
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
  | "output"
  | "engines"
  | "history"
  | "about";

interface NavItem {
  id: SettingsNavId;
  label: string;
  icon: React.ReactNode;
  group: "core" | "features";
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

  // Auto-updater state
  const [appVersion, setAppVersion] = useState<string>("v0.1.0-pre-alpha.1");
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "available" | "downloading" | "ready" | "up_to_date" | "error"
  >("idle");
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  const appWindow = getCurrentWebviewWindow();

  useEffect(() => {
    appWindow.unminimize().catch(() => {});
    appWindow.show().catch(() => {});
    appWindow.setFocus().catch(() => {});
  }, [appWindow]);

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
        const ff = await invoke<any>("get_ffmpeg_status");
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

  const handleToggleContextMenu = (enabled: boolean) => {
    if (settings) {
      const updated = {
        ...settings,
        general: { ...settings.general, explorer_context_menu: enabled },
      };
      handleSaveSettings(updated);
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
      label: "Trigger & Drag",
      icon: <HandGrabbing size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "wheel_ui",
      label: "Radial Wheel",
      icon: <CircleDashed size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "output",
      label: "Output & Files",
      icon: <PhFolderOpen size={18} weight="bold" />,
      group: "core",
    },
    {
      id: "engines",
      label: "Engines",
      icon: <Cpu size={18} weight="bold" />,
      group: "features",
    },
    {
      id: "history",
      label: "History",
      icon: <ClockCounterClockwise size={18} weight="bold" />,
      group: "features",
    },
    {
      id: "about",
      label: "About Lime",
      icon: <PhInfo size={18} weight="bold" />,
      group: "features",
    },
  ];

  if (!settings) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-[#1f1e1e] text-[#8e8e93] select-none">
        <span className="text-[13px]">Loading settings…</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen w-screen bg-[#1f1e1e] text-white font-sans select-none overflow-hidden rounded-[12px] border border-white/[0.08] shadow-2xl">
      {/* Top Titlebar styled exactly like Raycast Windows */}
      <header
        data-tauri-drag-region
        onMouseDown={(e) => {
          if (
            e.button === 0 &&
            !(e.target as HTMLElement).closest("button, input, select, textarea, [data-no-drag], [data-tauri-drag-region='false']")
          ) {
            getCurrentWebviewWindow().startDragging().catch(() => {});
          }
        }}
        className="h-[38px] flex items-center justify-between pl-4 pr-0 border-b border-white/[0.07] bg-[#1f1e1e] select-none shrink-0 z-20"
      >
        <div data-tauri-drag-region className="flex items-center gap-2">
          <img src="/logo.svg" alt="Lime" className="w-4 h-4 object-contain pointer-events-none" />
          <span className="text-[13px] font-medium text-neutral-200 pointer-events-none">
            Settings
          </span>

          <div className="flex items-center gap-0.5 ml-2">
            <button
              type="button"
              disabled={navHistoryIndex <= 0}
              onClick={handleNavBack}
              className="w-5 h-5 flex items-center justify-center rounded text-neutral-400 hover:text-white disabled:opacity-20 transition-colors cursor-default"
              title="Back"
            >
              <ChevronLeft size={13} strokeWidth={2.5} />
            </button>
            <button
              type="button"
              disabled={navHistoryIndex >= navHistory.length - 1}
              onClick={handleNavForward}
              className="w-5 h-5 flex items-center justify-center rounded text-neutral-400 hover:text-white disabled:opacity-20 transition-colors cursor-default"
              title="Forward"
            >
              <ChevronRight size={13} strokeWidth={2.5} />
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3 h-full">
          {isSaved && (
            <span className="flex items-center gap-1.5 text-xs text-emerald-400 font-medium">
              <Check size={12} strokeWidth={2.5} />
              Saved
            </span>
          )}

          <CaptionButtons />
        </div>
      </header>

      {/* Main Body: Left Sidebar + Right Settings Panel */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Navigation Sidebar */}
        <aside className="w-[224px] border-r border-white/[0.07] bg-[#1f1e1e] flex flex-col p-2 shrink-0 overflow-y-auto">
          {/* Navigation Items */}
          <nav className="flex flex-col gap-[2px]">
            {navItems.map((item, idx) => {
              const isActive = activeTab === item.id;
              const prevItem = navItems[idx - 1];
              const showDivider = prevItem && prevItem.group === "core" && item.group === "features";

              return (
                <div key={item.id}>
                  {showDivider && (
                    <div className="my-[6px] border-t border-white/[0.06]" />
                  )}
                  <button
                    onClick={() => handleSelectTab(item.id)}
                    className={`w-full flex items-center gap-[10px] px-2.5 py-[6px] rounded-[6px] text-[13px] transition-all cursor-default text-left select-none ${
                      isActive
                        ? "bg-white/[0.09] text-white"
                        : "text-[#8e8e93] hover:text-white hover:bg-white/[0.05]"
                    }`}
                  >
                    <span className="flex-shrink-0 w-5 h-5 flex items-center justify-center text-white">
                      {item.icon}
                    </span>
                    <span className={isActive ? "text-white font-medium" : "text-[#d1d1d6]"}>{item.label}</span>
                  </button>
                </div>
              );
            })}
          </nav>
        </aside>

        {/* Right content panel — same dark shade as sidebar */}
        <main className="flex-1 bg-[#1f1e1e] px-4 py-5 overflow-y-auto">
          {/* TAB 1: GENERAL */}
          {activeTab === "general" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Launch at Windows Login"
                  description="Start Lime automatically in the background when signing into Windows"
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
                  title="Close to System Tray"
                  description="Keep Lime running in the background tray when tool windows are closed"
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

              <SettingSection title="Updates & Release Channel">
                <SettingRow
                  title="Automatically Check for Updates"
                  description="Check for new versions in the background on startup"
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

                <SettingRow
                  title="Receive Pre-release Updates"
                  description="Include Beta and Alpha preview releases before general availability"
                >
                  <ToggleSwitch
                    checked={settings.general.include_prereleases ?? true}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, include_prereleases: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>
              </SettingSection>

              <SettingSection title="Windows Explorer Integration">
                <SettingRow
                  title="Explorer Context Menu"
                  description="Add 'Open with Lime' to Windows Explorer right-click context menus"
                >
                  <ToggleSwitch
                    checked={settings.general.explorer_context_menu}
                    onChange={handleToggleContextMenu}
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
                  title="Activation Modifier Key"
                  description="Key held while dragging files to summon the radial menu"
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
                  title="Movement Threshold"
                  description="Minimum drag distance in pixels before the radial wheel appears"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-400 w-12 text-right">
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
                      className="w-28 accent-[#ff6339] cursor-default bg-[#2a2929] rounded-full h-1"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Drop Confirmation Timeout"
                  description="Milliseconds to wait for Windows OLE drop confirmation before cancelling"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-400 w-16 text-right">
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
                      className="w-28 accent-[#ff6339] cursor-default bg-[#2a2929] rounded-full h-1"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Always Show on Drag"
                  description="Display the wheel on any file drag, even without holding a modifier key"
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
                  title="Pause Lime Trigger"
                  description="Temporarily silence the radial gesture without quitting the app"
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
                  title="Wheel Diameter"
                  description="Outer pixel diameter of the circular radial wheel"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-400 w-14 text-right">
                      {settings.wheel_ui.size} px
                    </span>
                    <input
                      type="range"
                      min="280"
                      max="400"
                      step="10"
                      value={settings.wheel_ui.size}
                      onChange={(e) => {
                        const updated = {
                          ...settings,
                          wheel_ui: { ...settings.wheel_ui, size: Number(e.target.value) },
                        };
                        handleSaveSettings(updated);
                      }}
                      className="w-28 accent-[#ff6339] cursor-default bg-[#2a2929] rounded-full h-1"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Context Filtering"
                  description="Automatically dim or filter actions incompatible with dragged files"
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
                  title="Hover Audio Effects"
                  description="Play subtle audio feedback when hovering over tools and extensions"
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
                  title="Reduced Motion"
                  description="Disable spring animations for instant radial appearance"
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

          {/* TAB 4: OUTPUT & FILES */}
          {activeTab === "output" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="Save Location Policy"
                  description="Where converted and processed files are saved"
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
                    title="Fixed Folder Path"
                    description="Absolute directory path where output files will be written"
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
                        className="w-48 px-2.5 py-1 text-xs bg-[#2a2929] border border-white/[0.08] focus:border-white/30 rounded-md text-white font-mono outline-none cursor-text"
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
                  title="Filename Suffix"
                  description="Appended between file stem and extension (leave blank for clean filename)"
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
                    className="w-32 px-2.5 py-1 text-xs bg-[#2a2929] border border-white/[0.08] focus:border-white/30 rounded-md text-white font-mono outline-none text-right cursor-text"
                  />
                </SettingRow>
              </SettingSection>

              <SettingSection title="File Safety & Preservation">
                <SettingRow
                  title="Preserve EXIF Metadata"
                  description="Retain camera, timestamp, and device metadata during conversions"
                >
                  <ToggleSwitch
                    checked={settings.output.preserve_metadata}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, preserve_metadata: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Send Source to Recycle Bin"
                  description="Safely move original files to the Windows Recycle Bin after successful conversion"
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
                  title="Overwrite Existing Files"
                  description="Replace existing files if an output file with the same name already exists"
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


          {/* TAB 5: ENGINES */}
          {activeTab === "engines" && (
            <div className="w-full">
              <SettingSection first title="Multimedia Conversion Engine">
                <SettingRow
                  title="FFmpeg Sidecar Status"
                  description={
                    ffmpegStatus?.installed
                      ? ffmpegStatus.version || "Installed and operational"
                      : "Optional sidecar engine for video, audio, and high-quality GIF creation"
                  }
                >
                  <span
                    className={`text-xs font-medium px-2.5 py-1 rounded-md ${
                      ffmpegStatus?.installed
                        ? "text-emerald-400 bg-emerald-500/10"
                        : "text-amber-400 bg-amber-500/10"
                    }`}
                  >
                    {ffmpegStatus?.installed ? "Installed" : "Not Found"}
                  </span>
                </SettingRow>
              </SettingSection>
            </div>
          )}

          {/* TAB 7: HISTORY */}
          {activeTab === "history" && (
            <div className="w-full">
              <SettingSection first>
                <SettingRow
                  title="SQLite Database History"
                  description={`${historyCount} completed file operation records stored in local database`}
                >
                  <WheelButton
                    variant="danger"
                    disabled={historyCount === 0}
                    onClick={handleClearHistory}
                  >
                    Clear History
                  </WheelButton>
                </SettingRow>
              </SettingSection>

              {historyList.length > 0 && (
                <SettingSection title="Recent Activity">
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
                          className="p-2.5 bg-[#2a2929] border border-white/[0.06] rounded-lg flex items-center justify-between gap-3 text-xs"
                        >
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <span
                                className="font-medium text-neutral-200 truncate"
                                title={outPath}
                              >
                                {fileName}
                              </span>
                              <span className="text-[10px] text-neutral-400 bg-white/[0.06] px-1.5 py-0.5 rounded font-mono shrink-0">
                                {item.action_id}
                              </span>
                            </div>
                            <div className="text-[11px] text-neutral-400 mt-0.5 truncate">
                              {timeStr} • {outPath}
                            </div>
                          </div>

                          <div className="flex items-center gap-1 shrink-0">
                            {outPath && (
                              <button
                                onClick={() =>
                                  invoke("open_in_folder", { path: outPath })
                                }
                                className="p-1.5 text-neutral-400 hover:text-white bg-white/[0.04] hover:bg-white/[0.08] rounded-md transition-colors cursor-default"
                                title="Reveal in File Explorer"
                              >
                                <FolderOpen size={13} />
                              </button>
                            )}
                            <button
                              onClick={() => handleDeleteHistoryItem(item.id)}
                              className="p-1.5 text-neutral-400 hover:text-red-400 bg-white/[0.04] hover:bg-red-500/10 rounded-md transition-colors cursor-default"
                              title="Delete record"
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

          {/* TAB 7: ABOUT */}
          {activeTab === "about" && (
            <div className="w-full">
              <SettingSection first>
                <div className="flex flex-col items-center justify-center p-6 bg-[#2a2929] border border-white/[0.06] rounded-lg mb-4 text-center">
                  <img
                    src="/logo.svg"
                    alt="Lime"
                    className="w-20 h-20 object-contain mb-3 drop-shadow-md select-none pointer-events-none"
                  />
                  <h2 className="text-base font-semibold text-white tracking-wide">Lime</h2>
                  <p className="text-xs text-neutral-400 mt-1">High-performance file toolkit for Windows</p>
                </div>

                <div className="p-4 bg-[#2a2929] border border-white/[0.06] rounded-lg space-y-3">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Application</span>
                    <span className="text-neutral-200 font-medium">Lime — File Toolkit for Windows</span>
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <span className="text-neutral-400">Version</span>
                      <div className="font-mono text-neutral-200 mt-0.5 flex items-center gap-2">
                        <span>{appVersion}</span>
                        {updateStatus === "up_to_date" && (
                          <span className="text-[11px] text-emerald-400 font-sans flex items-center gap-1">
                            <Check size={12} className="inline" /> Up to date
                          </span>
                        )}
                        {updateStatus === "ready" && (
                          <span className="text-[11px] text-lime-400 font-sans font-medium">
                            • v{updateVersion} installed & ready
                          </span>
                        )}
                        {updateStatus === "available" && (
                          <span className="text-[11px] text-amber-400 font-sans font-medium">
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
                    <div className="text-[11px] text-red-400 bg-red-500/10 border border-red-500/20 rounded px-2.5 py-1.5">
                      {updateError || "Could not check for updates."}
                    </div>
                  )}

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <div className="text-neutral-400">Update Channel</div>
                      <div className="text-neutral-300 text-[11px] mt-0.5">
                        {settings?.general?.include_prereleases ?? true ? (
                          <span className="text-amber-400 font-medium">Pre-release Channel (Alpha & Beta)</span>
                        ) : (
                          <span className="text-emerald-400 font-medium">Stable Channel</span>
                        )}
                      </div>
                    </div>
                    <ToggleSwitch
                      checked={settings?.general?.include_prereleases ?? true}
                      onChange={(checked) => {
                        if (settings) {
                          const updated = {
                            ...settings,
                            general: { ...settings.general, include_prereleases: checked },
                          };
                          handleSaveSettings(updated);
                        }
                      }}
                    />
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <div>
                      <div className="text-neutral-400">Local Data Storage</div>
                      <div className="font-mono text-neutral-400 text-[11px] truncate max-w-xs mt-0.5">
                        %LOCALAPPDATA%\Lime\
                      </div>
                    </div>
                    <WheelButton
                      variant="secondary"
                      onClick={() => invoke("open_data_folder")}
                    >
                      <FolderOpen size={12} className="inline mr-1" />
                      Open Folder
                    </WheelButton>
                  </div>
                </div>
              </SettingSection>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

