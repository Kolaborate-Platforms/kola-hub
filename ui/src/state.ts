import { api, extractErrorMessage } from "./api.js";
import type {
  CleanSession,
  CleanTab,
  FileRow,
  Profile,
  ProjectView,
  RecentProject,
  ReuseResult,
  Route,
  Settings,
  Step,
  TrashItem,
} from "./types.js";

export interface AppState {
  route: Route;
  version: string;
  settings: Settings;
  recentProjects: RecentProject[];
  activeProject: ProjectView | null;
  files: FileRow[];
  selectedFileId: string | null;
  trash: TrashItem[];
  cleanSession: CleanSession | null;
  cleanProfile: Profile | null;
  cleanTab: CleanTab;
  cleanPreviewStep: Step | null;
  cleanReuseResult: ReuseResult | null;
  notice: { tone: "green" | "amber" | "red"; text: string } | null;
}

const DEFAULT_LIMIT = 5 * 1024 * 1024 * 1024;

export const state: AppState = {
  route: "home",
  version: "0.1.0",
  settings: {
    language: "en",
    checkForUpdates: false,
    storageLimit: DEFAULT_LIMIT,
    trashRetentionDays: 30,
    labellerName: null,
    aiAccess: false,
  },
  recentProjects: [],
  activeProject: null,
  files: [],
  selectedFileId: null,
  trash: [],
  cleanSession: null,
  cleanProfile: null,
  cleanTab: "profile",
  cleanPreviewStep: null,
  cleanReuseResult: null,
  notice: null,
};

let renderCallback: (() => void) | null = null;

export function setRenderer(fn: () => void): void {
  renderCallback = fn;
}

export function renderApp(): void {
  if (renderCallback) {
    renderCallback();
  }
}

export function setNotice(tone: "green" | "amber" | "red", text: string): void {
  state.notice = { tone, text };
  renderApp();
}

export function clearNotice(): void {
  state.notice = null;
  renderApp();
}

export function showError(err: unknown): void {
  setNotice("red", extractErrorMessage(err));
}

export async function refreshProjectData(): Promise<void> {
  if (!state.activeProject) {
    state.files = [];
    state.trash = [];
    return;
  }
  const pid = state.activeProject.id;
  const [files, trash] = await Promise.all([api.filesList(pid), api.trashList(pid)]);
  state.files = files;
  state.trash = trash;
  if (state.selectedFileId && !files.some((f) => f.id === state.selectedFileId)) {
    state.selectedFileId = null;
  }
}

export async function openProjectView(view: ProjectView): Promise<void> {
  if (state.cleanSession) {
    await api.cleanClose(state.cleanSession.sessionId).catch(() => undefined);
    state.cleanSession = null;
    state.cleanProfile = null;
    state.cleanPreviewStep = null;
    state.cleanReuseResult = null;
  }
  state.activeProject = view;
  state.recentProjects = await api.projectsRecent();
  await refreshProjectData();
  renderApp();
}

export async function pickAndOpenProject(): Promise<boolean> {
  try {
    const view = await api.projectPick();
    if (!view) return false;
    await openProjectView(view);
    return true;
  } catch (err) {
    showError(err);
    return false;
  }
}

// Opening a large file is slow, so only the answer to the latest request may be shown.
let latestOpen = 0;

export async function openCleanFile(fileId: string): Promise<void> {
  if (!state.activeProject) return;
  const request = ++latestOpen;
  try {
    if (state.cleanSession) {
      await api.cleanClose(state.cleanSession.sessionId).catch(() => undefined);
      state.cleanSession = null;
    }
    const session = await api.cleanOpen(state.activeProject.id, fileId);
    const profile = await api.cleanProfile(session.sessionId);
    if (request !== latestOpen) {
      await api.cleanClose(session.sessionId).catch(() => undefined);
      return;
    }
    state.cleanSession = session;
    state.cleanProfile = profile;
    state.cleanTab = "profile";
    state.cleanPreviewStep = null;
    state.cleanReuseResult = null;
    state.route = "clean";
    renderApp();
  } catch (err) {
    showError(err);
  }
}

export async function refreshCleanProfile(): Promise<void> {
  if (!state.cleanSession) return;
  state.cleanProfile = await api.cleanProfile(state.cleanSession.sessionId);
}

export function navigate(route: Route): void {
  state.route = route;
  renderApp();
}
