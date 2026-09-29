import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface ActionManifest {
  id: string;
  title: string;
  icon: string;
  category: "convert" | "tools" | { custom: string };
  accepts: { extensions: string[]; multi: boolean };
  kind: "instant" | "window";
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
    set({ isDragging: true, dragFiles: files, dragExtensions: extensions, cursorX: x, cursorY: y }),
  clearDragState: () =>
    set({ isDragging: false, dragFiles: [], dragExtensions: [], hoveredWedge: null }),
  setHoveredWedge: (id) =>
    set((state) => (state.hoveredWedge === id ? state : { hoveredWedge: id })),
  setCursor: (x, y) => set({ cursorX: x, cursorY: y }),

  currentPage: "convert",
  setPage: (page) => set({ currentPage: page }),
  togglePage: () =>
    set((s) => ({ currentPage: s.currentPage === "convert" ? "tools" : "convert" })),

  recentJobs: [],
  addJob: (job) =>
    set((s) => ({ recentJobs: [job, ...s.recentJobs].slice(0, 50) })),
  updateJob: (id, update) =>
    set((s) => ({
      recentJobs: s.recentJobs.map((j) => (j.id === id ? { ...j, ...update } : j)),
    })),
}));
