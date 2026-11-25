import { api } from "../api.js";
import { badge, button, el, icon } from "../dom.js";
import { formatBytes, t } from "../i18n.js";
import {
  navigate,
  openCleanFile,
  pickAndOpenProject,
  refreshProjectData,
  renderApp,
  setNotice,
  showError,
  state,
} from "../state.js";
import type { FileRow } from "../types.js";

function isExcelFile(name: string): boolean {
  const lower = name.toLowerCase();
  return lower.endsWith(".xlsx") || lower.endsWith(".xls");
}

function renderFileList(): HTMLElement {
  const tableWrap = el("div", "data-table");
  const head = el("div", "dt-row dt-head files-cols");
  head.append(
    el("div", "dt-cell", t("files.colName")),
    el("div", "dt-cell", t("files.colType")),
    el("div", "dt-cell", t("files.colSize")),
    el("div", "dt-cell", t("files.colOpensIn")),
  );
  tableWrap.appendChild(head);

  if (state.files.length === 0) {
    tableWrap.appendChild(el("div", "dt-empty", t("files.empty")));
    return tableWrap;
  }

  for (const file of state.files) {
    const selected = state.selectedFileId === file.id;
    const row = el("button", selected ? "dt-row dt-row-btn files-cols selected" : "dt-row dt-row-btn files-cols");
    row.type = "button";

    const nameCell = el("div", "dt-cell file-name-cell");
    nameCell.append(icon(file.kind === "csv" ? "clean" : "folder", "ic-sm"), el("span", "truncate", file.name));

    const typeCell = el("div", "dt-cell");
    typeCell.appendChild(
      badge(file.kind === "csv" ? t("files.typeCsv") : t("files.typeOther"), file.kind === "csv" ? "green" : "grey"),
    );

    const sizeCell = el("div", "dt-cell", formatBytes(file.sizeBytes));
    const opensCell = el(
      "div",
      "dt-cell",
      file.kind === "csv" ? t("files.opensInClean") : t("files.opensInUnsupported"),
    );

    row.append(nameCell, typeCell, sizeCell, opensCell);
    row.addEventListener("click", () => {
      state.selectedFileId = file.id;
      renderApp();
    });
    tableWrap.appendChild(row);
  }

  return tableWrap;
}

function renderInspector(selected: FileRow | undefined): HTMLElement {
  const card = el("div", "card inspector-card");
  if (!selected) {
    card.appendChild(el("p", "muted", t("files.inspectorEmpty")));
    return card;
  }

  const head = el("div", "row-gap");
  head.append(
    icon(selected.kind === "csv" ? "clean" : "folder", "ic-md"),
    el("div", "card-title-sm break-word", selected.name),
  );
  card.appendChild(head);

  const metaSize = el("div", "lv");
  metaSize.append(el("span", "lv-label", t("files.inspectorSize")), el("span", "", formatBytes(selected.sizeBytes)));
  const metaType = el("div", "lv");
  metaType.append(
    el("span", "lv-label", t("files.inspectorType")),
    el("span", "", selected.kind === "csv" ? t("files.typeCsv") : t("files.typeOther")),
  );
  card.append(metaSize, metaType);

  const actions = el("div", "stack-sm mt-sm");
  if (selected.kind === "csv") {
    actions.appendChild(
      button(t("files.openInClean"), "primary", () => {
        void openCleanFile(selected.id);
      }, "clean"),
    );
  } else {
    const noteKey = isExcelFile(selected.name) ? "files.excelNotSupported" : "files.otherNotSupported";
    actions.appendChild(el("div", "note note-amber", t(noteKey)));
  }

  actions.appendChild(
    button(t("files.moveToTrash"), "outline", async () => {
      if (!state.activeProject) return;
      try {
        await api.fileTrash(state.activeProject.id, selected.id);
        await refreshProjectData();
        setNotice("green", t("files.movedToTrash", { name: selected.name }));
      } catch (err) {
        showError(err);
      }
    }, "trash"),
  );
  card.appendChild(actions);

  const trashRow = el("div", "lv mt-sm");
  const trashBtn = button(t("files.trashLink"), "ghost", () => navigate("backup"), "trash");
  trashRow.append(trashBtn, badge(String(state.trash.length), "grey"));
  card.appendChild(trashRow);

  return card;
}

export function renderFilesScreen(): { header: HTMLElement; body: HTMLElement } {
  const totalBytes = state.files.reduce((sum, f) => sum + f.sizeBytes, 0);
  const sub = state.activeProject
    ? t("files.subtitle", {
        project: state.activeProject.name,
        count: t("files.count", undefined, state.files.length),
        size: formatBytes(totalBytes),
      })
    : t("files.subtitleEmpty");

  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(el("h1", "topbar-title", t("files.title")), el("div", "topbar-sub", sub));
  const headActions = el("div", "topbar-actions");
  if (state.activeProject) {
    headActions.appendChild(
      button(t("common.refresh"), "outline", async () => {
        try {
          await refreshProjectData();
          renderApp();
        } catch (err) {
          showError(err);
        }
      }, "refresh"),
    );
  }
  headActions.appendChild(
    button(t("files.openFolder"), "outline", () => {
      void pickAndOpenProject();
    }, "folder"),
  );
  header.append(headText, headActions);

  const body = el("div", "page-body");
  if (!state.activeProject) {
    const empty = el("div", "card");
    empty.appendChild(el("p", "muted", t("files.noProject")));
    const row = el("div", "mt-sm");
    row.appendChild(
      button(t("files.openFolder"), "primary", () => {
        void pickAndOpenProject();
      }, "folder"),
    );
    empty.appendChild(row);
    body.appendChild(empty);
    return { header, body };
  }

  body.appendChild(el("div", "note note-green", t("files.note")));
  const split = el("div", "files-split");
  const selected = state.files.find((f) => f.id === state.selectedFileId) ?? state.files[0];
  if (selected && !state.selectedFileId) {
    state.selectedFileId = selected.id;
  }
  split.append(renderFileList(), renderInspector(selected));
  body.appendChild(split);

  return { header, body };
}
