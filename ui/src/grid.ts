import { api } from "./api.js";
import { clear, el } from "./dom.js";
import { t } from "./i18n.js";
import type { CleanWindow, ColumnProfile, Preview, PreviewCell, Step } from "./types.js";

const ROW_HEIGHT = 34;
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

function buildHeader(
  headerEl: HTMLElement,
  columns: string[],
  profiles?: ColumnProfile[],
): void {
  clear(headerEl);
  headerEl.appendChild(el("div", "vgrid-cell vgrid-rownum", "#"));
  for (let c = 0; c < columns.length; c++) {
    const cell = el("div", "vgrid-cell vgrid-head-cell");
    const name = el("span", "vgrid-col-name", columns[c]);
    name.title = columns[c];
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
    headerEl.appendChild(cell);
  }
}

function renderPreviewCell(cellEl: HTMLElement, cell: PreviewCell): void {
  if (cell.needsPerson) {
    cellEl.classList.add("cell-bad");
    cellEl.textContent = cell.old || t("clean.needsPersonCellBadge");
    return;
  }
  if (cell.changed) {
    const oldSpan = el("span", "cell-gone", cell.old);
    const space = document.createTextNode(" ");
    const newSpan = el("span", "cell-fixed", cell.new);
    cellEl.append(oldSpan, space, newSpan);
    return;
  }
  cellEl.textContent = cell.old;
}

export function createVirtualGrid(opts: VirtualGridOptions): VirtualGridHandle {
  const wrap = el("div", "vgrid");
  const headerEl = el("div", "vgrid-header");
  headerEl.setAttribute("role", "row");
  const viewportEl = el("div", "vgrid-viewport");
  viewportEl.tabIndex = 0;
  viewportEl.setAttribute("role", "grid");
  const spacerEl = el("div", "vgrid-spacer");
  spacerEl.style.height = `${Math.max(ROW_HEIGHT, opts.rowCount * ROW_HEIGHT)}px`;
  const rowsEl = el("div", "vgrid-rows");
  spacerEl.appendChild(rowsEl);
  viewportEl.appendChild(spacerEl);
  wrap.append(headerEl, viewportEl);

  buildHeader(headerEl, opts.columns, opts.columnProfiles);

  let winCache: CleanWindow | null = null;
  let prevCache: { start: number; data: Preview } | null = null;
  let seq = 0;
  let destroyed = false;

  const visibleRange = (): { first: number; last: number } => {
    if (opts.rowCount === 0) return { first: 0, last: -1 };
    const h = viewportEl.clientHeight || 360;
    const rawFirst = Math.floor(viewportEl.scrollTop / ROW_HEIGHT);
    const count = Math.ceil(h / ROW_HEIGHT);
    const first = Math.max(0, rawFirst - OVERSCAN);
    const last = Math.min(opts.rowCount - 1, rawFirst + count + OVERSCAN);
    return { first, last };
  };

  const drawNormal = (first: number, last: number): void => {
    if (!winCache || last < first) {
      clear(rowsEl);
      return;
    }
    rowsEl.style.transform = `translateY(${first * ROW_HEIGHT}px)`;
    const frag = document.createDocumentFragment();
    for (let r = first; r <= last; r++) {
      const rowData = winCache.rows[r - winCache.start];
      const rowEl = el("div", "vgrid-row");
      rowEl.appendChild(el("div", "vgrid-cell vgrid-rownum", String(r + 1)));
      for (let c = 0; c < opts.columns.length; c++) {
        const val = rowData ? (rowData[c] ?? "") : "";
        const cellEl = el("div", "vgrid-cell", val);
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
    rowsEl.style.transform = `translateY(${first * ROW_HEIGHT}px)`;
    const frag = document.createDocumentFragment();
    for (let r = first; r <= last; r++) {
      const pRow = prevCache.data.rows[r - prevCache.start];
      const rowEl = el("div", pRow?.removed ? "vgrid-row row-removed" : "vgrid-row");
      rowEl.appendChild(el("div", "vgrid-cell vgrid-rownum", String(r + 1)));
      for (let c = 0; c < opts.columns.length; c++) {
        const cellEl = el("div", "vgrid-cell");
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
