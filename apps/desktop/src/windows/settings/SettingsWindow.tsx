import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Settings as SettingsIcon,
  SlidersHorizontal,
  LayoutGrid,
  Keyboard,
  Cloud,
  Wrench,
  Users,
  Info,
  Sparkles,
  AppWindow,
  Globe,
  Search,
  ChevronLeft,
  ChevronRight,
  Check,
  Download,
  FileCheck2,
  Moon,
  Sun,
} from "lucide-react";
import {
  RaycastCaptionButtons,
  RaycastToggle,
  RaycastHotkeyPill,
  RaycastSelect,
  RaycastSegmented,
  RaycastButton,
  RaycastRow,
  RaycastSection,
} from "../../ui/RaycastUI";

type SettingsNavId =
  | "general"
  | "launcher"
  | "shortcuts"
  | "keyboard"
  | "cloud"
  | "advanced"
  | "orgs"
  | "about"
  | "ai"
  | "applications"
  | "browser";

interface NavItem {
  id: SettingsNavId;
  label: string;
  icon: React.ReactNode;
  badge?: string;
  isSpecial?: boolean;
}

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<SettingsNavId>("general");
  const [searchQuery, setSearchQuery] = useState("");
  const [settings, setSettings] = useState<any>(null);
  const [actions, setActions] = useState<any[]>([]);
  const [historyCount, setHistoryCount] = useState<number>(0);
  const [contextMenuEnabled, setContextMenuEnabled] = useState<boolean>(true);
  const [ffmpegStatus, setFfmpegStatus] = useState<any>(null);
  const [rmbgStatus, setRmbgStatus] = useState<any>(null);
  const [isSaved, setIsSaved] = useState<boolean>(false);
  const [isDownloadingModel, setIsDownloadingModel] = useState<boolean>(false);
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);

  const [followSystem, setFollowSystem] = useState(true);
  const [darkTheme, setDarkTheme] = useState("raycast_dark");
  const [lightTheme, setLightTheme] = useState("raycast_light");
  const [interfaceSize, setInterfaceSize] = useState("normal");
  const [showTaskbar, setShowTaskbar] = useState(false);

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

  const handleToggleContextMenu = async (enabled: boolean) => {
    try {
      await invoke("set_explorer_context_menu", { enabled });
      setContextMenuEnabled(enabled);
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

  const navItems: NavItem[] = [
    {
      id: "general",
      label: "General",
      icon: <SettingsIcon size={15} />,
    },
    {
      id: "launcher",
      label: "Launcher & Drag",
      icon: <SlidersHorizontal size={15} />,
    },
    {
      id: "shortcuts",
      label: "Radial Wheel",
      icon: <LayoutGrid size={15} />,
    },
    {
      id: "keyboard",
      label: "Keyboard & Formats",
      icon: <Keyboard size={15} />,
    },
    {
      id: "cloud",
      label: "Presets & Workflows",
      icon: <Cloud size={15} />,
      badge: "Pro",
    },
    {
      id: "advanced",
      label: "Advanced",
      icon: <Wrench size={15} />,
    },
    {
      id: "orgs",
      label: "Extensions",
      icon: <Users size={15} />,
    },
    {
      id: "about",
      label: "About",
      icon: <Info size={15} />,
    },
  ];

  const specialNavItems: NavItem[] = [
    {
      id: "ai",
      label: "AI & Neural Models",
      icon: <Sparkles size={15} className="text-white" />,
      badge: "Pro",
      isSpecial: true,
    },
    {
      id: "applications",
      label: "Applications",
      icon: <AppWindow size={15} className="text-emerald-400" />,
    },
    {
      id: "browser",
      label: "Browser & Media",
      icon: <Globe size={15} className="text-blue-400" />,
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
        <div className="flex items-center gap-4">
          <span className="text-[13px] font-medium text-neutral-300 pointer-events-none">
            Settings
          </span>

          {/* Navigation arrows like in Raycast on Windows */}
          <div className="flex items-center gap-0.5 text-neutral-500">
            <button
              onClick={() => setActiveTab("general")}
              className="p-1 hover:text-neutral-300 hover:bg-white/[0.04] rounded transition-colors cursor-pointer"
              title="Back"
            >
              <ChevronLeft size={14} />
            </button>
            <button
              onClick={() => setActiveTab("about")}
              className="p-1 hover:text-neutral-300 hover:bg-white/[0.04] rounded transition-colors cursor-pointer"
              title="Forward"
            >
              <ChevronRight size={14} />
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3">
          {isSaved && (
            <span className="flex items-center gap-1.5 text-xs text-emerald-400 font-medium animate-pulse">
              <Check size={12} strokeWidth={2.5} />
              Saved
            </span>
          )}

          <RaycastCaptionButtons />
        </div>
      </header>

      {/* Main Body: Sidebar + Settings Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Sidebar */}
        <aside className="w-60 border-r border-white/[0.06] bg-[#181818] flex flex-col p-2.5 shrink-0 overflow-y-auto">
          {/* Search Box */}
          <div className="relative mb-3">
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

          {/* User Profile Card matching screenshot */}
          <div className="flex items-center gap-2.5 px-2 py-2 mb-2 rounded-lg bg-transparent hover:bg-white/[0.03] transition-colors cursor-default">
            <div className="w-8 h-8 rounded-full bg-gradient-to-tr from-[#ff6339] via-purple-600 to-pink-500 p-[1.5px] shrink-0">
              <div className="w-full h-full rounded-full bg-[#181818] flex items-center justify-center text-[10px] font-bold text-white tracking-wider">
                MR
              </div>
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-xs font-semibold text-neutral-200 truncate">
                Muneeb Ur Rehman
              </span>
              <span className="text-[10px] text-neutral-500">Account</span>
            </div>
          </div>

          {/* Sidebar Nav Items */}
          <nav className="flex flex-col gap-0.5">
            {filteredNavItems.map((item) => {
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id)}
                  className={`flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-colors cursor-pointer text-left ${
                    isActive
                      ? "bg-white/[0.1] text-white font-medium"
                      : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
                  }`}
                >
                  <div className="flex items-center gap-2.5">
                    <span className={isActive ? "text-white" : "text-neutral-400"}>
                      {item.icon}
                    </span>
                    <span>{item.label}</span>
                  </div>

                  {item.badge && (
                    <span className="text-[10px] font-semibold text-[#38bdf8] bg-[#38bdf8]/15 px-1.5 py-0.5 rounded-full">
                      {item.badge}
                    </span>
                  )}
                </button>
              );
            })}

            <div className="h-px bg-white/[0.04] my-2" />

            {specialNavItems.map((item) => {
              const isActive = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id)}
                  className={`flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-colors cursor-pointer text-left ${
                    isActive
                      ? "bg-white/[0.1] text-white font-medium"
                      : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
                  }`}
                >
                  <div className="flex items-center gap-2.5">
                    {item.isSpecial ? (
                      <div className="w-5 h-5 rounded-md bg-gradient-to-br from-red-500 to-rose-600 flex items-center justify-center shrink-0">
                        {item.icon}
                      </div>
                    ) : (
                      <span className="shrink-0">{item.icon}</span>
                    )}
                    <span>{item.label}</span>
                  </div>

                  {item.badge && (
                    <span className="text-[10px] font-semibold text-[#38bdf8] bg-[#38bdf8]/15 px-1.5 py-0.5 rounded-full">
                      {item.badge}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>
        </aside>

        {/* Right Settings Content */}
        <main className="flex-1 bg-[#1e1e1e] p-7 overflow-y-auto">
          {/* TAB 1: GENERAL */}
          {activeTab === "general" && (
            <div className="max-w-xl">
              <RaycastSection>
                <RaycastRow title="Open at Login">
                  <RaycastToggle
                    checked={settings.general.launch_at_login}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        general: { ...settings.general, launch_at_login: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>

                <RaycastRow title="Show in System Tray">
                  <RaycastToggle
                    checked={true}
                    onChange={() => {}}
                  />
                </RaycastRow>

                <RaycastRow title="Automatically Show the Taskbar">
                  <RaycastToggle
                    checked={showTaskbar}
                    onChange={setShowTaskbar}
                  />
                </RaycastRow>

                <RaycastRow title="Raycast Hotkey">
                  <RaycastHotkeyPill
                    sublabel="Replace Start Menu ⊞"
                    label={settings.trigger.modifier ? `${settings.trigger.modifier.toUpperCase()} Space` : "Shift Drag"}
                    isRecording={isRecordingHotkey}
                    onClick={() => {
                      setIsRecordingHotkey(!isRecordingHotkey);
                      setTimeout(() => setIsRecordingHotkey(false), 3000);
                    }}
                    onReset={() => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, modifier: "shift" },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>
              </RaycastSection>

              {/* Appearance Section */}
              <RaycastSection title="Appearance">
                <RaycastRow title="Follow System Appearance">
                  <RaycastToggle
                    checked={followSystem}
                    onChange={setFollowSystem}
                  />
                </RaycastRow>

                <RaycastRow title="Dark Theme">
                  <RaycastSelect
                    value={darkTheme}
                    options={[
                      { label: "Raycast Dark", value: "raycast_dark", icon: <Moon size={12} /> },
                      { label: "Midnight Black", value: "midnight", icon: <Moon size={12} /> },
                      { label: "Charcoal Slate", value: "charcoal", icon: <Moon size={12} /> },
                    ]}
                    onChange={setDarkTheme}
                  />
                </RaycastRow>

                <RaycastRow title="Light Theme">
                  <RaycastSelect
                    value={lightTheme}
                    options={[
                      { label: "Raycast Light", value: "raycast_light", icon: <Sun size={12} /> },
                      { label: "Pure White", value: "pure_white", icon: <Sun size={12} /> },
                    ]}
                    onChange={setLightTheme}
                  />
                </RaycastRow>

                <RaycastRow
                  title="Theme Studio"
                  description="Edit or create themes for Raycast"
                >
                  <RaycastButton variant="pro">
                    Upgrade to Pro
                  </RaycastButton>
                </RaycastRow>

                <RaycastRow
                  title="Interface Size"
                  description="Adjust the size of the Raycast interface"
                >
                  <RaycastSegmented
                    value={interfaceSize}
                    options={[
                      { label: <span className="text-[10px]">Aa</span>, value: "small", title: "Compact" },
                      { label: <span className="text-xs">Aa</span>, value: "normal", title: "Standard" },
                      { label: <span className="text-sm font-semibold">Aa</span>, value: "large", title: "Large" },
                    ]}
                    onChange={setInterfaceSize}
                  />
                </RaycastRow>
              </RaycastSection>

              {/* Windows Shell Integration Section */}
              <RaycastSection title="Windows Integration">
                <RaycastRow
                  title="Explorer Context Menu"
                  description="Show 'Open with Wheel' in Windows 10/11 right-click context menus"
                >
                  <RaycastToggle
                    checked={contextMenuEnabled}
                    onChange={handleToggleContextMenu}
                  />
                </RaycastRow>

                <RaycastRow
                  title="Send Source to Recycle Bin"
                  description="Safely recycle original files after conversion (reversible)"
                >
                  <RaycastToggle
                    checked={settings.output.recycle_source}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, recycle_source: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>

                <RaycastRow
                  title="File Output Suffix"
                  description="Appended to converted files (leave blank to save as clean filename)"
                >
                  <input
                    type="text"
                    value={settings.output.suffix}
                    onChange={(e) => {
                      const updated = {
                        ...settings,
                        output: { ...settings.output, suffix: e.target.value },
                      };
                      handleSaveSettings(updated);
                    }}
                    placeholder=".converted"
                    className="w-32 px-2.5 py-1 text-xs bg-[#242424] border border-white/[0.08] focus:border-white/30 rounded-lg text-white font-mono outline-none text-right"
                  />
                </RaycastRow>
              </RaycastSection>
            </div>
          )}

          {/* TAB 2: LAUNCHER & DRAG */}
          {activeTab === "launcher" && (
            <div className="max-w-xl">
              <RaycastSection title="Trigger Preferences">
                <RaycastRow
                  title="Activation Modifier Key"
                  description="Modifier key held while dragging to summon the radial menu"
                >
                  <RaycastSegmented
                    value={settings.trigger.modifier}
                    options={[
                      { label: "Shift", value: "shift" },
                      { label: "Ctrl", value: "ctrl" },
                      { label: "Alt", value: "alt" },
                      { label: "None", value: "none" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, modifier: val },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>

                <RaycastRow
                  title="Movement Threshold"
                  description="Minimum cursor movement before drag triggers"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-300">
                      {settings.trigger.movement_threshold_px} px
                    </span>
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
                      className="w-28 accent-[#ff6339] cursor-pointer"
                    />
                  </div>
                </RaycastRow>

                <RaycastRow
                  title="Pause Wheel Globally"
                  description="Temporarily silence the radial gesture without quitting the app"
                >
                  <RaycastToggle
                    checked={settings.trigger.paused}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        trigger: { ...settings.trigger, paused: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>
              </RaycastSection>
            </div>
          )}

          {/* TAB 3: RADIAL WHEEL */}
          {activeTab === "shortcuts" && (
            <div className="max-w-xl">
              <RaycastSection title="Wheel Geometry">
                <RaycastRow
                  title="Slot Count per Page"
                  description="Number of radial wedges displayed per page"
                >
                  <RaycastSegmented
                    value={String(settings.wheel_ui.slot_count)}
                    options={[
                      { label: "6", value: "6" },
                      { label: "8", value: "8" },
                      { label: "10", value: "10" },
                      { label: "12", value: "12" },
                    ]}
                    onChange={(val) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, slot_count: Number(val) },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>

                <RaycastRow
                  title="Wheel Diameter"
                  description="Outer pixel diameter of the radial wheel"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-neutral-300">
                      {settings.wheel_ui.size} px
                    </span>
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
                      className="w-28 accent-[#ff6339] cursor-pointer"
                    />
                  </div>
                </RaycastRow>

                <RaycastRow
                  title="Context Filtering"
                  description="Automatically hide or dim formats incompatible with dragged files"
                >
                  <RaycastToggle
                    checked={settings.wheel_ui.context_filter_enabled}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, context_filter_enabled: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>

                <RaycastRow
                  title="Audio Haptic Feedback"
                  description="Play subtle mechanical tick sounds on wedge hover"
                >
                  <RaycastToggle
                    checked={settings.wheel_ui.sound_enabled}
                    onChange={(checked) => {
                      const updated = {
                        ...settings,
                        wheel_ui: { ...settings.wheel_ui, sound_enabled: checked },
                      };
                      handleSaveSettings(updated);
                    }}
                  />
                </RaycastRow>
              </RaycastSection>
            </div>
          )}

          {/* TAB 4: KEYBOARD & ACTIONS */}
          {activeTab === "keyboard" && (
            <div className="max-w-xl">
              <RaycastSection title="Registered Wheel Actions">
                <div className="bg-[#242424] border border-white/[0.06] rounded-xl divide-y divide-white/[0.04] overflow-hidden">
                  {actions.map((act) => (
                    <div key={act.id} className="p-3 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="w-7 h-7 rounded-lg bg-[#2e2e2e] border border-white/[0.08] flex items-center justify-center text-[#ff6339] text-xs font-bold font-mono">
                          {act.title.slice(0, 3).toUpperCase()}
                        </div>
                        <div>
                          <span className="text-xs font-medium text-neutral-200 block">
                            {act.title}
                          </span>
                          <span className="text-[11px] text-neutral-500 capitalize">
                            {act.category} • {act.kind}
                          </span>
                        </div>
                      </div>

                      <div className="flex items-center gap-2">
                        <span className="text-[10px] text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded font-medium">
                          Enabled
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </RaycastSection>
            </div>
          )}

          {/* TAB 5: PRESETS & WORKFLOWS */}
          {activeTab === "cloud" && (
            <div className="max-w-xl">
              <RaycastSection title="Multi-Step Recipes">
                <div className="space-y-2.5">
                  {(settings.presets || []).map((preset: any) => (
                    <div
                      key={preset.id}
                      className="p-3.5 bg-[#242424] border border-white/[0.06] rounded-xl flex items-center justify-between"
                    >
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-medium text-neutral-200">
                            {preset.name}
                          </span>
                          <span className="text-[10px] text-neutral-400 bg-white/[0.06] px-1.5 py-0.5 rounded">
                            {preset.steps.length} steps
                          </span>
                        </div>
                        <p className="text-[11px] text-neutral-500 mt-0.5">
                          {preset.description}
                        </p>
                      </div>

                      <RaycastButton variant="secondary">
                        Edit
                      </RaycastButton>
                    </div>
                  ))}
                </div>
              </RaycastSection>
            </div>
          )}

          {/* TAB 6: ADVANCED */}
          {activeTab === "advanced" && (
            <div className="max-w-xl">
              <RaycastSection title="Diagnostics & Maintenance">
                <RaycastRow
                  title="Local SQLite History"
                  description={`${historyCount} operations stored in local database`}
                >
                  <RaycastButton
                    variant="danger"
                    disabled={historyCount === 0}
                    onClick={handleClearHistory}
                  >
                    Clear History
                  </RaycastButton>
                </RaycastRow>

                <RaycastRow
                  title="FFmpeg Sidecar Status"
                  description={ffmpegStatus?.installed ? ffmpegStatus.version || "Installed" : "Not detected in system PATH"}
                >
                  <span className={`text-xs font-medium px-2 py-0.5 rounded ${ffmpegStatus?.installed ? "text-emerald-400 bg-emerald-500/10" : "text-amber-400 bg-amber-500/10"}`}>
                    {ffmpegStatus?.installed ? "Ready" : "Optional"}
                  </span>
                </RaycastRow>
              </RaycastSection>
            </div>
          )}

          {/* TAB 7: AI & MODELS */}
          {activeTab === "ai" && (
            <div className="max-w-xl">
              <RaycastSection title="Neural Background Removal Model">
                <RaycastRow
                  title="RMBG-1.4 Neural Model"
                  description={rmbgStatus?.installed ? "Installed in Local AppData with DirectML execution" : "Requires one-time download (176MB)"}
                >
                  {rmbgStatus?.installed ? (
                    <span className="text-emerald-400 font-medium text-xs flex items-center gap-1.5 bg-emerald-500/10 px-2.5 py-1 rounded-lg">
                      <FileCheck2 size={13} />
                      Installed
                    </span>
                  ) : (
                    <RaycastButton
                      variant="primary"
                      disabled={isDownloadingModel}
                      onClick={handleDownloadModel}
                    >
                      <Download size={12} className="inline mr-1" />
                      {isDownloadingModel ? "Downloading..." : "Download (176MB)"}
                    </RaycastButton>
                  )}
                </RaycastRow>
              </RaycastSection>
            </div>
          )}

          {/* TAB 8: ABOUT */}
          {activeTab === "about" && (
            <div className="max-w-xl">
              <RaycastSection title="About Wheel">
                <div className="p-4 bg-[#242424] border border-white/[0.06] rounded-xl space-y-3">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Application Version</span>
                    <span className="font-mono text-neutral-200">v0.1.0 (Windows x86_64)</span>
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Tauri Engine</span>
                    <span className="font-mono text-neutral-200">Tauri v2 + WebView2</span>
                  </div>

                  <div className="h-px bg-white/[0.04]" />

                  <div className="flex items-center justify-between text-xs">
                    <span className="text-neutral-400">Design Framework</span>
                    <span className="text-neutral-200">Raycast on Windows Design System</span>
                  </div>
                </div>
              </RaycastSection>
            </div>
          )}

          {/* FALLBACK / OTHER TABS */}
          {(activeTab === "orgs" || activeTab === "applications" || activeTab === "browser") && (
            <div className="max-w-xl">
              <RaycastSection title={activeTab.toUpperCase()}>
                <div className="p-8 text-center text-neutral-500 text-xs bg-[#242424] border border-white/[0.06] rounded-xl">
                  <span>No custom settings configured for this section yet.</span>
                </div>
              </RaycastSection>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
