import type {
  BackupProgress,
  CleanSession,
  CleanState,
  CleanWindow,
  ColumnProfile,
  CreateReport,
  FileRow,
  Issue,
  MatchReport,
  NeedsPerson,
  Preview,
  PreviewCell,
  PreviewRow,
  Profile,
  ProjectView,
  RecentProject,
  RestoreReport,
  ReuseResult,
  SavedFile,
  Settings,
  SpellingGroup,
  Step,
  StepEntry,
  StringCatalogue,
  TestReport,
  TrashItem,
} from "./types.js";

const GB = 1024 * 1024 * 1024;
const NOW = Math.floor(Date.now() / 1000);
const MOCK_ROW_COUNT = 240;

const progressListeners = new Set<(p: BackupProgress) => void>();

export function subscribeBackupProgress(handler: (p: BackupProgress) => void): () => void {
  progressListeners.add(handler);
  return () => {
    progressListeners.delete(handler);
  };
}

let settings: Settings = {
  language: "en",
  checkForUpdates: false,
  storageLimit: 5 * GB,
  trashRetentionDays: 30,
  labellerName: null,
  aiAccess: false,
};

const recentProjects: RecentProject[] = [
  { id: "proj-market", name: "Market survey", lastOpened: NOW - 7200, available: true },
  { id: "proj-clinic", name: "Clinic intake", lastOpened: NOW - 86400 * 3, available: true },
];

const filesByProject: Record<string, FileRow[]> = {
  "proj-market": [
    { id: "f-week3", name: "week3_prices.csv", sizeBytes: 1_240_000, kind: "csv" },
    { id: "f-week4", name: "week4_prices.csv", sizeBytes: 1_180_000, kind: "csv" },
    { id: "f-budget", name: "survey_budget.xlsx", sizeBytes: 49_200, kind: "other" },
    { id: "f-notes", name: "field_notes.txt", sizeBytes: 8_400, kind: "other" },
  ],
  "proj-clinic": [
    { id: "f-visits", name: "visits_october.csv", sizeBytes: 620_000, kind: "csv" },
  ],
};

const trashByProject: Record<string, TrashItem[]> = {
  "proj-market": [
    { id: "t-1", name: "week2_draft.csv", deletedAt: NOW - 86400, sizeBytes: 410_000, recovered: false },
    { id: "t-2", name: "old_export.csv", deletedAt: NOW - 86400 * 4, sizeBytes: 185_000, recovered: false },
  ],
  "proj-clinic": [],
};

const BASE_COLUMNS = ["Date", "Market", "Stall", "Product", "Price", "Recorder"];
const SEED_ROWS: string[][] = [
  ["12/11/2026", "Central", "A14", "Tomatoes", "2,500 UGX", "Sam"],
  ["12 Nov 2026", "central ", "A14", "Tomatoes", "2500", "Sam"],
  ["2026-11-13", "CENTRAL", "B03", "Bananas", "18,000", "Amina"],
  ["13/11/2026", "Central", "B03", "Bananas", "UGX 18000", "Amina"],
  ["14-Nov", "North", "C22", "Beans", "", "Sam"],
  ["14/11/2026", "North ", "C22", "Beans", "3,200", "Sam"],
  ["14/11/2026", "North ", "C22", "Beans", "3,200", "Sam"],
  ["15/11/2026", "East", "D07", "Onions", "1,800", "Amina"],
];

function buildRows(): string[][] {
  const out: string[][] = [];
  for (let i = 0; i < MOCK_ROW_COUNT; i++) {
    const src = SEED_ROWS[i % SEED_ROWS.length];
    out.push([...src]);
  }
  return out;
}

interface MockSession {
  id: string;
  projectId: string;
  fileId: string;
  fileName: string;
  baseColumns: string[];
  baseRows: string[][];
  steps: StepEntry[];
  redo: StepEntry[];
  cellFixes: Map<string, string>;
}

const sessions = new Map<string, MockSession>();
const savedStepsByFile = new Map<string, StepEntry[]>();

function stepName(step: Step): string {
  switch (step.type) {
    case "trimSpaces":
      return "Trim spaces";
    case "makeDates":
      return `Make dates real in ${step.column}`;
    case "textToNumbers":
      return `Turn ${step.column} into numbers`;
    case "mergeSpellings":
      return `One spelling per name in ${step.column}`;
    case "removeDuplicateRows":
      return "Remove duplicate rows";
    case "removeEmptyRows":
      return "Remove empty rows";
    case "renameColumn":
      return `Rename ${step.from} to ${step.to}`;
    case "changeType":
      return `Change type of ${step.column}`;
  }
}

function parseMockDate(raw: string): string | null {
  const s = raw.trim();
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return s;
  const slash = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(s);
  if (slash) return `${slash[3]}-${slash[2]}-${slash[1]}`;
  const named = /^(\d{1,2})\s+Nov\s+(\d{4})$/i.exec(s);
  if (named) return `${named[2]}-11-${named[1].padStart(2, "0")}`;
  return null;
}

function parseMockNumber(raw: string): string | null {
  const cleaned = raw.replace(/UGX/gi, "").replace(/,/g, "").trim();
  if (/^\d+(\.\d+)?$/.test(cleaned)) return cleaned;
  return null;
}

function applyOneStep(
  cols: string[],
  rows: { sourceRow: number; vals: string[] }[],
  step: Step,
  flags: NeedsPerson[],
): { cols: string[]; rows: { sourceRow: number; vals: string[] }[] } {
  const nextCols = [...cols];
  let nextRows = rows.map((r) => ({ sourceRow: r.sourceRow, vals: [...r.vals] }));

  if (step.type === "trimSpaces") {
    const targets = step.columns ? new Set(step.columns) : null;
    for (const r of nextRows) {
      for (let c = 0; c < nextCols.length; c++) {
        if (!targets || targets.has(nextCols[c])) r.vals[c] = r.vals[c].trim();
      }
    }
  } else if (step.type === "makeDates") {
    const idx = nextCols.indexOf(step.column);
    if (idx >= 0) {
      nextRows.forEach((r, rowIdx) => {
        const parsed = parseMockDate(r.vals[idx]);
        if (parsed) {
          r.vals[idx] = parsed;
        } else if (r.vals[idx].trim() !== "") {
          flags.push({
            row: rowIdx,
            column: idx,
            sourceRow: r.sourceRow,
            columnName: nextCols[idx],
            value: r.vals[idx],
            reason: "unreadableDate",
          });
        }
      });
    }
  } else if (step.type === "textToNumbers") {
    const idx = nextCols.indexOf(step.column);
    if (idx >= 0) {
      nextRows.forEach((r, rowIdx) => {
        if (r.vals[idx].trim() === "") {
          flags.push({
            row: rowIdx,
            column: idx,
            sourceRow: r.sourceRow,
            columnName: nextCols[idx],
            value: r.vals[idx],
            reason: "emptyNumber",
          });
          return;
        }
        const parsed = parseMockNumber(r.vals[idx]);
        if (parsed) {
          r.vals[idx] = parsed;
        } else {
          flags.push({
            row: rowIdx,
            column: idx,
            sourceRow: r.sourceRow,
            columnName: nextCols[idx],
            value: r.vals[idx],
            reason: "unreadableNumber",
          });
        }
      });
    }
  } else if (step.type === "mergeSpellings") {
    const idx = nextCols.indexOf(step.column);
    if (idx >= 0) {
      const map = new Map<string, string>();
      for (const g of step.groups) {
        for (const v of g.variants) map.set(v, g.canonical);
      }
      for (const r of nextRows) {
        const mapped = map.get(r.vals[idx]);
        if (mapped !== undefined) r.vals[idx] = mapped;
      }
    }
  } else if (step.type === "removeDuplicateRows") {
    const seen = new Set<string>();
    nextRows = nextRows.filter((r) => {
      const key = JSON.stringify(r.vals);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  } else if (step.type === "removeEmptyRows") {
    nextRows = nextRows.filter((r) => r.vals.some((v) => v.trim() !== ""));
  } else if (step.type === "renameColumn") {
    const idx = nextCols.indexOf(step.from);
    if (idx >= 0) nextCols[idx] = step.to;
  } else if (step.type === "changeType") {
    if (step.to === "date") {
      return applyOneStep(cols, rows, { type: "makeDates", column: step.column, dayFirst: true }, flags);
    }
    if (step.to === "number") {
      return applyOneStep(cols, rows, { type: "textToNumbers", column: step.column }, flags);
    }
    return applyOneStep(cols, rows, { type: "trimSpaces", columns: [step.column] }, flags);
  }
  return { cols: nextCols, rows: nextRows };
}

function evaluateSession(session: MockSession): {
  cols: string[];
  rows: { sourceRow: number; vals: string[] }[];
  flags: NeedsPerson[];
} {
  let cols = [...session.baseColumns];
  let rows = session.baseRows.map((vals, i) => ({ sourceRow: i, vals: [...vals] }));
  const flags: NeedsPerson[] = [];
  for (const entry of session.steps) {
    if (!entry.enabled) continue;
    const res = applyOneStep(cols, rows, entry.step, flags);
    cols = res.cols;
    rows = res.rows;
  }
  for (const [key, val] of session.cellFixes.entries()) {
    const [rStr, cStr] = key.split(":");
    const r = Number(rStr);
    const c = Number(cStr);
    if (rows[r] && c >= 0 && c < cols.length) {
      rows[r].vals[c] = val;
    }
  }
  const activeFlags = flags.filter((f) => !session.cellFixes.has(`${f.row}:${f.column}`));
  return { cols, rows, flags: activeFlags };
}

function buildState(session: MockSession): CleanState {
  const ev = evaluateSession(session);
  const rowsNeeding = new Set(ev.flags.map((f) => f.row)).size;
  return {
    blocked: null,
    steps: session.steps,
    canUndo: session.steps.length > 0,
    canRedo: session.redo.length > 0,
    summary: {
      rowsIn: session.baseRows.length,
      rowsOut: ev.rows.length,
      rowsNeedingPerson: rowsNeeding,
    },
    needsPerson: ev.flags.slice(0, 500),
    rowCount: ev.rows.length,
    columns: ev.cols,
  };
}

function buildProfile(session: MockSession): Profile {
  const ev = evaluateSession(session);
  const cols: ColumnProfile[] = [];
  const issues: Issue[] = [];
  const wrongSet = new Set<string>();

  for (let c = 0; c < ev.cols.length; c++) {
    const name = ev.cols[c];
    let spaces = 0;
    let messyDates = 0;
    let badDates = 0;
    let messyNums = 0;
    let empty = 0;
    const distinct = new Set<string>();
    const spaceCells: { row: number; column: number }[] = [];
    const messyCells: { row: number; column: number }[] = [];
    const badCells: { row: number; column: number }[] = [];
    const emptyCells: { row: number; column: number }[] = [];

    const isDateCol = name.toLowerCase().includes("date");
    const isNumCol = name.toLowerCase().includes("price");

    ev.rows.forEach((r, rowIdx) => {
      const v = r.vals[c] ?? "";
      distinct.add(v);
      if (v.trim() !== v) {
        spaces++;
        wrongSet.add(`${rowIdx}:${c}`);
        if (spaceCells.length < 200) spaceCells.push({ row: rowIdx, column: c });
      }
      if (v.trim() === "") {
        empty++;
        if (isDateCol || isNumCol) {
          wrongSet.add(`${rowIdx}:${c}`);
          if (emptyCells.length < 200) emptyCells.push({ row: rowIdx, column: c });
        }
        return;
      }
      if (isDateCol) {
        const parsed = parseMockDate(v);
        if (!parsed) {
          badDates++;
          wrongSet.add(`${rowIdx}:${c}`);
          if (badCells.length < 200) badCells.push({ row: rowIdx, column: c });
        } else if (parsed !== v) {
          messyDates++;
          wrongSet.add(`${rowIdx}:${c}`);
          if (messyCells.length < 200) messyCells.push({ row: rowIdx, column: c });
        }
      } else if (isNumCol) {
        const parsed = parseMockNumber(v);
        if (parsed && parsed !== v) {
          messyNums++;
          wrongSet.add(`${rowIdx}:${c}`);
          if (messyCells.length < 200) messyCells.push({ row: rowIdx, column: c });
        }
      }
    });

    const notFitting = spaces + messyDates + badDates + messyNums;
    cols.push({
      name,
      detectedType: isDateCol ? "date" : isNumCol ? "number" : "text",
      shareNotFitting: ev.rows.length > 0 ? notFitting / ev.rows.length : 0,
      notFitting,
      distinct: distinct.size,
      empty,
    });

    if (spaces > 0) issues.push({ kind: "extraSpaces", column: name, count: spaces, cells: spaceCells });
    if (messyDates > 0) issues.push({ kind: "datesNotReal", column: name, count: messyDates, cells: messyCells });
    if (badDates > 0) issues.push({ kind: "unreadableDates", column: name, count: badDates, cells: badCells });
    if (messyNums > 0) issues.push({ kind: "numbersWithText", column: name, count: messyNums, cells: messyCells });
    if (emptyCells.length > 0) issues.push({ kind: "emptyCells", column: name, count: empty, cells: emptyCells });
  }

  const seen = new Set<string>();
  let dupes = 0;
  const dupeCells: { row: number; column: number }[] = [];
  ev.rows.forEach((r, rowIdx) => {
    const k = JSON.stringify(r.vals);
    if (seen.has(k)) {
      dupes++;
      if (dupeCells.length < 200) dupeCells.push({ row: rowIdx, column: 0 });
    } else {
      seen.add(k);
    }
  });
  if (dupes > 0) {
    issues.push({ kind: "duplicateRows", column: null, count: dupes, cells: dupeCells });
  }
  issues.sort((a, b) => b.count - a.count);

  return {
    rows: ev.rows.length,
    columns: cols,
    wrongCells: wrongSet.size,
    duplicateRows: dupes,
    issues,
  };
}

function buildProjectView(id: string): ProjectView {
  const proj = recentProjects.find((p) => p.id === id) ?? recentProjects[0];
  const usedBytes = 320 * 1024 * 1024;
  const status =
    usedBytes > settings.storageLimit
      ? "over"
      : usedBytes * 100 >= settings.storageLimit * 80
        ? "warn"
        : "ok";
  return {
    id: proj.id,
    name: proj.name,
    recovered: { finishedMoves: 0, rolledBack: 0, recovered: 0, dropped: 0 },
    budget: { usedBytes, limitBytes: settings.storageLimit, status },
  };
}

async function loadMockStrings(): Promise<StringCatalogue> {
  try {
    const res = await fetch("./en.json");
    if (res.ok) {
      return (await res.json()) as StringCatalogue;
    }
  } catch {
    // Fallback when opened without HTTP server.
  }
  return {
    "app.name": "Kola Hub",
    "rail.home": "Home",
    "rail.files": "Files",
    "rail.clean": "Clean",
    "rail.annotate": "Annotate",
    "rail.backup": "Backup",
    "rail.settings": "Settings",
    "status.offline": "Works offline",
  };
}

function getSession(id: string): MockSession {
  const s = sessions.get(id);
  if (!s) throw { code: "not_found", message: "That Clean session is no longer open." };
  return s;
}

export async function mockInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  switch (cmd) {
    case "get_meta":
      return { columns: [], rowCount: 0, benchMode: false, error: null } as T;
    case "app_info":
      return { version: "0.1.0" } as T;
    case "strings":
      return (await loadMockStrings()) as T;
    case "settings_get":
      return { ...settings } as T;
    case "settings_save":
      settings = { ...(args.settings as Settings) };
      return { ...settings } as T;
    case "projects_recent":
      return [...recentProjects] as T;
    case "project_pick":
      return buildProjectView("proj-market") as T;
    case "project_open":
      return buildProjectView(String(args.projectId)) as T;
    case "project_forget": {
      const idx = recentProjects.findIndex((p) => p.id === args.projectId);
      if (idx >= 0) recentProjects.splice(idx, 1);
      return undefined as T;
    }
    case "files_list":
      return [...(filesByProject[String(args.projectId)] ?? [])] as T;
    case "file_trash": {
      const pid = String(args.projectId);
      const fid = String(args.fileId);
      const list = filesByProject[pid] ?? [];
      const idx = list.findIndex((f) => f.id === fid);
      if (idx >= 0) {
        const [removed] = list.splice(idx, 1);
        trashByProject[pid] = trashByProject[pid] ?? [];
        trashByProject[pid].unshift({
          id: `t-${Date.now()}`,
          name: removed.name,
          deletedAt: Math.floor(Date.now() / 1000),
          sizeBytes: removed.sizeBytes,
          recovered: false,
        });
      }
      return undefined as T;
    }
    case "trash_list":
      return [...(trashByProject[String(args.projectId)] ?? [])] as T;
    case "trash_restore": {
      const pid = String(args.projectId);
      const tid = String(args.trashId);
      const list = trashByProject[pid] ?? [];
      const idx = list.findIndex((t) => t.id === tid);
      if (idx >= 0) {
        const [item] = list.splice(idx, 1);
        filesByProject[pid] = filesByProject[pid] ?? [];
        filesByProject[pid].push({
          id: `f-restored-${Date.now()}`,
          name: item.name,
          sizeBytes: item.sizeBytes,
          kind: item.name.endsWith(".csv") ? "csv" : "other",
        });
      }
      return undefined as T;
    }
    case "trash_empty": {
      const pid = String(args.projectId);
      const count = (trashByProject[pid] ?? []).length;
      trashByProject[pid] = [];
      return count as T;
    }
    case "backup_create": {
      for (const l of progressListeners) l({ files: 4, bytes: 1_900_000 });
      const report: CreateReport = { files: 4, bytes: 1_900_000, skipped: [] };
      return report as T;
    }
    case "backup_restore": {
      const report: RestoreReport = { files: 4, bytes: 1_900_000 };
      return report as T;
    }
    case "backup_test": {
      const report: TestReport = { files: 4, bytes: 1_900_000, sampled: 3 };
      return report as T;
    }
    case "clean_open": {
      const pid = String(args.projectId);
      const fid = String(args.fileId);
      const file = (filesByProject[pid] ?? []).find((f) => f.id === fid);
      const sid = `sess-${Date.now()}`;
      const existingSteps = savedStepsByFile.get(fid) ?? [];
      const session: MockSession = {
        id: sid,
        projectId: pid,
        fileId: fid,
        fileName: file?.name ?? "table.csv",
        baseColumns: [...BASE_COLUMNS],
        baseRows: buildRows(),
        steps: existingSteps.map((s) => ({ ...s })),
        redo: [],
        cellFixes: new Map(),
      };
      sessions.set(sid, session);
      const state = buildState(session);
      const res: CleanSession = {
        sessionId: sid,
        fileName: session.fileName,
        rowCount: state.rowCount,
        columns: state.columns,
        state,
      };
      return res as T;
    }
    case "clean_close":
      sessions.delete(String(args.sessionId));
      return undefined as T;
    case "clean_profile":
      return buildProfile(getSession(String(args.sessionId))) as T;
    case "clean_window": {
      const s = getSession(String(args.sessionId));
      const ev = evaluateSession(s);
      const start = Math.max(0, Number(args.start ?? 0));
      const count = Math.max(1, Number(args.count ?? 100));
      const slice = ev.rows.slice(start, start + count).map((r) => r.vals);
      const win: CleanWindow = { start, columns: ev.cols, rows: slice };
      return win as T;
    }
    case "clean_preview": {
      const s = getSession(String(args.sessionId));
      const ev = evaluateSession(s);
      const start = Math.max(0, Number(args.start ?? 0));
      const count = Math.max(1, Number(args.count ?? 100));
      const slice = ev.rows.slice(start, start + count);
      const step = args.step as Step;
      const flags: NeedsPerson[] = [];
      const after = applyOneStep(ev.cols, slice, step, flags);
      const previewRows: PreviewRow[] = slice.map((oldRow) => {
        const afterRow = after.rows.find((ar) => ar.sourceRow === oldRow.sourceRow);
        const cells: PreviewCell[] = oldRow.vals.map((oldVal, cIdx) => {
          const newVal = afterRow ? (afterRow.vals[cIdx] ?? oldVal) : oldVal;
          const needs = flags.some((f) => f.sourceRow === oldRow.sourceRow && f.column === cIdx);
          return { old: oldVal, new: newVal, changed: newVal !== oldVal, needsPerson: needs };
        });
        return { sourceRow: oldRow.sourceRow, removed: !afterRow, cells };
      });
      const preview: Preview = { columns: ev.cols, newColumns: after.cols, rows: previewRows };
      return preview as T;
    }
    case "clean_suggest_groups": {
      const s = getSession(String(args.sessionId));
      const ev = evaluateSession(s);
      const idx = ev.cols.indexOf(String(args.column));
      if (idx < 0) return [] as T;
      const byKey = new Map<string, Set<string>>();
      for (const r of ev.rows) {
        const raw = r.vals[idx];
        const k = raw.trim().toLowerCase();
        if (!k) continue;
        if (!byKey.has(k)) byKey.set(k, new Set());
        byKey.get(k)!.add(raw);
      }
      const groups: SpellingGroup[] = [];
      for (const variantsSet of byKey.values()) {
        if (variantsSet.size >= 2) {
          const variants = [...variantsSet];
          groups.push({ canonical: variants[0].trim(), variants });
        }
      }
      return groups as T;
    }
    case "clean_state":
      return buildState(getSession(String(args.sessionId))) as T;
    case "clean_add_step": {
      const s = getSession(String(args.sessionId));
      const step = args.step as Step;
      s.redo = [];
      s.steps.push({ name: stepName(step), enabled: true, step });
      savedStepsByFile.set(s.fileId, [...s.steps]);
      return buildState(s) as T;
    }
    case "clean_undo": {
      const s = getSession(String(args.sessionId));
      const last = s.steps.pop();
      if (last) s.redo.push(last);
      savedStepsByFile.set(s.fileId, [...s.steps]);
      return buildState(s) as T;
    }
    case "clean_redo": {
      const s = getSession(String(args.sessionId));
      const next = s.redo.pop();
      if (next) s.steps.push(next);
      savedStepsByFile.set(s.fileId, [...s.steps]);
      return buildState(s) as T;
    }
    case "clean_move_step": {
      const s = getSession(String(args.sessionId));
      const from = Number(args.from);
      const to = Number(args.to);
      if (from >= 0 && from < s.steps.length && to >= 0 && to < s.steps.length) {
        const [item] = s.steps.splice(from, 1);
        s.steps.splice(to, 0, item);
        savedStepsByFile.set(s.fileId, [...s.steps]);
      }
      return buildState(s) as T;
    }
    case "clean_set_enabled": {
      const s = getSession(String(args.sessionId));
      const idx = Number(args.index);
      if (s.steps[idx]) s.steps[idx].enabled = Boolean(args.enabled);
      savedStepsByFile.set(s.fileId, [...s.steps]);
      return buildState(s) as T;
    }
    case "clean_remove_step": {
      const s = getSession(String(args.sessionId));
      s.steps.splice(Number(args.index), 1);
      savedStepsByFile.set(s.fileId, [...s.steps]);
      return buildState(s) as T;
    }
    case "clean_fix_cell": {
      const s = getSession(String(args.sessionId));
      s.cellFixes.set(`${args.row}:${args.column}`, String(args.value));
      return buildState(s) as T;
    }
    case "clean_save": {
      const s = getSession(String(args.sessionId));
      const outName = s.fileName.replace(/\.csv$/i, " (cleaned).csv");
      const saved: SavedFile = { id: `f-saved-${Date.now()}`, name: outName };
      filesByProject[s.projectId] = filesByProject[s.projectId] ?? [];
      filesByProject[s.projectId].push({ id: saved.id, name: saved.name, sizeBytes: 1_120_000, kind: "csv" });
      return saved as T;
    }
    case "clean_reuse": {
      const s = getSession(String(args.sessionId));
      const fromId = String(args.fromFileId);
      const sourceSteps = savedStepsByFile.get(fromId) ?? [
        { name: "Trim spaces", enabled: true, step: { type: "trimSpaces", columns: null } },
        { name: "Make dates real in Date", enabled: true, step: { type: "makeDates", column: "Date", dayFirst: true } },
        { name: "Turn Price into numbers", enabled: true, step: { type: "textToNumbers", column: "Price" } },
      ];
      const report: MatchReport = {
        columns: [
          { name: "Date", found: s.baseColumns.includes("Date"), repeated: false },
          { name: "Price", found: s.baseColumns.includes("Price"), repeated: false },
        ],
      };
      s.steps = sourceSteps.map((st) => ({ ...st }));
      savedStepsByFile.set(s.fileId, [...s.steps]);
      const res: ReuseResult = { report, state: buildState(s) };
      return res as T;
    }
    default:
      throw { code: "unknown_command", message: `Command ${cmd} is not supported.` };
  }
}
