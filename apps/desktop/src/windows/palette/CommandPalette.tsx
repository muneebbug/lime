import React, { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Search,
  RefreshCw,
  Sparkles,
  History,
  FileImage,
  X,
  Scissors,
} from "lucide-react";

interface PaletteItem {
  id: string;
  title: string;
  subtitle: string;
  category: "tools" | "converts" | "presets" | "history";
  icon: React.ReactNode;
  actionId?: string;
  filePath?: string;
}

export function CommandPalette() {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [actions, setActions] = useState<any[]>([]);
  const [history, setHistory] = useState<any[]>([]);
  const [settings, setSettings] = useState<any>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const appWindow = getCurrentWebviewWindow();

  useEffect(() => {
    inputRef.current?.focus();

    async function loadData() {
      try {
        const acts = await invoke<any[]>("get_actions");
        setActions(acts);
      } catch (e) {
        console.error("Failed to load actions", e);
      }

      try {
        const hist = await invoke<any[]>("get_history", { limit: 15 });
        setHistory(hist);
      } catch (e) {
        console.error("Failed to load history", e);
      }

      try {
        const s = await invoke<any>("get_settings");
        setSettings(s);
      } catch (e) {
        console.error("Failed to load settings in palette", e);
      }
    }
    loadData();
  }, []);

  const handleClose = async () => {
    try {
      await appWindow.close();
    } catch (e) {
      console.error("Failed to close palette", e);
    }
  };

  // Build searchable items
  const allItems: PaletteItem[] = useMemo(() => {
    const items: PaletteItem[] = [];

    // 1. Tool Actions
    for (const a of actions.filter((x) => x.category === "tools")) {
      items.push({
        id: a.id,
        title: a.title,
        subtitle: a.id === "tool.trim" ? "Trim transparent blank pixels (Photoshop Trim)" : `Launch ${a.title} tool`,
        category: "tools",
        icon: <Scissors size={14} className="text-orange-400" />,
        actionId: a.id,
      });
    }


    // 2. Convert Actions
    for (const a of actions.filter((x) => x.category === "convert")) {
      items.push({
        id: a.id,
        title: `Convert to ${a.title}`,
        subtitle: `Direct background conversion to ${a.title}`,
        category: "converts",
        icon: <RefreshCw size={14} className="text-cyan-400" />,
        actionId: a.id,
      });
    }

    // 3. Preset Chains (only enabled presets from settings)
    const activePresets = settings?.presets ?? [
      {
        id: "preset.trim_webp",
        name: "Trim & WebP",
        description: "Trim transparent blank pixels and convert to WebP",
        enabled: true,
      },
      {
        id: "preset.trim_png",
        name: "Trim PNG",
        description: "Trim transparent blank pixels from PNG",
        enabled: true,
      },
    ];

    for (const p of activePresets) {
      if (p.enabled !== false) {
        items.push({
          id: p.id,
          title: `Preset: ${p.name}`,
          subtitle: p.description,
          category: "presets",
          icon: <Sparkles size={14} className="text-purple-400" />,
          actionId: p.id,
        });
      }
    }

    // 4. Recent Files from History
    for (const h of history) {
      const outPath = h.outputs?.[0] || h.inputs?.[0];
      if (outPath) {
        const fname = outPath.split(/[/\\]/).pop() ?? outPath;
        items.push({
          id: `hist-${h.id}`,
          title: fname,
          subtitle: `${h.action_id} • ${new Date(h.created_at).toLocaleTimeString()}`,
          category: "history",
          icon: <History size={14} className="text-zinc-400" />,
          filePath: outPath,
        });
      }
    }

    return items;
  }, [actions, history, settings]);

  // Filter based on query
  const filteredItems = useMemo(() => {
    if (!query.trim()) return allItems;
    const q = query.toLowerCase();
    return allItems.filter(
      (item) =>
        item.title.toLowerCase().includes(q) ||
        item.subtitle.toLowerCase().includes(q) ||
        item.category.toLowerCase().includes(q)
    );
  }, [allItems, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Scroll active item into view
  useEffect(() => {
    if (!listRef.current) return;
    const activeEl = listRef.current.children[selectedIndex] as HTMLElement | undefined;
    if (activeEl) {
      activeEl.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  const handleSelectItem = async (item: PaletteItem) => {
    if (item.category === "history" && item.filePath) {
      try {
        await invoke("open_file", { path: item.filePath });
        handleClose();
      } catch (e) {
        console.error("Failed to open file", e);
      }
      return;
    }

    if (item.actionId) {
      const targetFiles = selectedFile ? [selectedFile] : [];
      try {
        await invoke("dispatch_action", {
          request: {
            action_id: item.actionId,
            files: targetFiles,
            params: {},
          },
        });
        handleClose();
      } catch (e) {
        console.error("Failed to dispatch action", e);
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % Math.max(1, filteredItems.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + filteredItems.length) % Math.max(1, filteredItems.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredItems[selectedIndex]) {
        handleSelectItem(filteredItems[selectedIndex]);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      handleClose();
    }
  };

  return (
    <div
      onKeyDown={handleKeyDown}
      className="flex flex-col h-screen w-screen bg-[#151516] text-neutral-200 font-sans select-none overflow-hidden rounded-xl border border-white/[0.08] shadow-2xl"
    >
      {/* Search Input Bar with integrated window controls */}
      <div
        data-tauri-drag-region
        onMouseDown={(e) => {
          if (e.button === 0 && !(e.target as HTMLElement).closest("input, button, [data-no-drag], [data-tauri-drag-region='false']")) {
            appWindow.startDragging().catch(() => {});
          }
        }}
        className="relative z-10 flex items-center gap-3 px-4 py-3 border-b border-white/[0.06] bg-[#151516]"
      >
        <Search size={16} className="text-neutral-400 shrink-0" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search commands, tools, conversions, or history..."
          className="flex-1 bg-transparent text-[13px] text-white placeholder-neutral-500 outline-none cursor-text"
        />

        <div className="flex items-center gap-2">
          <kbd className="px-1.5 py-0.5 text-[10px] font-mono text-neutral-400 bg-white/[0.06] border border-white/[0.08] rounded">
            Esc
          </kbd>
          <button
            onClick={handleClose}
            className="w-6 h-6 rounded flex items-center justify-center text-neutral-400 hover:text-white hover:bg-[#e81123] transition-colors cursor-default"
            title="Close"
          >
            <X size={13} />
          </button>
        </div>
      </div>

      {/* Selected target file pill (if any) */}
      {selectedFile && (
        <div className="px-4 py-1.5 bg-[#19191a] border-b border-white/[0.06] flex items-center justify-between text-xs text-neutral-300">
          <div className="flex items-center gap-2 truncate">
            <FileImage size={13} className="text-[#ff6339] shrink-0" />
            <span className="truncate">Active file: <strong>{selectedFile}</strong></span>
          </div>
          <button
            onClick={() => setSelectedFile(null)}
            className="text-neutral-400 hover:text-white cursor-default ml-2 text-xs"
          >
            Clear
          </button>
        </div>
      )}

      {/* Results List */}
      <div
        ref={listRef}
        className="flex-1 overflow-y-auto p-1.5 relative z-10 space-y-0.5 bg-[#19191a]"
      >
        {filteredItems.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-48 text-neutral-500 gap-1.5 text-xs">
            <Search size={22} className="opacity-30 mb-1" />
            <span>No matching commands or actions found</span>
          </div>
        ) : (
          filteredItems.map((item, index) => {
            const isSelected = index === selectedIndex;
            return (
              <div
                key={item.id}
                onClick={() => handleSelectItem(item)}
                onMouseEnter={() => setSelectedIndex(index)}
                className={`flex items-center justify-between px-3 py-2 rounded-md cursor-default transition-colors ${
                  isSelected
                    ? "bg-white/[0.12] text-white shadow-xs"
                    : "text-neutral-300 hover:bg-white/[0.04]"
                }`}
              >
                <div className="flex items-center gap-2.5 truncate">
                  <div className="w-6 h-6 rounded-md bg-[#222225] border border-white/[0.06] flex items-center justify-center shrink-0">
                    {item.icon}
                  </div>
                  <div className="truncate">
                    <span className="block text-xs font-medium truncate text-neutral-100">{item.title}</span>
                    <span className="block text-[11px] text-neutral-500 truncate">{item.subtitle}</span>
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <span className="text-[10px] px-2 py-0.5 rounded bg-white/[0.05] border border-white/[0.06] text-neutral-400 capitalize">
                    {item.category}
                  </span>
                  {isSelected && (
                    <div className="flex items-center gap-1 text-[11px] text-neutral-400 font-mono">
                      <span>↵</span>
                    </div>
                  )}
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Action Bar */}
      <footer className="flex items-center justify-between px-4 py-2 border-t border-white/[0.06] bg-[#151516] text-xs text-neutral-400 relative z-10 shrink-0">
        <div className="flex items-center gap-2 truncate">
          <span className="text-neutral-500 text-[11px]">Wheel Command Palette</span>
          {filteredItems[selectedIndex] && (
            <>
              <span className="text-neutral-600">•</span>
              <span className="text-neutral-300 font-medium text-[11px] truncate">
                {filteredItems[selectedIndex].title}
              </span>
            </>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => filteredItems[selectedIndex] && handleSelectItem(filteredItems[selectedIndex])}
            className="flex items-center gap-1.5 px-3 py-1 bg-[#ff6339] hover:bg-[#ff7247] text-white font-medium rounded-md transition-all cursor-default shadow-xs active:scale-[0.98] text-xs"
          >
            <span>Open Action</span>
            <kbd className="text-[10px] bg-black/25 text-white/90 px-1 py-0.5 rounded font-mono font-bold">
              ↵
            </kbd>
          </button>
        </div>
      </footer>
    </div>
  );
}
