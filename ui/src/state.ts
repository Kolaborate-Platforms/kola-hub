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
  SpellingGroup,
  Step,
  TrashItem,
} from "./types.js";

export interface SpellingSuggestion {
  column: string;
  groups: SpellingGroup[];
}

export interface AppState {
  route: Route;
  version: string;
  settings: Settings;
  recentProjects: RecentProject[];
  activeProject: ProjectView | null;
  files: FileRow[];
  selectedFileId: string | null;
  trash: TrashItem[];
  cleanFileId: string | null;
  cleanSession: CleanSession | null;
  cleanProfile: Profile | null;
  cleanSpellingSuggestions: SpellingSuggestion[];
  cleanTab: CleanTab;
  cleanPreviewStep: Step | null;
  cleanReuseResult: ReuseResult | null;
  notice: { tone: "green" | "amber" | "red"; text: string } | null;
}

const DEFAULT_LIMIT = 5 * 1024 * 1024 * 1024;
const SUCCESS_NOTICE_MS = 8000;
const MAX_SPELLING_DISTINCT = 200;

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
  cleanFileId: null,
  cleanSession: null,
  cleanProfile: null,
  cleanSpellingSuggestions: [],
  cleanTab: "profile",
  cleanPreviewStep: null,
  cleanReuseResult: null,
  notice: null,
};

let renderCallback: (() => void) | null = null;
let noticeTimer: ReturnType<typeof setTimeout> | null = null;

function cancelNoticeTimer(): void {
  if (noticeTimer !== null) {
    clearTimeout(noticeTimer);
    noticeTimer = null;
  }
}

export function setRenderer(fn: () => void): void {
  renderCallback = fn;
}

export function renderApp(): void {
  if (renderCallback) {
    renderCallback();
  }
}

export function setNotice(tone: "green" | "amber" | "red", text: string): void {
  cancelNoticeTimer();
  state.notice = { tone, text };
  if (tone === "green") {
    noticeTimer = setTimeout(() => {
      noticeTimer = null;
      if (state.notice?.tone === "green") {
        state.notice = null;
        renderApp();
      }
    }, SUCCESS_NOTICE_MS);
  }
  renderApp();
}

export function clearNotice(): void {
  cancelNoticeTimer();
  state.notice = null;
  renderApp();
}

export function clearSuccessNotice(): void {
  if (state.notice?.tone === "green") {
    cancelNoticeTimer();
    state.notice = null;
  }
}

export function showError(err: unknown): void {
  setNotice("red", extractErrorMessage(err));
}

// Each look at the table gets a number. Only the newest one may change what is shown.
let latestScan = 0;

/** Looks for look alike spellings one column at a time, then redraws. The table is usable meanwhile. */
async function scanSpellingSuggestions(sessionId: string, profile: Profile, scan: number): Promise<void> {
  const candidates = profile.columns.filter(
    (col) => col.detectedType === "text" && col.distinct <= MAX_SPELLING_DISTINCT,
  );
  const found: SpellingSuggestion[] = [];
  for (const col of candidates) {
    if (scan !== latestScan) return;
    const groups = await api.cleanSuggestGroups(sessionId, col.name).catch(() => []);
    if (groups.length > 0) found.push({ column: col.name, groups });
  }
  if (scan !== latestScan || state.cleanSession?.sessionId !== sessionId) return;
  state.cleanSpellingSuggestions = found;
  if (found.length > 0 && state.route === "clean") renderApp();
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
  clearSuccessNotice();
  latestOpen += 1;
  if (state.cleanSession) {
    await api.cleanClose(state.cleanSession.sessionId).catch(() => undefined);
    state.cleanFileId = null;
    state.cleanSession = null;
    state.cleanProfile = null;
    state.cleanSpellingSuggestions = [];
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
  clearSuccessNotice();
  const request = ++latestOpen;
  try {
    if (state.cleanSession) {
      await api.cleanClose(state.cleanSession.sessionId).catch(() => undefined);
      state.cleanFileId = null;
      state.cleanSession = null;
    }
    const session = await api.cleanOpen(state.activeProject.id, fileId);
    const profile = await api.cleanProfile(session.sessionId);
    if (request !== latestOpen) {
      await api.cleanClose(session.sessionId).catch(() => undefined);
      return;
    }
    state.cleanFileId = fileId;
    state.cleanSession = session;
    state.cleanProfile = profile;
    state.cleanSpellingSuggestions = [];
    state.cleanTab = "profile";
    state.cleanPreviewStep = null;
    state.cleanReuseResult = null;
    state.route = "clean";
    renderApp();
    void scanSpellingSuggestions(session.sessionId, profile, ++latestScan);
  } catch (err) {
    showError(err);
  }
}

export async function refreshCleanProfile(): Promise<void> {
  if (!state.cleanSession) return;
  const sessionId = state.cleanSession.sessionId;
  const scan = ++latestScan;
  const profile = await api.cleanProfile(sessionId);
  if (scan !== latestScan || state.cleanSession?.sessionId !== sessionId) return;
  state.cleanProfile = profile;
  state.cleanSpellingSuggestions = [];
  void scanSpellingSuggestions(sessionId, profile, scan);
}

export function navigate(route: Route): void {
  clearSuccessNotice();
  // A file still opening must not pull the person back to Clean after they moved on.
  latestOpen += 1;
  state.route = route;
  renderApp();
}
