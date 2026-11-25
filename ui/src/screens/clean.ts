import { api } from "../api.js";
import { badge, button, el, icon } from "../dom.js";
import { formatBytes, t } from "../i18n.js";
import type { VirtualGridHandle } from "../grid.js";
import {
  openCleanFile,
  pickAndOpenProject,
  refreshCleanProfile,
  refreshProjectData,
  renderApp,
  setNotice,
  showError,
  state,
} from "../state.js";
import type { CleanTab } from "../types.js";
import { renderCleanFix, seedDraftFromStep } from "./clean-fix.js";
import { renderCleanProfile } from "./clean-profile.js";
import { renderCleanReuse } from "./clean-reuse.js";

let activeGrid: VirtualGridHandle | null = null;

export function destroyActiveCleanGrid(): void {
  if (activeGrid) {
    activeGrid.destroy();
    activeGrid = null;
  }
}

export async function runCleanUndo(): Promise<void> {
  if (!state.cleanSession?.state.canUndo) return;
  try {
    state.cleanSession.state = await api.cleanUndo(state.cleanSession.sessionId);
    state.cleanPreviewStep = null;
    await refreshCleanProfile();
    renderApp();
  } catch (err) {
    showError(err);
  }
}

export async function runCleanRedo(): Promise<void> {
  if (!state.cleanSession?.state.canRedo) return;
  try {
    state.cleanSession.state = await api.cleanRedo(state.cleanSession.sessionId);
    state.cleanPreviewStep = null;
    await refreshCleanProfile();
    renderApp();
  } catch (err) {
    showError(err);
  }
}

export async function runCleanSave(): Promise<void> {
  if (!state.cleanSession) return;
  try {
    const saved = await api.cleanSave(state.cleanSession.sessionId);
    await refreshProjectData();
    setNotice("green", t("clean.savedFile", { name: saved.name }));
  } catch (err) {
    showError(err);
  }
}

function renderFilePicker(): { header: HTMLElement; body: HTMLElement } {
  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("clean.title")),
    el("div", "topbar-sub", t("clean.subtitleEmpty")),
  );
  header.appendChild(headText);

  const body = el("div", "page-body");
  const card = el("div", "card");
  card.appendChild(el("h2", "card-title", t("clean.pickHeading")));

  if (!state.activeProject) {
    card.appendChild(el("p", "muted", t("clean.pickNoProject")));
    const row = el("div", "mt-sm");
    row.appendChild(
      button(t("home.openFolder"), "primary", () => {
        void pickAndOpenProject();
      }, "folder"),
    );
    card.appendChild(row);
    body.appendChild(card);
    return { header, body };
  }

  const csvs = state.files.filter((f) => f.kind === "csv");
  if (csvs.length === 0) {
    card.appendChild(el("p", "muted", t("clean.pickEmpty")));
    body.appendChild(card);
    return { header, body };
  }

  for (const file of csvs) {
    const row = el("div", "lv row-between");
    const left = el("div", "row-gap-sm");
    left.append(icon("clean", "ic-sm"), el("strong", "", file.name), badge(formatBytes(file.sizeBytes), "grey"));
    row.append(
      left,
      button(t("files.openInClean"), "primary", () => {
        void openCleanFile(file.id);
      }),
    );
    card.appendChild(row);
  }

  body.appendChild(card);
  return { header, body };
}

function renderTabs(): HTMLElement {
  const tabs = el("div", "clean-tabs");
  tabs.setAttribute("role", "tablist");
  const items: [CleanTab, string][] = [
    ["profile", "clean.tabProfile"],
    ["fix", "clean.tabFix"],
    ["reuse", "clean.tabReuse"],
  ];
  for (const [tabId, key] of items) {
    const active = state.cleanTab === tabId;
    const btn = el("button", active ? "tab-btn active" : "tab-btn", t(key));
    btn.type = "button";
    btn.setAttribute("role", "tab");
    btn.setAttribute("aria-selected", String(active));
    btn.addEventListener("click", () => {
      state.cleanTab = tabId;
      renderApp();
    });
    tabs.appendChild(btn);
  }
  return tabs;
}

export function renderCleanScreen(): { header: HTMLElement; body: HTMLElement } {
  destroyActiveCleanGrid();
  const session = state.cleanSession;
  if (!session) {
    return renderFilePicker();
  }

  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  const sub = t("clean.subtitleSession", {
    rows: t("rows.count", undefined, session.state.rowCount),
    cols: t("cols.count", undefined, session.state.columns.length),
    steps: t("steps.count", undefined, session.state.steps.length),
  });
  headText.append(el("h1", "topbar-title", session.fileName), el("div", "topbar-sub", sub));

  const actions = el("div", "topbar-actions");
  const undoBtn = button(t("clean.undo"), "ghost", () => void runCleanUndo(), "undo");
  undoBtn.disabled = !session.state.canUndo;
  const redoBtn = button(t("clean.redo"), "ghost", () => void runCleanRedo(), "redo");
  redoBtn.disabled = !session.state.canRedo;
  const switchBtn = button(t("clean.closeFile"), "outline", async () => {
    await api.cleanClose(session.sessionId).catch(() => undefined);
    state.cleanSession = null;
    state.cleanProfile = null;
    state.cleanPreviewStep = null;
    state.cleanReuseResult = null;
    renderApp();
  });
  const saveBtn = button(t("clean.saveNew"), "primary", () => void runCleanSave(), "save");
  actions.append(undoBtn, redoBtn, switchBtn, saveBtn);
  header.append(headText, actions);

  const body = el("div", "page-body");
  body.appendChild(renderTabs());
  const blocked = session.state.blocked;
  if (blocked) {
    saveBtn.disabled = true;
    body.appendChild(el("div", "note note-amber", blocked.message));
  }

  const onGridMount = (grid: VirtualGridHandle): void => {
    activeGrid = grid;
  };

  if (state.cleanTab === "profile") {
    body.appendChild(renderCleanProfile(onGridMount, seedDraftFromStep));
  } else if (state.cleanTab === "fix") {
    body.appendChild(renderCleanFix(onGridMount, () => void runCleanSave()));
  } else {
    body.appendChild(renderCleanReuse(() => void runCleanSave()));
  }

  return { header, body };
}
