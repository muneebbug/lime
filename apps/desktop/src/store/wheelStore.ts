import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

/**
 * An action as the Rust registry reports it over the Tauri bridge.
 *
 * Every field here is snake_case, because that is what serde emits. An earlier
 * version of this file declared camelCase field names while the wire format was
 * snake_case, which typechecked fine and then read `undefined` for every
 * property at runtime. The rule for this file is simple: mirror the Rust struct
 * in `crates/wheel-core/src/action.rs`, character for character.
 */
export interface ActionManifest {
  id: string;
  title: string;
  icon: string;
  /** "convert", "tools", or `{ custom: "name" }` for a user-defined action. */
  category: "convert" | "tools" | { custom: string };
  accepts: { extensions: string[]; multi: boolean };
  kind: "instant" | "window";
  /** Present only for `kind: "window"`. */
  window?: { width: number; height: number; resizable: boolean; mica: boolean };
  /** Format- or tool-specific default parameters. */
  defaults?: unknown;
  enabled: boolean;
  order: number;
}

export type WheelPage = "convert" | "tools";

interface WheelStore {
  // Actions registry
  actions: ActionManifest[];
  loadActions: () => Promise<void>;

  // Drag state
  isDragging: boolean;
  dragFiles: string[];
  dragExtensions: string[];
  cursorX: number;
  cursorY: number;
  hoveredWedge: string | null;

  setDragState: (files: string[], extensions: string[], x: number, y: number) => void;
  clearDragState: () => void;
  setHoveredWedge: (id: string | null) => void;
  setCursor: (x: number, y: number) => void;

  // Page
  currentPage: WheelPage;
  lastToggleTime: number;
  setPage: (page: WheelPage) => void;
  togglePage: () => void;

  // Jobs
  recentJobs: JobSummary[];
  addJob: (job: JobSummary) => void;
  updateJob: (id: string, update: Partial<JobSummary>) => void;
}

export interface JobSummary {
  id: string;
  actionId: string;
  status: "queued" | "running" | "completed" | "failed" | "cancelled";
  outputs: string[];
  error?: string;
}

export const useWheelStore = create<WheelStore>((set) => ({
  actions: [],
  loadActions: async () => {
    try {
      const actions = await invoke<ActionManifest[]>("get_actions");
      set({ actions });
    } catch (e) {
      console.error("Failed to load actions:", e);
    }
  },

  isDragging: false,
  dragFiles: [],
  dragExtensions: [],
  cursorX: 0,
  cursorY: 0,
  hoveredWedge: null,

  setDragState: (files, extensions, x, y) =>
    set((state) => ({
      isDragging: true,
      dragFiles: files.length > 0 ? files : state.dragFiles,
      dragExtensions: extensions.length > 0 ? extensions : state.dragExtensions,
      cursorX: x,
      cursorY: y,
      currentPage: state.isDragging ? state.currentPage : "convert",
    })),
  clearDragState: () =>
    set({
      isDragging: false,
      dragFiles: [],
      dragExtensions: [],
      hoveredWedge: null,
      currentPage: "convert",
    }),
  setHoveredWedge: (id) =>
    set((state) => (state.hoveredWedge === id ? state : { hoveredWedge: id })),
  setCursor: (x, y) => set({ cursorX: x, cursorY: y }),

  currentPage: "convert",
  lastToggleTime: 0,
  setPage: (page) => set({ currentPage: page, hoveredWedge: null }),
  togglePage: () => {
    const now = Date.now();
    const state = useWheelStore.getState();
    if (now - state.lastToggleTime < 180) {
      return;
    }
    set((s) => ({
      lastToggleTime: now,
      currentPage: s.currentPage === "convert" ? "tools" : "convert",
      hoveredWedge: null,
    }));
  },

  recentJobs: [],
  addJob: (job) =>
    set((s) => ({ recentJobs: [job, ...s.recentJobs].slice(0, 50) })),
  updateJob: (id, update) =>
    set((s) => ({
      recentJobs: s.recentJobs.map((j) => (j.id === id ? { ...j, ...update } : j)),
    })),
}));
