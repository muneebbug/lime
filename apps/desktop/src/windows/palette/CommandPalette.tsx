import React, { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Search,
  Wrench,
  RefreshCw,
  Sparkles,
  History,
  FileImage,
  ArrowRight,
  X,
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
        subtitle: `Launch ${a.title} tool window`,
        category: "tools",
        icon: <Wrench size={14} className="text-orange-400" />,
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

    // 3. Preset Chains
    items.push({
      id: "preset.clean_web",
      title: "Preset: Clean Web Asset",
      subtitle: "Strip EXIF metadata + Convert to optimized WebP",
      category: "presets",
      icon: <Sparkles size={14} className="text-purple-400" />,
      actionId: "preset.clean_web",
    });

    items.push({
      id: "preset.share_screenshot",
      title: "Preset: Share Screenshot",
      subtitle: "Add stylish background canvas padding + PNG export",
      category: "presets",
      icon: <Sparkles size={14} className="text-purple-400" />,
      actionId: "preset.share_screenshot",
    });

    items.push({
      id: "preset.transparent_png",
      title: "Preset: Cutout PNG",
      subtitle: "Remove background + export transparent PNG",
      category: "presets",
      icon: <Sparkles size={14} className="text-purple-400" />,
      actionId: "preset.transparent_png",
    });

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
  }, [actions, history]);

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
      className="flex flex-col h-screen w-screen bg-zinc-950 text-zinc-100 font-sans select-none overflow-hidden rounded-2xl border border-white/10 shadow-2xl backdrop-blur-2xl"
    >
      {/* Background warm glow */}
      <div className="absolute inset-0 pointer-events-none bg-[radial-gradient(ellipse_at_50%_-10%,rgba(249,115,22,0.18),transparent_65%)]" />

      {/* Search Input Bar */}
      <div
        data-tauri-drag-region
        className="relative z-10 flex items-center gap-3 px-4 py-3.5 border-b border-white/10 bg-zinc-900/60"
      >
        <Search size={18} className="text-orange-400 shrink-0" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search commands, tools, conversions, or history..."
          className="flex-1 bg-transparent text-sm text-white placeholder-zinc-500 outline-none"
        />
        <button
          onClick={handleClose}
          className="w-6 h-6 rounded-full flex items-center justify-center bg-white/5 hover:bg-white/10 text-zinc-400 cursor-pointer"
        >
          <X size={13} />
        </button>
      </div>

      {/* Selected target file pill (if any) */}
      {selectedFile && (
        <div className="px-4 py-2 bg-orange-500/10 border-b border-orange-500/20 flex items-center justify-between text-xs text-orange-300">
          <div className="flex items-center gap-2 truncate">
            <FileImage size={13} className="text-orange-400 shrink-0" />
            <span className="truncate">Active file: <strong>{selectedFile}</strong></span>
          </div>
          <button
            onClick={() => setSelectedFile(null)}
            className="text-orange-400 hover:text-white cursor-pointer ml-2 text-xs"
          >
            Clear
          </button>
        </div>
      )}

      {/* Results List */}
      <div
        ref={listRef}
        className="flex-1 overflow-y-auto p-2 divide-y divide-white/5 relative z-10"
      >
        {filteredItems.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-48 text-zinc-500 gap-1.5 text-xs">
            <Search size={24} className="opacity-30 mb-1" />
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
                className={`flex items-center justify-between px-3 py-2.5 rounded-xl cursor-pointer transition-all ${
                  isSelected
                    ? "bg-orange-500/15 border border-orange-500/30 text-white shadow-sm"
                    : "border border-transparent text-zinc-300 hover:bg-white/5"
                }`}
              >
                <div className="flex items-center gap-3 truncate">
                  <div className="w-7 h-7 rounded-lg bg-zinc-900 border border-white/10 flex items-center justify-center shrink-0">
                    {item.icon}
                  </div>
                  <div className="truncate">
                    <span className="block text-xs font-semibold truncate text-zinc-100">{item.title}</span>
                    <span className="block text-[11px] text-zinc-500 truncate">{item.subtitle}</span>
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <span className="text-[10px] px-2 py-0.5 rounded-full bg-white/5 border border-white/10 text-zinc-400 capitalize">
                    {item.category}
                  </span>
                  {isSelected && <ArrowRight size={13} className="text-orange-400" />}
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Footer Navigation Bar */}
      <footer className="flex items-center justify-between px-4 py-2 border-t border-white/10 bg-zinc-900/50 text-[11px] text-zinc-500 relative z-10">
        <div className="flex items-center gap-3">
          <span><kbd className="px-1.5 py-0.5 rounded bg-white/5 border border-white/10 text-zinc-400 font-mono">↑↓</kbd> Navigate</span>
          <span><kbd className="px-1.5 py-0.5 rounded bg-white/5 border border-white/10 text-zinc-400 font-mono">↵</kbd> Select</span>
          <span><kbd className="px-1.5 py-0.5 rounded bg-white/5 border border-white/10 text-zinc-400 font-mono">Esc</kbd> Close</span>
        </div>

        <div className="flex items-center gap-1.5 text-zinc-400">
          <Sparkles size={11} className="text-orange-400" />
          <span>Wheel Palette</span>
        </div>
      </footer>
    </div>
  );
}
