import { api } from "./api.js";
import { clear, el } from "./dom.js";
import { t } from "./i18n.js";
import type { CleanWindow, ColumnProfile, Preview, PreviewCell, Step } from "./types.js";

const ROW_HEIGHT = 34;
const PREVIEW_ROW_HEIGHT = 54;
const ROWNUM_WIDTH = 54;
const COL_WIDTH = 150;
const PREVIEW_COL_WIDTH = 220;
const SCROLL_MARGIN = 40;
const WINDOW_SIZE = 100;
const OVERSCAN = 4;

export interface VirtualGridOptions {
  sessionId: string;
  rowCount: number;
  columns: string[];
  columnProfiles?: ColumnProfile[];
  issueCells?: Set<string>;
  previewStep?: Step | null;
}

export interface VirtualGridHandle {
  element: HTMLElement;
  refresh: () => void;
  destroy: () => void;
}

function previewTargetIndices(step: Step | null | undefined, columns: string[]): Set<number> {
  const targets = new Set<number>();
  if (!step) return targets;
  if (
    step.type === "makeDates" ||
    step.type === "textToNumbers" ||
    step.type === "mergeSpellings" ||
    step.type === "changeType"
  ) {
    const idx = columns.indexOf(step.column);
    if (idx >= 0) targets.add(idx);
  } else if (step.type === "renameColumn") {
    const idx = columns.indexOf(step.from);
    if (idx >= 0) targets.add(idx);
  } else if (step.type === "trimSpaces") {
    if (step.columns === null) {
      columns.forEach((_, i) => targets.add(i));
    } else {
      for (const name of step.columns) {
        const idx = columns.indexOf(name);
        if (idx >= 0) targets.add(idx);
      }
    }
  }
  return targets;
}

function columnLeftOffset(colIdx: number, previewTargets: Set<number>): number {
  let offset = ROWNUM_WIDTH;
  for (let c = 0; c < colIdx; c++) {
    offset += previewTargets.has(c) ? PREVIEW_COL_WIDTH : COL_WIDTH;
  }
  return offset;
}

function buildHeader(
  headerEl: HTMLElement,
  columns: string[],
  profiles: ColumnProfile[] | undefined,
  previewTargets: Set<number>,
  previewStep?: Step | null,
): void {
  clear(headerEl);
  const rowEl = el("div", "vgrid-header-row");
  rowEl.setAttribute("role", "row");
  rowEl.setAttribute("aria-rowindex", "1");

  const numHead = el("div", "vgrid-cell vgrid-rownum", "#");
  numHead.setAttribute("role", "columnheader");
  numHead.setAttribute("aria-label", t("clean.gridRowNumber"));
  rowEl.appendChild(numHead);

  for (let c = 0; c < columns.length; c++) {
    const isTarget = previewTargets.has(c);
    const cls = isTarget
      ? "vgrid-cell vgrid-head-cell vgrid-col-preview"
      : "vgrid-cell vgrid-head-cell";
    const cell = el("div", cls);
    cell.setAttribute("role", "columnheader");

    const label =
      previewStep?.type === "renameColumn" && previewStep.from === columns[c]
        ? `${columns[c]} -> ${previewStep.to}`
        : columns[c];
    const name = el("span", "vgrid-col-name", label);
    name.title = label;
    cell.appendChild(name);

    const prof = profiles?.[c];
    if (prof) {
      const typeKey = `clean.colType${prof.detectedType[0].toUpperCase()}${prof.detectedType.slice(1)}`;
      const subText =
        prof.notFitting > 0
          ? t("clean.colHeaderBad", { type: t(typeKey), notFitting: prof.notFitting })
          : t("clean.colHeaderSub", { type: t(typeKey), distinct: prof.distinct });
      cell.appendChild(el("span", "vgrid-col-sub", subText));
    }
    rowEl.appendChild(cell);
  }
  headerEl.appendChild(rowEl);
}

function renderPreviewCell(cellEl: HTMLElement, cell: PreviewCell): void {
  if (cell.needsPerson) {
    cellEl.classList.add("cell-bad");
    const text = cell.old || t("clean.needsPersonCellBadge");
    cellEl.textContent = text;
    cellEl.title = text;
    return;
  }
  if (cell.changed) {
    cellEl.classList.add("vgrid-cell-diff");
    const stack = el("div", "cell-diff-stack");
    const oldSpan = el("span", "cell-gone", cell.old);
    const newSpan = el("span", "cell-fixed", cell.new);
    stack.append(oldSpan, newSpan);
    cellEl.appendChild(stack);
    cellEl.title = `${cell.old} -> ${cell.new}`;
    return;
  }
  cellEl.textContent = cell.old;
  cellEl.title = cell.old;
}

export function createVirtualGrid(opts: VirtualGridOptions): VirtualGridHandle {
  const previewTargets = previewTargetIndices(opts.previewStep, opts.columns);
  const rowHeight = opts.previewStep && previewTargets.size > 0 ? PREVIEW_ROW_HEIGHT : ROW_HEIGHT;

  const wrap = el("div", opts.previewStep ? "vgrid vgrid-preview" : "vgrid");
  wrap.setAttribute("role", "table");
  wrap.setAttribute("aria-label", t("clean.gridLabel"));
  wrap.setAttribute("aria-rowcount", String(opts.rowCount + 1));
  wrap.setAttribute("aria-colcount", String(opts.columns.length + 1));

  const headerEl = el("div", "vgrid-header");
  headerEl.setAttribute("role", "rowgroup");
  const viewportEl = el("div", "vgrid-viewport");
  viewportEl.tabIndex = 0;
  viewportEl.setAttribute("aria-label", t("clean.gridLabel"));

  const spacerEl = el("div", "vgrid-spacer");
  spacerEl.style.height = `${Math.max(rowHeight, opts.rowCount * rowHeight)}px`;
  const rowsEl = el("div", "vgrid-rows");
  rowsEl.setAttribute("role", "rowgroup");
  spacerEl.appendChild(rowsEl);
  viewportEl.appendChild(spacerEl);
  wrap.append(headerEl, viewportEl);

  buildHeader(headerEl, opts.columns, opts.columnProfiles, previewTargets, opts.previewStep);

  let winCache: CleanWindow | null = null;
  let prevCache: { start: number; data: Preview } | null = null;
  let seq = 0;
  let destroyed = false;

  const visibleRange = (): { first: number; last: number } => {
    if (opts.rowCount === 0) return { first: 0, last: -1 };
    const h = viewportEl.clientHeight || 360;
    const rawFirst = Math.floor(viewportEl.scrollTop / rowHeight);
    const count = Math.ceil(h / rowHeight);
    const first = Math.max(0, rawFirst - OVERSCAN);
    const last = Math.min(opts.rowCount - 1, rawFirst + count + OVERSCAN);
    return { first, last };
  };

  const drawNormal = (first: number, last: number): void => {
    if (!winCache || last < first) {
      clear(rowsEl);
      return;
    }
    rowsEl.style.transform = `translateY(${first * rowHeight}px)`;
    const frag = document.createDocumentFragment();
    for (let r = first; r <= last; r++) {
      const rowData = winCache.rows[r - winCache.start];
      const rowEl = el("div", "vgrid-row");
      rowEl.setAttribute("role", "row");
      rowEl.setAttribute("aria-rowindex", String(r + 2));

      const numCell = el("div", "vgrid-cell vgrid-rownum", String(r + 1));
      numCell.setAttribute("role", "rowheader");
      rowEl.appendChild(numCell);

      for (let c = 0; c < opts.columns.length; c++) {
        const val = rowData ? (rowData[c] ?? "") : "";
        const cellEl = el("div", "vgrid-cell", val);
        cellEl.setAttribute("role", "cell");
        if (opts.issueCells?.has(`${r}:${c}`)) {
          cellEl.classList.add("cell-bad");
        }
        cellEl.title = val;
        rowEl.appendChild(cellEl);
      }
      frag.appendChild(rowEl);
    }
    clear(rowsEl);
    rowsEl.appendChild(frag);
  };

  const drawPreview = (first: number, last: number): void => {
    if (!prevCache || last < first) {
      clear(rowsEl);
      return;
    }
    rowsEl.style.transform = `translateY(${first * rowHeight}px)`;
    const frag = document.createDocumentFragment();
    const rowCls = rowHeight === PREVIEW_ROW_HEIGHT ? "vgrid-row vgrid-row-tall" : "vgrid-row";

    for (let r = first; r <= last; r++) {
      const pRow = prevCache.data.rows[r - prevCache.start];
      const rowEl = el("div", pRow?.removed ? `${rowCls} row-removed` : rowCls);
      rowEl.setAttribute("role", "row");
      rowEl.setAttribute("aria-rowindex", String(r + 2));

      const numCell = el("div", "vgrid-cell vgrid-rownum", String(r + 1));
      numCell.setAttribute("role", "rowheader");
      rowEl.appendChild(numCell);

      for (let c = 0; c < opts.columns.length; c++) {
        const cls = previewTargets.has(c) ? "vgrid-cell vgrid-col-preview" : "vgrid-cell";
        const cellEl = el("div", cls);
        cellEl.setAttribute("role", "cell");
        const pCell = pRow?.cells[c];
        if (pCell) {
          renderPreviewCell(cellEl, pCell);
        }
        rowEl.appendChild(cellEl);
      }
      frag.appendChild(rowEl);
    }
    clear(rowsEl);
    rowsEl.appendChild(frag);
  };

  const renderVisible = async (): Promise<void> => {
    if (destroyed) return;
    const { first, last } = visibleRange();
    if (last < first) {
      clear(rowsEl);
      return;
    }
    if (opts.previewStep) {
      const hit =
        prevCache &&
        first >= prevCache.start &&
        last < prevCache.start + prevCache.data.rows.length;
      if (!hit) {
        const req = ++seq;
        const start = Math.max(0, first - 10);
        const data = await api.cleanPreview(opts.sessionId, opts.previewStep, start, WINDOW_SIZE);
        if (destroyed || req !== seq) return;
        prevCache = { start, data };
      }
      drawPreview(first, last);
      return;
    }
    const hit =
      winCache && first >= winCache.start && last < winCache.start + winCache.rows.length;
    if (!hit) {
      const req = ++seq;
      const start = Math.max(0, first - 10);
      const data = await api.cleanWindow(opts.sessionId, start, WINDOW_SIZE);
      if (destroyed || req !== seq) return;
      winCache = data;
    }
    drawNormal(first, last);
  };

  const onScroll = (): void => {
    headerEl.scrollLeft = viewportEl.scrollLeft;
    void renderVisible();
  };

  viewportEl.addEventListener("scroll", onScroll);
  void renderVisible();

  if (previewTargets.size > 0) {
    const firstTarget = Math.min(...previewTargets);
    requestAnimationFrame(() => {
      if (destroyed) return;
      const colLeft = columnLeftOffset(firstTarget, previewTargets);
      const colRight = colLeft + PREVIEW_COL_WIDTH;
      const viewWidth = viewportEl.clientWidth;
      if (viewWidth > 0 && (colRight > viewportEl.scrollLeft + viewWidth || colLeft < viewportEl.scrollLeft)) {
        viewportEl.scrollLeft = Math.max(0, colLeft - SCROLL_MARGIN);
        headerEl.scrollLeft = viewportEl.scrollLeft;
      }
    });
  }

  return {
    element: wrap,
    refresh: () => {
      winCache = null;
      prevCache = null;
      void renderVisible();
    },
    destroy: () => {
      destroyed = true;
      viewportEl.removeEventListener("scroll", onScroll);
    },
  };
}
