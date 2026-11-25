import { api } from "./api.js";
import { clear, el } from "./dom.js";
import type { BenchMeta, BenchWindow } from "./types.js";

const ROW_HEIGHT = 24;
const WINDOW_SIZE = 100;
const OVERSCAN = 4;
const BENCH_FRAMES = 600;
const BENCH_STEP_ROWS = 500;

function nextFrame(): Promise<number> {
  return new Promise((resolve) => {
    requestAnimationFrame((ts) => resolve(ts));
  });
}

async function afterPaint(): Promise<void> {
  await nextFrame();
  await nextFrame();
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length, Math.max(1, rank)) - 1];
}

interface BenchDom {
  statusEl: HTMLElement;
  errorEl: HTMLElement;
  headerEl: HTMLElement;
  viewportEl: HTMLElement;
  spacerEl: HTMLElement;
  rowsEl: HTMLElement;
  goLastBtn: HTMLButtonElement;
}

function mountBenchShell(root: HTMLElement): BenchDom {
  clear(root);
  root.className = "bench-root";

  const toolbar = el("header", "bench-toolbar");
  const info = el("div", "bench-toolbar-info");
  info.appendChild(el("span", "bench-title", "Kola Hub Grid"));
  const statusEl = el("span", "bench-status", "Loading...");
  info.appendChild(statusEl);
  const goLastBtn = el("button", "bench-go-last", "Go to last row");
  goLastBtn.type = "button";
  toolbar.append(info, goLastBtn);

  const shell = el("main", "bench-shell");
  const errorEl = el("div", "bench-error");
  errorEl.hidden = true;
  const headerEl = el("div", "bench-header");
  headerEl.setAttribute("role", "row");
  const viewportEl = el("div", "bench-viewport");
  viewportEl.tabIndex = 0;
  const spacerEl = el("div", "bench-spacer");
  const rowsEl = el("div", "bench-rows");
  spacerEl.appendChild(rowsEl);
  viewportEl.appendChild(spacerEl);
  shell.append(errorEl, headerEl, viewportEl);
  root.append(toolbar, shell);

  return { statusEl, errorEl, headerEl, viewportEl, spacerEl, rowsEl, goLastBtn };
}

function buildHeader(headerEl: HTMLElement, columns: string[]): void {
  clear(headerEl);
  headerEl.appendChild(el("div", "bench-cell bench-row-num", "#"));
  for (const name of columns) {
    const cell = el("div", "bench-cell", name);
    cell.title = name;
    headerEl.appendChild(cell);
  }
}

export async function runBenchMode(root: HTMLElement, meta: BenchMeta): Promise<void> {
  const dom = mountBenchShell(root);
  if (meta.error) {
    dom.statusEl.textContent = "Error";
    dom.errorEl.textContent = meta.error;
    dom.errorEl.hidden = false;
    return;
  }

  dom.statusEl.textContent = `${meta.rowCount.toLocaleString()} rows, ${meta.columns.length} columns`;
  buildHeader(dom.headerEl, meta.columns);
  dom.spacerEl.style.height = `${meta.rowCount * ROW_HEIGHT}px`;

  let cachedWindow: BenchWindow = { start: 0, rows: [] };
  let renderSeq = 0;
  let benching = false;

  const getVisibleRange = (): { firstRow: number; lastRow: number } => {
    if (meta.rowCount === 0) return { firstRow: 0, lastRow: -1 };
    const viewportHeight = dom.viewportEl.clientHeight || 700;
    const rawFirst = Math.floor(dom.viewportEl.scrollTop / ROW_HEIGHT);
    const visibleCount = Math.ceil(viewportHeight / ROW_HEIGHT);
    const firstRow = Math.max(0, rawFirst - OVERSCAN);
    const lastRow = Math.min(meta.rowCount - 1, rawFirst + visibleCount + OVERSCAN);
    return { firstRow, lastRow };
  };

  const drawVisibleRows = (firstRow: number, lastRow: number): void => {
    if (lastRow < firstRow) {
      clear(dom.rowsEl);
      return;
    }
    dom.rowsEl.style.transform = `translateY(${firstRow * ROW_HEIGHT}px)`;
    const frag = document.createDocumentFragment();
    for (let r = firstRow; r <= lastRow; r++) {
      const idx = r - cachedWindow.start;
      const rowData = idx >= 0 && idx < cachedWindow.rows.length ? cachedWindow.rows[idx] : null;
      const rowEl = el("div", r % 2 === 1 ? "bench-row alt" : "bench-row");
      rowEl.appendChild(el("div", "bench-cell bench-row-num", String(r + 1)));
      for (let c = 0; c < meta.columns.length; c++) {
        const val = rowData ? (rowData[c] ?? "") : "";
        rowEl.appendChild(el("div", "bench-cell", val));
      }
      frag.appendChild(rowEl);
    }
    clear(dom.rowsEl);
    dom.rowsEl.appendChild(frag);
  };

  const renderVisible = async (): Promise<void> => {
    const { firstRow, lastRow } = getVisibleRange();
    if (lastRow < firstRow) {
      clear(dom.rowsEl);
      return;
    }
    const cachedEnd = cachedWindow.start + cachedWindow.rows.length - 1;
    if (cachedWindow.rows.length === 0 || firstRow < cachedWindow.start || lastRow > cachedEnd) {
      const seq = ++renderSeq;
      const payload = await api.getWindow(Math.max(0, firstRow - 10), WINDOW_SIZE);
      if (seq !== renderSeq) return;
      cachedWindow = payload;
    }
    drawVisibleRows(firstRow, lastRow);
  };

  dom.viewportEl.addEventListener("scroll", () => {
    dom.headerEl.scrollLeft = dom.viewportEl.scrollLeft;
    if (!benching) void renderVisible();
  });

  dom.goLastBtn.addEventListener("click", () => {
    dom.viewportEl.scrollTop = Math.max(0, meta.rowCount * ROW_HEIGHT - dom.viewportEl.clientHeight);
    void renderVisible();
  });

  await renderVisible();
  benching = true;
  await executeBenchSequence(meta, dom, getVisibleRange, (win, first, last) => {
    cachedWindow = win;
    drawVisibleRows(first, last);
  });
}

async function executeBenchSequence(
  meta: BenchMeta,
  dom: BenchDom,
  getVisibleRange: () => { firstRow: number; lastRow: number },
  paintWindow: (win: BenchWindow, first: number, last: number) => void,
): Promise<void> {
  try {
    await afterPaint();
    await api.benchReady();

    const deltas: number[] = [];
    let targetRow = 0;
    const maxScroll = Math.max(0, meta.rowCount * ROW_HEIGHT - dom.viewportEl.clientHeight);
    let prevTs = await nextFrame();

    for (let i = 0; i < BENCH_FRAMES; i++) {
      targetRow = meta.rowCount > 0 ? (targetRow + BENCH_STEP_ROWS) % meta.rowCount : 0;
      dom.viewportEl.scrollTop = Math.min(targetRow * ROW_HEIGHT, maxScroll);
      const { firstRow, lastRow } = getVisibleRange();
      const win = await api.getWindow(Math.max(0, firstRow), WINDOW_SIZE);
      paintWindow(win, firstRow, lastRow);
      const nowTs = await nextFrame();
      deltas.push(nowTs - prevTs);
      prevTs = nowTs;
    }

    deltas.sort((a, b) => a - b);
    await api.benchScroll(
      BENCH_FRAMES,
      percentile(deltas, 50),
      percentile(deltas, 95),
      deltas.length > 0 ? deltas[deltas.length - 1] : 0,
    );

    dom.viewportEl.scrollTop = 0;
    const range0 = getVisibleRange();
    paintWindow(await api.getWindow(0, WINDOW_SIZE), range0.firstRow, range0.lastRow);
    await nextFrame();

    const editStart = performance.now();
    await api.setCell00("Edited value");
    paintWindow(await api.getWindow(0, WINDOW_SIZE), range0.firstRow, range0.lastRow);
    await afterPaint();
    await api.benchEdit(performance.now() - editStart);
  } catch (err) {
    await api.benchError(String(err));
  }
}
