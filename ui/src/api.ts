import { hasString, t } from "./i18n.js";
import type {
  AppInfo,
  BackupProgress,
  BenchMeta,
  BenchWindow,
  CleanSession,
  CleanState,
  CleanWindow,
  CreateReport,
  FileRow,
  Preview,
  Profile,
  ProjectView,
  RecentProject,
  RestoreReport,
  ReuseResult,
  SavedFile,
  Settings,
  SpellingGroup,
  Step,
  StringCatalogue,
  TestReport,
  TrashItem,
} from "./types.js";

type MockModule = typeof import("./mock.js");
let mockPromise: Promise<MockModule> | null = null;

function hasTauri(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI__?.core?.invoke);
}

async function getMock(): Promise<MockModule> {
  if (!mockPromise) {
    mockPromise = import("./mock.js").catch((err) => {
      mockPromise = null;
      throw err;
    });
  }
  return mockPromise;
}

export async function hasMockBackend(): Promise<boolean> {
  try {
    await getMock();
    return true;
  } catch {
    return false;
  }
}

export function extractErrorMessage(err: unknown): string {
  if (typeof err === "string") {
    return err;
  }
  if (err && typeof err === "object") {
    const record = err as Record<string, unknown>;
    if (typeof record.code === "string" && record.code.trim() !== "") {
      const key = `error.${record.code.trim()}`;
      if (hasString(key)) {
        return t(key);
      }
    }
    if (typeof record.message === "string" && record.message.trim() !== "") {
      return record.message;
    }
  }
  return String(err);
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (hasTauri()) {
    return window.__TAURI__!.core.invoke<T>(cmd, args);
  }
  const mock = await getMock();
  return mock.mockInvoke<T>(cmd, args);
}

export async function onBackupProgress(
  handler: (progress: BackupProgress) => void,
): Promise<() => void> {
  if (hasTauri() && window.__TAURI__?.event?.listen) {
    return window.__TAURI__.event.listen<BackupProgress>("backup-progress", (ev) => {
      handler(ev.payload);
    });
  }
  const mock = await getMock();
  return mock.subscribeBackupProgress(handler);
}

export const api = {
  // Bench mode commands
  getMeta: (): Promise<BenchMeta> => invoke<BenchMeta>("get_meta"),
  getWindow: (start: number, count: number): Promise<BenchWindow> =>
    invoke<BenchWindow>("get_window", { start, count }),
  setCell00: (value: string): Promise<void> => invoke<void>("set_cell_0_0", { value }),
  benchReady: (): Promise<void> => invoke<void>("bench_ready"),
  benchScroll: (frames: number, p50Ms: number, p95Ms: number, maxMs: number): Promise<void> =>
    invoke<void>("bench_scroll", { frames, p50Ms, p95Ms, maxMs }),
  benchEdit: (ms: number): Promise<void> => invoke<void>("bench_edit", { ms }),
  benchError: (message: string): Promise<void> => invoke<void>("bench_error", { message }),

  // App commands
  appInfo: (): Promise<AppInfo> => invoke<AppInfo>("app_info"),
  strings: (): Promise<StringCatalogue> => invoke<StringCatalogue>("strings"),
  settingsGet: (): Promise<Settings> => invoke<Settings>("settings_get"),
  settingsSave: (settings: Settings): Promise<Settings> =>
    invoke<Settings>("settings_save", { settings }),
  projectsRecent: (): Promise<RecentProject[]> => invoke<RecentProject[]>("projects_recent"),
  projectPick: (): Promise<ProjectView | null> => invoke<ProjectView | null>("project_pick"),
  projectOpen: (projectId: string): Promise<ProjectView> =>
    invoke<ProjectView>("project_open", { projectId }),
  projectForget: (projectId: string): Promise<void> =>
    invoke<void>("project_forget", { projectId }),
  filesList: (projectId: string): Promise<FileRow[]> =>
    invoke<FileRow[]>("files_list", { projectId }),
  fileTrash: (projectId: string, fileId: string): Promise<void> =>
    invoke<void>("file_trash", { projectId, fileId }),
  trashList: (projectId: string): Promise<TrashItem[]> =>
    invoke<TrashItem[]>("trash_list", { projectId }),
  trashRestore: (projectId: string, trashId: string): Promise<void> =>
    invoke<void>("trash_restore", { projectId, trashId }),
  trashEmpty: (projectId: string): Promise<number> =>
    invoke<number>("trash_empty", { projectId }),
  backupCreate: (projectIds: string[], passphrase: string): Promise<CreateReport | null> =>
    invoke<CreateReport | null>("backup_create", { projectIds, passphrase }),
  backupRestore: (passphrase: string): Promise<RestoreReport | null> =>
    invoke<RestoreReport | null>("backup_restore", { passphrase }),
  backupTest: (passphrase: string): Promise<TestReport | null> =>
    invoke<TestReport | null>("backup_test", { passphrase }),
  cleanOpen: (projectId: string, fileId: string): Promise<CleanSession> =>
    invoke<CleanSession>("clean_open", { projectId, fileId }),
  cleanClose: (sessionId: string): Promise<void> =>
    invoke<void>("clean_close", { sessionId }),
  cleanProfile: (sessionId: string): Promise<Profile> =>
    invoke<Profile>("clean_profile", { sessionId }),
  cleanWindow: (sessionId: string, start: number, count: number): Promise<CleanWindow> =>
    invoke<CleanWindow>("clean_window", { sessionId, start, count }),
  cleanPreview: (
    sessionId: string,
    step: Step,
    start: number,
    count: number,
  ): Promise<Preview> => invoke<Preview>("clean_preview", { sessionId, step, start, count }),
  cleanSuggestGroups: (sessionId: string, column: string): Promise<SpellingGroup[]> =>
    invoke<SpellingGroup[]>("clean_suggest_groups", { sessionId, column }),
  cleanState: (sessionId: string): Promise<CleanState> =>
    invoke<CleanState>("clean_state", { sessionId }),
  cleanAddStep: (sessionId: string, step: Step): Promise<CleanState> =>
    invoke<CleanState>("clean_add_step", { sessionId, step }),
  cleanUndo: (sessionId: string): Promise<CleanState> =>
    invoke<CleanState>("clean_undo", { sessionId }),
  cleanRedo: (sessionId: string): Promise<CleanState> =>
    invoke<CleanState>("clean_redo", { sessionId }),
  cleanMoveStep: (sessionId: string, from: number, to: number): Promise<CleanState> =>
    invoke<CleanState>("clean_move_step", { sessionId, from, to }),
  cleanSetEnabled: (sessionId: string, index: number, enabled: boolean): Promise<CleanState> =>
    invoke<CleanState>("clean_set_enabled", { sessionId, index, enabled }),
  cleanRemoveStep: (sessionId: string, index: number): Promise<CleanState> =>
    invoke<CleanState>("clean_remove_step", { sessionId, index }),
  cleanFixCell: (
    sessionId: string,
    row: number,
    column: number,
    value: string,
  ): Promise<CleanState> =>
    invoke<CleanState>("clean_fix_cell", { sessionId, row, column, value }),
  cleanSave: (sessionId: string): Promise<SavedFile> =>
    invoke<SavedFile>("clean_save", { sessionId }),
  cleanReuse: (sessionId: string, fromFileId: string): Promise<ReuseResult> =>
    invoke<ReuseResult>("clean_reuse", { sessionId, fromFileId }),
};

export function isTauriRuntime(): boolean {
  return hasTauri();
}
