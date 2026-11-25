const ROW_HEIGHT = 24;
const WINDOW_SIZE = 100;
const OVERSCAN = 4;

interface TableMeta {
  columns: string[];
  rowCount: number;
  benchMode: boolean;
  error: string | null;
}

interface WindowPayload {
  start: number;
  rows: string[][];
}

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return window.__TAURI__.core.invoke<T>(cmd, args);
}

function nextFrame(): Promise<number> {
  return new Promise((resolve) => {
    requestAnimationFrame((ts) => resolve(ts));
  });
}

// A frame callback runs before its frame is painted, so the second one proves the first is on screen.
async function afterPaint(): Promise<void> {
  await nextFrame();
  await nextFrame();
}

// Nearest rank, the same rule as the egui demo.
function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length, Math.max(1, rank)) - 1];
}

const statusEl = document.getElementById("status") as HTMLElement;
const errorEl = document.getElementById("error-banner") as HTMLElement;
const headerEl = document.getElementById("grid-header") as HTMLElement;
const viewportEl = document.getElementById("grid-viewport") as HTMLElement;
const spacerEl = document.getElementById("grid-spacer") as HTMLElement;
const rowsEl = document.getElementById("grid-rows") as HTMLElement;
const goLastBtn = document.getElementById("go-last-btn") as HTMLButtonElement;

let meta: TableMeta = {
  columns: [],
  rowCount: 0,
  benchMode: false,
  error: null,
};

let cachedWindow: WindowPayload = { start: 0, rows: [] };
let renderSeq = 0;
let editingCell00 = false;
// The bench draws each frame itself, so the scroll listener must not fetch a second time.
let benching = false;

function buildHeader(columns: string[]): void {
  headerEl.textContent = "";
  const numCell = document.createElement("div");
  numCell.className = "grid-cell row-num";
  numCell.textContent = "#";
  headerEl.appendChild(numCell);

  for (const name of columns) {
    const cell = document.createElement("div");
    cell.className = "grid-cell";
    cell.textContent = name;
    cell.title = name;
    headerEl.appendChild(cell);
  }
}

function windowCovers(firstRow: number, lastRow: number): boolean {
  if (meta.rowCount === 0) return true;
  if (cachedWindow.rows.length === 0) return false;
  const cachedEnd = cachedWindow.start + cachedWindow.rows.length - 1;
  return firstRow >= cachedWindow.start && lastRow <= cachedEnd;
}

async function ensureWindow(firstRow: number, lastRow: number): Promise<boolean> {
  if (windowCovers(firstRow, lastRow)) {
    return true;
  }
  const reqSeq = ++renderSeq;
  const fetchStart = Math.max(0, firstRow - 10);
  const payload = await invoke<WindowPayload>("get_window", {
    start: fetchStart,
    count: WINDOW_SIZE,
  });
  if (reqSeq !== renderSeq) {
    return false;
  }
  cachedWindow = payload;
  return true;
}

function getVisibleRange(): { firstRow: number; lastRow: number } {
  if (meta.rowCount === 0) {
    return { firstRow: 0, lastRow: -1 };
  }
  const viewportHeight = viewportEl.clientHeight || 700;
  const rawFirst = Math.floor(viewportEl.scrollTop / ROW_HEIGHT);
  const visibleCount = Math.ceil(viewportHeight / ROW_HEIGHT);
  const firstRow = Math.max(0, rawFirst - OVERSCAN);
  const lastRow = Math.min(meta.rowCount - 1, rawFirst + visibleCount + OVERSCAN);
  return { firstRow, lastRow };
}

function openCell00Editor(cellEl: HTMLElement, currentValue: string): void {
  if (editingCell00) return;
  editingCell00 = true;

  const input = document.createElement("input");
  input.type = "text";
  input.className = "cell-input";
  input.value = currentValue;
  input.setAttribute("aria-label", "Edit row 1 column 1");

  const commit = async (): Promise<void> => {
    if (!editingCell00) return;
    editingCell00 = false;
    const nextValue = input.value;
    await invoke<void>("set_cell_0_0", { value: nextValue });
    if (cachedWindow.start === 0 && cachedWindow.rows.length > 0 && cachedWindow.rows[0].length > 0) {
      cachedWindow.rows[0][0] = nextValue;
    }
    await renderVisible();
  };

  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      void commit();
    } else if (ev.key === "Escape") {
      editingCell00 = false;
      void renderVisible();
    }
  });
  input.addEventListener("blur", () => {
    void commit();
  });

  cellEl.textContent = "";
  cellEl.appendChild(input);
  input.focus();
  input.select();
}

function drawVisibleRows(firstRow: number, lastRow: number): void {
  if (lastRow < firstRow) {
    rowsEl.textContent = "";
    return;
  }

  rowsEl.style.transform = `translateY(${firstRow * ROW_HEIGHT}px)`;
  const frag = document.createDocumentFragment();

  for (let r = firstRow; r <= lastRow; r++) {
    const rowIndexInCache = r - cachedWindow.start;
    const rowData =
      rowIndexInCache >= 0 && rowIndexInCache < cachedWindow.rows.length
        ? cachedWindow.rows[rowIndexInCache]
        : null;

    const rowEl = document.createElement("div");
    rowEl.className = r % 2 === 1 ? "grid-row alt" : "grid-row";

    const numCell = document.createElement("div");
    numCell.className = "grid-cell row-num";
    numCell.textContent = String(r + 1);
    rowEl.appendChild(numCell);

    for (let c = 0; c < meta.columns.length; c++) {
      const cellEl = document.createElement("div");
      const val = rowData ? (rowData[c] ?? "") : "";
      if (r === 0 && c === 0) {
        cellEl.className = "grid-cell editable";
        cellEl.title = "Click to edit cell (row 1, column 1)";
        cellEl.textContent = val;
        cellEl.addEventListener("click", () => openCell00Editor(cellEl, val));
      } else {
        cellEl.className = "grid-cell";
        cellEl.textContent = val;
      }
      rowEl.appendChild(cellEl);
    }

    frag.appendChild(rowEl);
  }

  rowsEl.textContent = "";
  rowsEl.appendChild(frag);
}

async function renderVisible(): Promise<void> {
  if (editingCell00) return;
  const { firstRow, lastRow } = getVisibleRange();
  if (lastRow < firstRow) {
    rowsEl.textContent = "";
    return;
  }
  const ok = await ensureWindow(firstRow, lastRow);
  if (!ok) return;
  drawVisibleRows(firstRow, lastRow);
}

function scrollToLastRow(): void {
  if (meta.rowCount === 0) return;
  const maxScroll = Math.max(0, meta.rowCount * ROW_HEIGHT - viewportEl.clientHeight);
  viewportEl.scrollTop = maxScroll;
  void renderVisible();
}

async function runBench(): Promise<void> {
  try {
    // 1. Report ready after initial paint.
    await afterPaint();
    await invoke<void>("bench_ready");

    // 2. Scroll benchmark: 600 frames, each moving 500 rows further (wrapping).
    const totalFrames = 600;
    const stepRows = 500;
    const deltas: number[] = [];
    let targetRow = 0;
    const maxScroll = Math.max(0, meta.rowCount * ROW_HEIGHT - viewportEl.clientHeight);

    let prevTs = await nextFrame();
    for (let i = 0; i < totalFrames; i++) {
      targetRow = meta.rowCount > 0 ? (targetRow + stepRows) % meta.rowCount : 0;
      const desiredTop = Math.min(targetRow * ROW_HEIGHT, maxScroll);
      viewportEl.scrollTop = desiredTop;

      const { firstRow, lastRow } = getVisibleRange();
      cachedWindow = await invoke<WindowPayload>("get_window", {
        start: Math.max(0, firstRow),
        count: WINDOW_SIZE,
      });
      drawVisibleRows(firstRow, lastRow);

      const nowTs = await nextFrame();
      deltas.push(nowTs - prevTs);
      prevTs = nowTs;
    }

    deltas.sort((a, b) => a - b);
    const p50Ms = percentile(deltas, 50);
    const p95Ms = percentile(deltas, 95);
    const maxMs = deltas.length > 0 ? deltas[deltas.length - 1] : 0;
    await invoke<void>("bench_scroll", {
      frames: totalFrames,
      p50Ms,
      p95Ms,
      maxMs,
    });

    // 3. Edit benchmark: scroll to row 0, edit cell 0,0 from code, wait until painted.
    viewportEl.scrollTop = 0;
    cachedWindow = await invoke<WindowPayload>("get_window", {
      start: 0,
      count: WINDOW_SIZE,
    });
    const range0 = getVisibleRange();
    drawVisibleRows(range0.firstRow, range0.lastRow);
    await nextFrame();

    const editStart = performance.now();
    await invoke<void>("set_cell_0_0", { value: "Edited value" });
    cachedWindow = await invoke<WindowPayload>("get_window", {
      start: 0,
      count: WINDOW_SIZE,
    });
    drawVisibleRows(range0.firstRow, range0.lastRow);
    await afterPaint();
    const editMs = performance.now() - editStart;

    await invoke<void>("bench_edit", { ms: editMs });
  } catch (err) {
    await invoke<void>("bench_error", { message: String(err) });
  }
}

async function init(): Promise<void> {
  meta = await invoke<TableMeta>("get_meta");

  if (meta.error) {
    statusEl.textContent = "Error";
    errorEl.textContent = meta.error;
    errorEl.hidden = false;
    return;
  }

  statusEl.textContent = `${meta.rowCount.toLocaleString()} rows, ${meta.columns.length} columns`;
  buildHeader(meta.columns);

  // Note: browsers cap element height (typically 16M to 33M px).
  // 100,000 rows * 24 px = 2,400,000 px fits safely; larger tables need offset scaling.
  spacerEl.style.height = `${meta.rowCount * ROW_HEIGHT}px`;

  viewportEl.addEventListener("scroll", () => {
    headerEl.scrollLeft = viewportEl.scrollLeft;
    if (!benching) void renderVisible();
  });

  goLastBtn.addEventListener("click", () => {
    scrollToLastRow();
  });

  await renderVisible();

  if (meta.benchMode) {
    benching = true;
    await runBench();
  }
}

void init();
