import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Settings as SettingsIcon,
  SlidersHorizontal,
  LayoutGrid,
  FolderOutput,
  Sparkles,
  Cpu,
  History,
  Info,
  Search,
  Check,
  Download,
  Trash2,
  FileCheck2,
} from "lucide-react";
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
  | "presets"
  | "engines"
  | "history"
  | "about";

interface NavItem {
  id: SettingsNavId;
  label: string;
  icon: React.ReactNode;
}

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<SettingsNavId>("general");
  const [searchQuery, setSearchQuery] = useState("");
  const [settings, setSettings] = useState<any>(null);
  const [historyCount, setHistoryCount] = useState<number>(0);
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
        const hist = await invoke<any[]>("get_history", { limit: 100 });
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

  const handleToggleContextMenu = async (enabled: boolean) => {
    try {
      await invoke("set_explorer_context_menu", { enabled });
      if (settings) {
        const updated = {
          ...settings,
          general: { ...settings.general, explorer_context_menu: enabled },
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

  const handleDeleteModel = async () => {
    try {
      await invoke("delete_rmbg_model");
      const rmbg = await invoke<any>("get_rmbg_model_status");
      setRmbgStatus(rmbg);
    } catch (e) {
      console.error("Failed to delete model", e);
    }
  };

  const navItems: NavItem[] = [
    {
      id: "general",
      label: "General",
      icon: <SettingsIcon size={14} />,
    },
    {
      id: "trigger",
      label: "Trigger & Drag",
      icon: <SlidersHorizontal size={14} />,
    },
    {
      id: "wheel_ui",
      label: "Radial Wheel",
      icon: <LayoutGrid size={14} />,
    },
    {
      id: "output",
      label: "Output & Files",
      icon: <FolderOutput size={14} />,
    },
    {
      id: "presets",
      label: "Presets & Workflows",
      icon: <Sparkles size={14} />,
    },
    {
      id: "engines",
      label: "Engines & AI",
      icon: <Cpu size={14} />,
    },
    {
      id: "history",
      label: "History",
      icon: <History size={14} />,
    },
    {
      id: "about",
      label: "About Wheel",
      icon: <Info size={14} />,
    },
  ];

  const filteredNavItems = useMemo(() => {
    if (!searchQuery.trim()) return navItems;
    return navItems.filter((item) =>
      item.label.toLowerCase().includes(searchQuery.toLowerCase())
    );
  }, [navItems, searchQuery]);

  if (!settings) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-[#181818] text-neutral-400 select-none">
        <span className="text-xs">Loading Settings...</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen w-screen bg-[#181818] text-neutral-200 font-sans select-none overflow-hidden rounded-xl border border-white/[0.08] shadow-2xl">
      {/* Top Titlebar */}
      <header
        data-tauri-drag-region
        className="h-10 flex items-center justify-between px-4 border-b border-white/[0.06] bg-[#181818] select-none cursor-move shrink-0 z-20"
      >
        <div className="flex items-center gap-3">
          <span className="text-[13px] font-medium text-neutral-200 pointer-events-none">
            Wheel Settings
          </span>
        </div>

        <div className="flex items-center gap-3">
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
        <aside className="w-56 border-r border-white/[0.06] bg-[#181818] flex flex-col p-2.5 shrink-0 overflow-y-auto">
          {/* Search Box */}
          <div className="relative mb-2.5">
            <Search
              size={13}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-neutral-500 pointer-events-none"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search settings..."
              className="w-full pl-8 pr-3 py-1.5 bg-[#222222] border border-white/[0.06] focus:border-white/20 text-xs rounded-lg text-neutral-200 placeholder:text-neutral-500 outline-none transition-colors"
            />
          </div>

          {/* Navigation Items */}
          <nav className="flex flex-col gap-0.5">
            {filteredNavItems.map((item) => {
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id)}
                  className={`flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs transition-colors cursor-pointer text-left ${
                    isActive
                      ? "bg-white/[0.1] text-white font-medium"
                      : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
                  }`}
                >
                  <span className={isActive ? "text-white" : "text-neutral-400"}>
                    {item.icon}
                  </span>
                  <span>{item.label}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        {/* Right Settings Content Panel */}
        <main className="flex-1 bg-[#1e1e1e] p-7 overflow-y-auto">
          {/* TAB 1: GENERAL */}
          {activeTab === "general" && (
            <div className="max-w-xl">
              <SettingSection title="System Startup & Tray">
                <SettingRow
                  title="Launch at Windows Login"
                  description="Start Wheel automatically in the background when signing into Windows"
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
                  description="Keep Wheel running in the background tray when tool windows are closed"
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

                <SettingRow
                  title="Automatic Updates"
                  description="Check for new application releases automatically in the background"
                >
                  <ToggleSwitch
                    checked={settings.general.auto_update}
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

              <SettingSection title="Windows Explorer Integration">
                <SettingRow
                  title="Explorer Context Menu"
                  description="Add 'Open with Wheel' to Windows Explorer right-click context menus"
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
            <div className="max-w-xl">
              <SettingSection title="Gesture Activation">
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
                    <span className="font-mono text-xs text-neutral-300 w-12 text-right">
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
                      className="w-28 accent-[#ff6339] cursor-pointer"
                    />
                  </div>
                </SettingRow>

                <SettingRow
                  title="Drop Confirmation Timeout"
                  description="Milliseconds to wait for Windows OLE drop confirmation before cancelling"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-300 w-16 text-right">
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
                      className="w-28 accent-[#ff6339] cursor-pointer"
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
                  title="Pause Wheel Trigger"
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
            <div className="max-w-xl">
              <SettingSection title="Wheel Layout & Display">
                <SettingRow
                  title="Interface Theme"
                  description="Color appearance of the radial wheel overlay"
                >
                  <SegmentedControl
                    value={settings.wheel_ui.theme}
                    options={[
                      { label: "System", value: "system" },
                      { label: "Dark", value: "dark" },
                      { label: "Light", value: "light" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, theme: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Default Start Page"
                  description="Which radial wheel page is displayed when a drag starts"
                >
                  <SegmentedControl
                    value={settings.wheel_ui.start_page}
                    options={[
                      { label: "Convert (Page 1)", value: "convert" },
                      { label: "Tools (Page 2)", value: "tools" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, start_page: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </SettingRow>

                <SettingRow
                  title="Wheel Diameter"
                  description="Outer pixel diameter of the circular radial wheel"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-300 w-14 text-right">
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
                      className="w-28 accent-[#ff6339] cursor-pointer"
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
                  title="Audio Tick Effects"
                  description="Play subtle mechanical tick sounds during wedge hover"
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
            <div className="max-w-xl">
              <SettingSection title="File Destination">
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
                      placeholder="C:\Users\...\Pictures\Wheel"
                      className="w-56 px-2.5 py-1 text-xs bg-[#242424] border border-white/[0.08] focus:border-white/30 rounded-lg text-white font-mono outline-none"
                    />
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
                    className="w-32 px-2.5 py-1 text-xs bg-[#242424] border border-white/[0.08] focus:border-white/30 rounded-lg text-white font-mono outline-none text-right"
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

          {/* TAB 5: PRESETS & WORKFLOWS */}
          {activeTab === "presets" && (
            <div className="max-w-xl">
              <SettingSection title="Multi-Step Automated Recipes">
                <div className="space-y-2.5">
                  {(settings.presets || []).map((preset: any, index: number) => (
                    <div
                      key={preset.id}
                      className="p-3.5 bg-[#242424] border border-white/[0.06] rounded-xl flex items-center justify-between gap-4"
                    >
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-semibold text-neutral-200">
                            {preset.name}
                          </span>
                          <span className="text-[10px] text-neutral-400 bg-white/[0.06] px-1.5 py-0.5 rounded font-mono">
                            {preset.steps.length} steps
                          </span>
                        </div>
                        <p className="text-[11px] text-neutral-400 mt-1 leading-relaxed">
                          {preset.description}
                        </p>
                      </div>

                      <ToggleSwitch
                        checked={preset.enabled}
                        onChange={(checked) => {
                          const updatedPresets = [...settings.presets];
                          updatedPresets[index] = { ...preset, enabled: checked };
                          const updated = { ...settings, presets: updatedPresets };
                          handleSaveSettings(updated);
                        }}
                      />
                    </div>
                  ))}
                </div>
              </SettingSection>
            </div>
          )}

          {/* TAB 6: ENGINES & AI */}
          {activeTab === "engines" && (
            <div className="max-w-xl">
              <SettingSection title="AI Background Removal Model">
                <SettingRow
                  title="RMBG-1.4 Neural Model"
                  description={
                    rmbgStatus?.installed
                      ? `Installed locally (${(
                          (rmbgStatus.file_size_bytes || 176000000) /
                          (1024 * 1024)
                        ).toFixed(1)} MB)`
                      : "Local neural network for high-fidelity background removal"
                  }
                >
                  {rmbgStatus?.installed ? (
                    <div className="flex items-center gap-2">
                      <span className="text-emerald-400 font-medium text-xs flex items-center gap-1.5 bg-emerald-500/10 px-2.5 py-1 rounded-lg">
                        <FileCheck2 size={13} />
                        Ready
                      </span>
                      <button
                        onClick={handleDeleteModel}
                        className="p-1.5 text-neutral-400 hover:text-red-400 bg-white/[0.04] hover:bg-red-500/10 rounded-lg transition-colors cursor-pointer"
                        title="Delete model file from disk"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  ) : (
                    <WheelButton
                      variant="primary"
                      disabled={isDownloadingModel}
                      onClick={handleDownloadModel}
                    >
                      <Download size={12} className="inline mr-1" />
                      {isDownloadingModel ? "Downloading (~176 MB)..." : "Download Model (176 MB)"}
                    </WheelButton>
                  )}
                </SettingRow>
              </SettingSection>

              <SettingSection title="Multimedia Conversion Engine">
                <SettingRow
                  title="FFmpeg Sidecar Status"
                  description={
                    ffmpegStatus?.installed
                      ? ffmpegStatus.version || "Installed and operational"
                      : "Optional sidecar engine for video, audio, and high-quality GIF creation"
                  }
                >
                  <span
                    className={`text-xs font-medium px-2.5 py-1 rounded-lg ${
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
            <div className="max-w-xl">
              <SettingSection title="Conversion & Job History">
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
            </div>
          )}

          {/* TAB 8: ABOUT */}
          {activeTab === "about" && (
            <div className="max-w-xl">
              <SettingSection title="About Wheel">
                <div className="p-4 bg-[#242424] border border-white/[0.06] rounded-xl space-y-3">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Application</span>
                    <span className="text-neutral-200 font-medium">Wheel — File Toolkit for Windows</span>
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Version</span>
                    <span className="font-mono text-neutral-200">v0.1.0</span>
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Local Data Storage</span>
                    <span className="font-mono text-neutral-400 text-[11px] truncate max-w-xs">
                      %LOCALAPPDATA%\Wheel\
                    </span>
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
