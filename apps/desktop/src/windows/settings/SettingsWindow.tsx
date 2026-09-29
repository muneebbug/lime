import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Sliders,
  MousePointer,
  Compass,
  Layers,
  Wand2,
  Info,
  X,
  Check,
  Trash2,
  Sparkles,
  Download,
  FileCheck2,
} from "lucide-react";

type SettingsTab = "general" | "trigger" | "wheel" | "actions" | "presets" | "about";

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [settings, setSettings] = useState<any>(null);
  const [actions, setActions] = useState<any[]>([]);
  const [historyCount, setHistoryCount] = useState<number>(0);
  const [contextMenuEnabled, setContextMenuEnabled] = useState<boolean>(true);
  const [ffmpegStatus, setFfmpegStatus] = useState<any>(null);
  const [rmbgStatus, setRmbgStatus] = useState<any>(null);
  const [isSaved, setIsSaved] = useState<boolean>(false);
  const [isDownloadingModel, setIsDownloadingModel] = useState<boolean>(false);

  const appWindow = getCurrentWebviewWindow();

  useEffect(() => {
    appWindow.unminimize().catch(() => {});
    appWindow.show().catch(() => {});
    appWindow.setFocus().catch(() => {});
  }, [appWindow]);

  // Load initial settings and statuses
  useEffect(() => {
    async function loadData() {
      try {
        const s = await invoke<any>("get_settings");
        setSettings(s);
      } catch (e) {
        console.error("Failed to load settings", e);
      }

      try {
        const acts = await invoke<any[]>("get_actions");
        setActions(acts);
      } catch (e) {
        console.error("Failed to load actions", e);
      }

      try {
        const hist = await invoke<any[]>("get_history", { limit: 100 });
        setHistoryCount(hist.length);
      } catch (e) {
        console.error("Failed to load history count", e);
      }

      try {
        const cm = await invoke<boolean>("is_explorer_context_menu_enabled");
        setContextMenuEnabled(cm);
      } catch (e) {
        console.error("Failed to check context menu status", e);
      }

      try {
        const ff = await invoke<any>("get_ffmpeg_status");
        setFfmpegStatus(ff);
      } catch (e) {
        console.error("Failed to check ffmpeg status", e);
      }

      try {
        const rmbg = await invoke<any>("get_rmbg_model_status");
        setRmbgStatus(rmbg);
      } catch (e) {
        console.error("Failed to check rmbg status", e);
      }
    }
    loadData();
  }, []);

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

  const handleToggleContextMenu = async () => {
    const nextState = !contextMenuEnabled;
    try {
      await invoke("set_explorer_context_menu", { enabled: nextState });
      setContextMenuEnabled(nextState);
      if (settings) {
        const updated = {
          ...settings,
          general: { ...settings.general, explorer_context_menu: nextState },
        };
        handleSaveSettings(updated);
      }
    } catch (e) {
      console.error("Failed to update context menu registration", e);
    }
  };

  const handleClearHistory = async () => {
    try {
      await invoke("clear_history");
      setHistoryCount(0);
    } catch (e) {
      console.error("Failed to clear history", e);
    }
  };

  const handleDownloadModel = async () => {
    setIsDownloadingModel(true);
    try {
      await invoke("download_rmbg_model");
      const rmbg = await invoke<any>("get_rmbg_model_status");
      setRmbgStatus(rmbg);
    } catch (e) {
      console.error("Failed to download model", e);
    } finally {
      setIsDownloadingModel(false);
    }
  };

  const handleClose = async () => {
    try {
      await appWindow.close();
    } catch (e) {
      console.error("Failed to close window", e);
    }
  };

  if (!settings) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-zinc-950 text-zinc-400">
        <span className="text-sm">Loading Wheel Settings...</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen w-screen bg-zinc-950 text-zinc-100 font-sans select-none overflow-hidden rounded-2xl border border-white/10 shadow-2xl backdrop-blur-2xl">
      {/* Background warm ambient accent */}
      <div className="absolute inset-0 pointer-events-none bg-[radial-gradient(ellipse_at_30%_-20%,rgba(249,115,22,0.15),transparent_65%)]" />

      {/* Header Bar */}
      <header
        data-tauri-drag-region
        className="relative z-10 flex items-center justify-between px-5 py-3 border-b border-white/10 bg-zinc-900/50 cursor-move"
      >
        <div className="flex items-center gap-3 pointer-events-none">
          <div className="w-6 h-6 rounded-lg bg-orange-500/20 border border-orange-500/40 flex items-center justify-center text-orange-400">
            <Compass size={14} />
          </div>
          <span className="text-sm font-semibold text-zinc-200">Wheel Settings</span>
        </div>

        <div className="flex items-center gap-3">
          {isSaved && (
            <span className="flex items-center gap-1.5 text-xs text-emerald-400 font-medium animate-pulse">
              <Check size={13} />
              Saved
            </span>
          )}
          <button
            onClick={handleClose}
            className="w-7 h-7 rounded-full flex items-center justify-center bg-white/5 hover:bg-red-500/20 hover:text-red-400 border border-white/10 transition-colors text-zinc-400 cursor-pointer"
            title="Close (Esc)"
          >
            <X size={14} />
          </button>
        </div>
      </header>

      {/* Main Body: Sidebar + Tab Content */}
      <div className="flex flex-1 overflow-hidden relative z-10">
        {/* Navigation Sidebar */}
        <aside className="w-56 border-r border-white/10 bg-zinc-900/30 p-3 flex flex-col gap-1">
          <button
            onClick={() => setActiveTab("general")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "general"
                ? "bg-orange-500 text-white shadow-md shadow-orange-500/20"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
            }`}
          >
            <Sliders size={15} />
            <span>General</span>
          </button>

          <button
            onClick={() => setActiveTab("trigger")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "trigger"
                ? "bg-orange-500 text-white shadow-md shadow-orange-500/20"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
            }`}
          >
            <MousePointer size={15} />
            <span>Trigger & Drag</span>
          </button>

          <button
            onClick={() => setActiveTab("wheel")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "wheel"
                ? "bg-orange-500 text-white shadow-md shadow-orange-500/20"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
            }`}
          >
            <Compass size={15} />
            <span>Radial Wheel</span>
          </button>

          <button
            onClick={() => setActiveTab("actions")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "actions"
                ? "bg-orange-500 text-white shadow-md shadow-orange-500/20"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
            }`}
          >
            <Layers size={15} />
            <span>Actions & Formats</span>
          </button>

          <button
            onClick={() => setActiveTab("presets")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "presets"
                ? "bg-orange-500 text-white shadow-md shadow-orange-500/20"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
            }`}
          >
            <Wand2 size={15} />
            <span>Presets & Chains</span>
          </button>

          <div className="flex-1" />

          <button
            onClick={() => setActiveTab("about")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-colors cursor-pointer ${
              activeTab === "about"
                ? "bg-white/10 text-white"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-white/5"
            }`}
          >
            <Info size={15} />
            <span>About & Diagnostics</span>
          </button>
        </aside>

        {/* Tab Content Panel */}
        <main className="flex-1 p-6 overflow-y-auto">
          {/* 1. GENERAL TAB */}
          {activeTab === "general" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">General Preferences</h2>
                <p className="text-xs text-zinc-400">Configure startup behavior, Explorer menus, and file output handling.</p>
              </div>

              {/* Startup & Shell */}
              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl p-4 space-y-4">
                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Start Wheel at Login</span>
                    <p className="text-xs text-zinc-500">Automatically run in the background upon Windows boot.</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.general.launch_at_login}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, launch_at_login: e.target.checked },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>

                <div className="h-px bg-white/5" />

                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Windows Explorer Context Menu</span>
                    <p className="text-xs text-zinc-500">Show "Open with Wheel" in Windows 10/11 right-click menus.</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={contextMenuEnabled}
                    onChange={handleToggleContextMenu}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>
              </div>

              {/* Output Directory Policy */}
              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl p-4 space-y-4">
                <h3 className="text-sm font-medium text-zinc-200">Output Location Policy</h3>

                <div className="grid grid-cols-2 gap-2">
                  <button
                    onClick={() => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, policy: "next_to_source" },
                      };
                      handleSaveSettings(updated);
                    }}
                    className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                      settings.output.policy === "next_to_source"
                        ? "border-orange-500/60 bg-orange-500/10 text-orange-200"
                        : "border-white/5 bg-zinc-950/40 text-zinc-400 hover:border-white/10"
                    }`}
                  >
                    <span className="block text-xs font-semibold">Next to Source File</span>
                    <span className="block text-[11px] text-zinc-500 mt-1">Saves in the same folder with optional suffix</span>
                  </button>

                  <button
                    onClick={() => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, policy: "fixed_folder" },
                      };
                      handleSaveSettings(updated);
                    }}
                    className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                      settings.output.policy === "fixed_folder"
                        ? "border-orange-500/60 bg-orange-500/10 text-orange-200"
                        : "border-white/5 bg-zinc-950/40 text-zinc-400 hover:border-white/10"
                    }`}
                  >
                    <span className="block text-xs font-semibold">Fixed Folder</span>
                    <span className="block text-[11px] text-zinc-500 mt-1">Directs all conversions into a designated directory</span>
                  </button>
                </div>

                {/* Suffix Input */}
                <div>
                  <label className="block text-xs font-medium text-zinc-400 mb-1.5">File Name Suffix</label>
                  <input
                    type="text"
                    value={settings.output.suffix}
                    placeholder=".converted (leave blank for clean name)"
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, suffix: e.target.value },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-full px-3 py-1.5 text-xs bg-zinc-950 border border-white/10 rounded-xl text-white outline-none focus:border-orange-500"
                  />
                </div>

                <div className="h-px bg-white/5" />

                {/* Recycle Bin & Overwrite Source */}
                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Send Source to Recycle Bin</span>
                    <p className="text-xs text-zinc-500">Recycle original file after successful conversion (reversible).</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.output.recycle_source}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, recycle_source: e.target.checked },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>
              </div>
            </div>
          )}

          {/* 2. TRIGGER TAB */}
          {activeTab === "trigger" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Trigger & Gesture</h2>
                <p className="text-xs text-zinc-400">Configure mouse drag thresholds and modifier key activation.</p>
              </div>

              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl p-4 space-y-4">
                <div>
                  <label className="block text-xs font-medium text-zinc-400 mb-2">Activation Modifier Key</label>
                  <div className="grid grid-cols-4 gap-2">
                    {["shift", "ctrl", "alt", "none"].map((mod) => (
                      <button
                        key={mod}
                        onClick={() => {
                          const updated = {
                            ...settings,
                            trigger: { ...settings.trigger, modifier: mod },
                          };
                          handleSaveSettings(updated);
                        }}
                        className={`py-2 px-3 rounded-xl text-xs font-medium border capitalize cursor-pointer transition-all ${
                          settings.trigger.modifier === mod
                            ? "bg-orange-500 text-white border-orange-500 shadow-md shadow-orange-500/20"
                            : "bg-zinc-950/40 border-white/5 text-zinc-400 hover:border-white/10"
                        }`}
                      >
                        {mod === "none" ? "None (Any Drag)" : mod}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="h-px bg-white/5" />

                <div>
                  <div className="flex justify-between text-xs mb-1.5">
                    <span className="font-medium text-zinc-200">Movement Threshold</span>
                    <span className="text-orange-400 font-mono">{settings.trigger.movement_threshold_px} px</span>
                  </div>
                  <input
                    type="range"
                    min="3"
                    max="24"
                    value={settings.trigger.movement_threshold_px}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, movement_threshold_px: Number(e.target.value) },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-full accent-orange-500 cursor-pointer"
                  />
                  <p className="text-[11px] text-zinc-500 mt-1">Minimum drag distance in pixels before Wheel triggers.</p>
                </div>

                <div className="h-px bg-white/5" />

                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Pause Wheel Globally</span>
                    <p className="text-xs text-zinc-500">Temporarily disable the radial overlay trigger without exiting.</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.trigger.paused}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, paused: e.target.checked },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>
              </div>
            </div>
          )}

          {/* 3. WHEEL TAB */}
          {activeTab === "wheel" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Radial Wheel Geometry</h2>
                <p className="text-xs text-zinc-400">Tailor the visual dimensions, slot density, and behavior of the overlay.</p>
              </div>

              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl p-4 space-y-4">
                <div>
                  <label className="block text-xs font-medium text-zinc-400 mb-2">Slot Count per Page</label>
                  <div className="grid grid-cols-4 gap-2">
                    {[6, 8, 10, 12].map((count) => (
                      <button
                        key={count}
                        onClick={() => {
                          const updated = {
                            ...settings,
                            wheel_ui: { ...settings.wheel_ui, slot_count: count },
                          };
                          handleSaveSettings(updated);
                        }}
                        className={`py-2 px-3 rounded-xl text-xs font-medium border cursor-pointer transition-all ${
                          settings.wheel_ui.slot_count === count
                            ? "bg-orange-500 text-white border-orange-500 shadow-md shadow-orange-500/20"
                            : "bg-zinc-950/40 border-white/5 text-zinc-400 hover:border-white/10"
                        }`}
                      >
                        {count} Wedges
                      </button>
                    ))}
                  </div>
                </div>

                <div className="h-px bg-white/5" />

                <div>
                  <div className="flex justify-between text-xs mb-1.5">
                    <span className="font-medium text-zinc-200">Wheel Diameter</span>
                    <span className="text-orange-400 font-mono">{settings.wheel_ui.size} px</span>
                  </div>
                  <input
                    type="range"
                    min="260"
                    max="440"
                    step="10"
                    value={settings.wheel_ui.size}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, size: Number(e.target.value) },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-full accent-orange-500 cursor-pointer"
                  />
                </div>

                <div className="h-px bg-white/5" />

                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Context Filtering</span>
                    <p className="text-xs text-zinc-500">Automatically dim or hide formats incompatible with dragged files.</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.wheel_ui.context_filter_enabled}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, context_filter_enabled: e.target.checked },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>

                <div className="h-px bg-white/5" />

                <label className="flex items-center justify-between cursor-pointer">
                  <div>
                    <span className="text-sm font-medium text-zinc-200">Audio Haptic Feedback</span>
                    <p className="text-xs text-zinc-500">Play subtle tick sound when hovering over radial wedges.</p>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.wheel_ui.sound_enabled}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, sound_enabled: e.target.checked },
                      };
                      handleSaveSettings(updated);
                    }}
                    className="w-4 h-4 accent-orange-500 rounded cursor-pointer"
                  />
                </label>
              </div>
            </div>
          )}

          {/* 4. ACTIONS TAB */}
          {activeTab === "actions" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Actions & Formats</h2>
                <p className="text-xs text-zinc-400">Manage available tools and conversion formats displayed on the wheel.</p>
              </div>

              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl divide-y divide-white/5">
                {actions.map((act) => (
                  <div key={act.id} className="p-3.5 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className="w-8 h-8 rounded-lg bg-white/5 border border-white/10 flex items-center justify-center text-orange-400 text-xs font-bold">
                        {act.title.slice(0, 3).toUpperCase()}
                      </div>
                      <div>
                        <span className="text-xs font-semibold text-zinc-200">{act.title}</span>
                        <span className="block text-[11px] text-zinc-500 capitalize">{act.category} • {act.kind}</span>
                      </div>
                    </div>

                    <span className="text-xs text-emerald-400 font-medium">Active</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 5. PRESETS TAB */}
          {activeTab === "presets" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Automated Action Chains</h2>
                <p className="text-xs text-zinc-400">Sequential multi-step recipes executed in a single drop.</p>
              </div>

              <div className="grid gap-3">
                {(settings.presets || []).map((preset: any) => (
                  <div
                    key={preset.id}
                    className="bg-zinc-900/40 border border-white/5 hover:border-orange-500/40 rounded-2xl p-4 transition-all"
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex items-center gap-2.5">
                        <div className="w-7 h-7 rounded-lg bg-orange-500/20 text-orange-400 flex items-center justify-center">
                          <Sparkles size={14} />
                        </div>
                        <div>
                          <span className="text-sm font-semibold text-zinc-200">{preset.name}</span>
                          <p className="text-xs text-zinc-500">{preset.description}</p>
                        </div>
                      </div>

                      <span className="text-[11px] px-2 py-0.5 rounded-full bg-white/5 border border-white/10 text-zinc-400">
                        {preset.steps.length} Steps
                      </span>
                    </div>

                    <div className="mt-3 flex items-center gap-1.5 overflow-x-auto text-[11px] text-zinc-400">
                      {preset.steps.map((st: any, i: number) => (
                        <React.Fragment key={i}>
                          <span className="px-2 py-1 bg-black/40 rounded border border-white/5 font-mono text-orange-300">
                            {st.action_id}
                          </span>
                          {i < preset.steps.length - 1 && <span className="text-zinc-600">→</span>}
                        </React.Fragment>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 6. ABOUT TAB */}
          {activeTab === "about" && (
            <div className="max-w-xl space-y-6">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">System & Diagnostics</h2>
                <p className="text-xs text-zinc-400">Engine statuses, local history persistence, and neural models.</p>
              </div>

              {/* Status summary */}
              <div className="bg-zinc-900/40 border border-white/5 rounded-2xl p-4 space-y-3">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-zinc-400">Wheel Core Engine</span>
                  <span className="font-mono text-zinc-200">v0.1.0 (Windows x86_64)</span>
                </div>

                <div className="h-px bg-white/5" />

                <div className="flex items-center justify-between text-xs">
                  <span className="text-zinc-400">SQLite History DB</span>
                  <span className="font-mono text-zinc-200">{historyCount} Records</span>
                </div>

                <div className="h-px bg-white/5" />

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="text-zinc-400">RMBG-1.4 Neural Model</span>
                    <span className="block text-[11px] text-zinc-500">
                      {rmbgStatus?.installed ? "Installed in Local AppData" : "Not downloaded (using fallback)"}
                    </span>
                  </div>
                  {rmbgStatus?.installed ? (
                    <span className="text-emerald-400 font-medium flex items-center gap-1">
                      <FileCheck2 size={13} />
                      Ready
                    </span>
                  ) : (
                    <button
                      onClick={handleDownloadModel}
                      disabled={isDownloadingModel}
                      className="px-2.5 py-1 bg-orange-500 text-white rounded-lg text-xs font-medium flex items-center gap-1 cursor-pointer hover:bg-orange-600 disabled:opacity-50"
                    >
                      <Download size={12} />
                      {isDownloadingModel ? "Downloading..." : "Download (176MB)"}
                    </button>
                  )}
                </div>

                <div className="h-px bg-white/5" />

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="text-zinc-400">FFmpeg Transcoder</span>
                    <span className="block text-[11px] text-zinc-500 truncate max-w-xs">
                      {ffmpegStatus?.installed ? ffmpegStatus.version || "Installed" : "Not found in PATH or sidecar"}
                    </span>
                  </div>
                  <span className={`font-medium ${ffmpegStatus?.installed ? "text-emerald-400" : "text-amber-400"}`}>
                    {ffmpegStatus?.installed ? "Detected" : "Optional"}
                  </span>
                </div>
              </div>

              {/* Maintenance Actions */}
              <div className="flex items-center gap-3">
                <button
                  onClick={handleClearHistory}
                  disabled={historyCount === 0}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-white/5 hover:bg-red-500/10 text-zinc-400 hover:text-red-400 border border-white/10 text-xs font-medium transition-colors cursor-pointer disabled:opacity-40"
                >
                  <Trash2 size={13} />
                  <span>Clear History</span>
                </button>
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
