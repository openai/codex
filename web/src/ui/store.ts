import { create } from "zustand";
import type { TreeEntry, WorkspaceRoot } from "./api";

export type OpenTab = {
  id: string; // `${rootId}:${path}`
  rootId: string;
  path: string;
  title: string;
};

type TreeState = {
  entries?: TreeEntry[];
  loading?: boolean;
  error?: string;
};

type FileState = {
  text?: string;
  loading?: boolean;
  error?: string;
};

type State = {
  roots: WorkspaceRoot[];
  rootsLoading: boolean;
  rootsError?: string;

  openTabs: OpenTab[];
  activeTabId?: string;

  tree: Record<string, TreeState>; // key: `${rootId}:${dirPath}`
  files: Record<string, FileState>; // key: `${rootId}:${filePath}`

  setRoots: (roots: WorkspaceRoot[]) => void;
  setRootsLoading: (loading: boolean) => void;
  setRootsError: (error?: string) => void;

  openTab: (tab: OpenTab) => void;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;

  setTreeState: (key: string, next: TreeState) => void;
  setFileState: (key: string, next: FileState) => void;
};

export const useAppStore = create<State>((set) => ({
  roots: [],
  rootsLoading: false,

  openTabs: [],

  tree: {},
  files: {},

  setRoots: (roots) => set({ roots }),
  setRootsLoading: (rootsLoading) => set({ rootsLoading }),
  setRootsError: (rootsError) => set({ rootsError }),

  openTab: (tab) =>
    set((s) => {
      const exists = s.openTabs.some((t) => t.id === tab.id);
      const openTabs = exists ? s.openTabs : [...s.openTabs, tab];
      return { openTabs, activeTabId: tab.id };
    }),
  closeTab: (tabId) =>
    set((s) => {
      const openTabs = s.openTabs.filter((t) => t.id !== tabId);
      const activeTabId =
        s.activeTabId === tabId ? openTabs.at(-1)?.id : s.activeTabId;
      return { openTabs, activeTabId };
    }),
  setActiveTab: (activeTabId) => set({ activeTabId }),

  setTreeState: (key, next) =>
    set((s) => ({ tree: { ...s.tree, [key]: { ...s.tree[key], ...next } } })),
  setFileState: (key, next) =>
    set((s) => ({ files: { ...s.files, [key]: { ...s.files[key], ...next } } })),
}));
