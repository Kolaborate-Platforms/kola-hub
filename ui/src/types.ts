export type Route = "home" | "files" | "clean" | "annotate" | "backup" | "settings";
export type CleanTab = "profile" | "fix" | "reuse";

export interface AppInfo {
  version: string;
}

export type MessageValue = string | { one: string; other: string };
export type StringCatalogue = Record<string, MessageValue>;

export interface Settings {
  language: string;
  checkForUpdates: boolean;
  storageLimit: number;
  trashRetentionDays: number;
  labellerName: string | null;
  aiAccess: boolean;
}

export interface RecentProject {
  id: string;
  name: string;
  lastOpened: number;
  available: boolean;
}

export interface RepairReport {
  finishedMoves: number;
  rolledBack: number;
  recovered: number;
  dropped: number;
}

export type BudgetStatus = "ok" | "warn" | "over";

export interface StorageBudget {
  usedBytes: number;
  limitBytes: number;
  status: BudgetStatus;
}

export interface ProjectView {
  id: string;
  name: string;
  recovered: RepairReport;
  budget: StorageBudget;
}

export type FileKind = "csv" | "other";

export interface FileRow {
  id: string;
  name: string;
  sizeBytes: number;
  kind: FileKind;
  hasSteps: boolean;
}

export interface TrashItem {
  id: string;
  name: string;
  deletedAt: number;
  sizeBytes: number;
  recovered: boolean;
}

export interface BackupProgress {
  files: number;
  bytes: number;
}

export interface CreateReport {
  files: number;
  bytes: number;
  skipped: string[];
}

export interface RestoreReport {
  files: number;
  bytes: number;
}

export interface TestReport {
  files: number;
  bytes: number;
  sampled: number;
}

export type DetectedType = "empty" | "number" | "date" | "text";

export type IssueKind =
  | "duplicateRows"
  | "extraSpaces"
  | "datesNotReal"
  | "unreadableDates"
  | "numbersWithText"
  | "unreadableNumbers"
  | "emptyCells";

export interface CellRef {
  row: number;
  column: number;
}

export interface Issue {
  kind: IssueKind;
  column: string | null;
  count: number;
  cells: CellRef[];
}

export interface ColumnProfile {
  name: string;
  detectedType: DetectedType;
  shareNotFitting: number;
  notFitting: number;
  distinct: number;
  empty: number;
}

export interface Profile {
  rows: number;
  columns: ColumnProfile[];
  wrongCells: number;
  duplicateRows: number;
  issues: Issue[];
}

export type TargetType = "text" | "number" | "date";

export interface SpellingGroup {
  canonical: string;
  variants: string[];
}

export type Step =
  | { type: "trimSpaces"; columns: string[] | null }
  | { type: "makeDates"; column: string; dayFirst: boolean }
  | { type: "textToNumbers"; column: string }
  | { type: "mergeSpellings"; column: string; groups: SpellingGroup[] }
  | { type: "removeDuplicateRows" }
  | { type: "removeEmptyRows" }
  | { type: "renameColumn"; from: string; to: string }
  | { type: "changeType"; column: string; to: TargetType };

export interface StepEntry {
  name: string;
  enabled: boolean;
  step: Step;
}

export type Reason = "emptyNumber" | "unreadableNumber" | "unreadableDate";

export interface NeedsPerson {
  row: number;
  column: number;
  sourceRow: number;
  columnName: string;
  value: string;
  reason: Reason;
}

export interface ReplaySummary {
  rowsIn: number;
  rowsOut: number;
  rowsNeedingPerson: number;
}

export interface CleanState {
  steps: StepEntry[];
  canUndo: boolean;
  canRedo: boolean;
  summary: ReplaySummary;
  needsPerson: NeedsPerson[];
  rowCount: number;
  columns: string[];
  /** Set when the saved steps cannot run on this file. */
  blocked: Blocked | null;
}

export interface Blocked {
  report: MatchReport;
  message: string;
}

export interface CleanSession {
  sessionId: string;
  fileName: string;
  rowCount: number;
  columns: string[];
  state: CleanState;
}

export interface CleanWindow {
  start: number;
  columns: string[];
  rows: string[][];
}

export interface PreviewCell {
  old: string;
  new: string;
  changed: boolean;
  needsPerson: boolean;
}

export interface PreviewRow {
  sourceRow: number;
  removed: boolean;
  cells: PreviewCell[];
}

export interface Preview {
  columns: string[];
  newColumns: string[];
  rows: PreviewRow[];
}

export interface SavedFile {
  id: string;
  name: string;
}

export interface ColumnMatch {
  name: string;
  found: boolean;
  repeated: boolean;
}

export interface MatchReport {
  columns: ColumnMatch[];
}

export interface ReuseResult {
  report: MatchReport;
  state: CleanState | null;
}

export interface BenchMeta {
  columns: string[];
  rowCount: number;
  benchMode: boolean;
  error: string | null;
}

export interface BenchWindow {
  start: number;
  rows: string[][];
}
