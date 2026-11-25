import { api } from "../api.js";
import { badge, button, el } from "../dom.js";
import { t } from "../i18n.js";
import { createVirtualGrid, type VirtualGridHandle } from "../grid.js";
import {
  refreshCleanProfile,
  refreshProjectData,
  renderApp,
  setNotice,
  showError,
  state,
} from "../state.js";
import type { Issue, NeedsPerson, SpellingGroup, Step, StepEntry, TargetType } from "../types.js";
import { stepForIssue } from "./clean-profile.js";

type StepKind = Step["type"];

const STEP_OPTIONS: [StepKind, string][] = [
  ["trimSpaces", "clean.stepTrimSpaces"],
  ["makeDates", "clean.stepMakeDates"],
  ["textToNumbers", "clean.stepTextToNumbers"],
  ["mergeSpellings", "clean.stepMergeSpellings"],
  ["removeDuplicateRows", "clean.stepRemoveDuplicates"],
  ["removeEmptyRows", "clean.stepRemoveEmptyRows"],
  ["renameColumn", "clean.stepRenameColumn"],
  ["changeType", "clean.stepChangeType"],
];

const ACTIONABLE_ISSUES = new Set<Issue["kind"]>([
  "duplicateRows",
  "extraSpaces",
  "datesNotReal",
  "numbersWithText",
]);

let draftKind: StepKind = "trimSpaces";
let draftCol = "";
let draftDayFirst = true;
let draftNewName = "";
let draftTargetType: TargetType = "number";
let suggestedGroups: SpellingGroup[] = [];
let selectedGroupIdx = new Set<number>();
let loadedGroupsFor = "";

export function formatStepName(step: Step): string {
  switch (step.type) {
    case "trimSpaces":
      if (!step.columns || step.columns.length === 0) {
        return t("clean.stepNameTrimAll");
      }
      return t("clean.stepNameTrimCols", { columns: step.columns.join(", ") });
    case "makeDates":
      return t("clean.stepNameMakeDates", { column: step.column });
    case "textToNumbers":
      return t("clean.stepNameTextToNumbers", { column: step.column });
    case "mergeSpellings":
      return t("clean.stepNameMergeSpellings", { column: step.column });
    case "removeDuplicateRows":
      return t("clean.stepNameRemoveDuplicates");
    case "removeEmptyRows":
      return t("clean.stepNameRemoveEmptyRows");
    case "renameColumn":
      return t("clean.stepNameRenameColumn", { from: step.from, to: step.to });
    case "changeType": {
      const typeKey = `clean.targetType${step.to[0].toUpperCase()}${step.to.slice(1)}`;
      return t("clean.stepNameChangeType", { column: step.column, type: t(typeKey) });
    }
  }
}

function isTextColumn(name: string): boolean {
  const col = state.cleanProfile?.columns.find((c) => c.name === name);
  return col?.detectedType === "text";
}

function defaultTextColumn(columns: string[]): string {
  const withGroups = state.cleanSpellingSuggestions.find((s) => columns.includes(s.column));
  if (withGroups) return withGroups.column;
  const textCol = state.cleanProfile?.columns.find(
    (c) => c.detectedType === "text" && columns.includes(c.name),
  );
  return textCol?.name ?? columns[0] ?? "";
}

function applyDraftFields(step: Step): void {
  draftKind = step.type;
  loadedGroupsFor = "";
  if (step.type === "makeDates") {
    draftCol = step.column;
    draftDayFirst = step.dayFirst;
  } else if (step.type === "textToNumbers") {
    draftCol = step.column;
  } else if (step.type === "mergeSpellings") {
    draftCol = step.column;
    if (step.groups.length > 0 && state.cleanSession) {
      suggestedGroups = step.groups;
      selectedGroupIdx = new Set(step.groups.map((_, i) => i));
      loadedGroupsFor = `${state.cleanSession.sessionId}:${step.column}`;
    }
  } else if (step.type === "renameColumn") {
    draftCol = step.from;
    draftNewName = step.to;
  } else if (step.type === "changeType") {
    draftCol = step.column;
    draftTargetType = step.to;
  } else if (step.type === "trimSpaces") {
    draftCol = step.columns?.[0] ?? "";
  }
}

export function seedDraftFromStep(step: Step | null): void {
  state.cleanPreviewStep = step;
  if (step) {
    applyDraftFields(step);
  }
}

function stepSignature(step: Step): string {
  switch (step.type) {
    case "trimSpaces":
      return `trimSpaces:${step.columns ? step.columns.join(",") : "*"}`;
    case "makeDates":
      return `makeDates:${step.column}`;
    case "textToNumbers":
      return `textToNumbers:${step.column}`;
    case "mergeSpellings":
      return `mergeSpellings:${step.column}`;
    case "removeDuplicateRows":
      return "removeDuplicateRows";
    case "removeEmptyRows":
      return "removeEmptyRows";
    case "renameColumn":
      return `renameColumn:${step.from}:${step.to}`;
    case "changeType":
      return `changeType:${step.column}:${step.to}`;
  }
}

function pickNextUsefulStep(columns: string[], applied: StepEntry[]): Step | null {
  const done = new Set(applied.filter((s) => s.enabled).map((s) => stepSignature(s.step)));
  const issues = state.cleanProfile?.issues ?? [];

  for (const issue of issues) {
    if (!ACTIONABLE_ISSUES.has(issue.kind)) continue;
    const step = stepForIssue(issue);
    if (step && !done.has(stepSignature(step))) return step;
  }

  for (const suggestion of state.cleanSpellingSuggestions) {
    if (!columns.includes(suggestion.column)) continue;
    const step: Step = {
      type: "mergeSpellings",
      column: suggestion.column,
      groups: suggestion.groups,
    };
    if (!done.has(stepSignature(step))) return step;
  }

  for (const issue of issues) {
    const step = stepForIssue(issue);
    if (step && !done.has(stepSignature(step))) return step;
  }

  return null;
}

function buildDraftStep(columns: string[]): Step | null {
  const col = draftCol || columns[0] || "";
  switch (draftKind) {
    case "trimSpaces":
      return { type: "trimSpaces", columns: draftCol ? [draftCol] : null };
    case "makeDates":
      return col ? { type: "makeDates", column: col, dayFirst: draftDayFirst } : null;
    case "textToNumbers":
      return col ? { type: "textToNumbers", column: col } : null;
    case "mergeSpellings": {
      if (!col) return null;
      const picked = suggestedGroups.filter((_, i) => selectedGroupIdx.has(i));
      return { type: "mergeSpellings", column: col, groups: picked };
    }
    case "removeDuplicateRows":
      return { type: "removeDuplicateRows" };
    case "removeEmptyRows":
      return { type: "removeEmptyRows" };
    case "renameColumn": {
      const to = draftNewName.trim();
      return col && to ? { type: "renameColumn", from: col, to } : null;
    }
    case "changeType":
      return col ? { type: "changeType", column: col, to: draftTargetType } : null;
  }
}

function reasonText(reason: NeedsPerson["reason"]): string {
  if (reason === "emptyNumber") return t("clean.reasonEmptyNumber");
  if (reason === "unreadableNumber") return t("clean.reasonUnreadableNumber");
  return t("clean.reasonUnreadableDate");
}

function renderSpellingPicker(columns: string[], sessionId: string): HTMLElement {
  const boxWrap = el("div", "mt-sm stack-xs");
  const cacheKey = `${sessionId}:${draftCol}`;
  if (loadedGroupsFor !== cacheKey) {
    loadedGroupsFor = cacheKey;
    boxWrap.appendChild(el("div", "muted", t("common.loading")));
    void api
      .cleanSuggestGroups(sessionId, draftCol)
      .then((groups) => {
        if (loadedGroupsFor !== cacheKey) return;
        suggestedGroups = groups;
        selectedGroupIdx = new Set(groups.map((_, i) => i));
        if (state.cleanPreviewStep?.type === "mergeSpellings") {
          state.cleanPreviewStep = buildDraftStep(columns);
        }
        renderApp();
      })
      .catch(showError);
    return boxWrap;
  }
  if (suggestedGroups.length === 0) {
    boxWrap.appendChild(el("div", "muted", t("clean.spellingNone")));
    return boxWrap;
  }
  boxWrap.appendChild(el("div", "muted", t("clean.spellingSuggestHint")));
  suggestedGroups.forEach((g, idx) => {
    const row = el("label", "check-row");
    const cb = el("input");
    cb.type = "checkbox";
    cb.checked = selectedGroupIdx.has(idx);
    cb.addEventListener("change", () => {
      if (cb.checked) selectedGroupIdx.add(idx);
      else selectedGroupIdx.delete(idx);
      state.cleanPreviewStep = buildDraftStep(columns);
      renderApp();
    });
    const info = el("div");
    info.append(
      el("div", "bold", g.canonical),
      el("div", "muted", t("clean.spellingVariants", { variants: g.variants.join(", ") })),
    );
    row.append(cb, info);
    boxWrap.appendChild(row);
  });
  return boxWrap;
}

function renderKindExtras(columns: string[], sessionId: string): HTMLElement | null {
  if (draftKind === "makeDates") {
    const checkRow = el("label", "check-row mt-sm");
    const box = el("input");
    box.type = "checkbox";
    box.checked = draftDayFirst;
    box.addEventListener("change", () => {
      draftDayFirst = box.checked;
      state.cleanPreviewStep = buildDraftStep(columns);
      renderApp();
    });
    checkRow.append(box, el("span", "", t("clean.dayFirstLabel")));
    return checkRow;
  }
  if (draftKind === "renameColumn") {
    const wrap = el("div", "mt-sm");
    const lbl = el("label", "field-label", t("clean.newNameLabel"));
    lbl.htmlFor = "clean-rename-to";
    const inp = el("input", "inp");
    inp.id = "clean-rename-to";
    inp.type = "text";
    inp.value = draftNewName;
    inp.addEventListener("input", () => {
      draftNewName = inp.value;
    });
    wrap.append(lbl, inp);
    return wrap;
  }
  if (draftKind === "changeType") {
    const wrap = el("div", "mt-sm");
    const lbl = el("label", "field-label", t("clean.targetTypeLabel"));
    lbl.htmlFor = "clean-target-type";
    const sel = el("select", "inp");
    sel.id = "clean-target-type";
    const types: [TargetType, string][] = [
      ["text", "clean.targetTypeText"],
      ["number", "clean.targetTypeNumber"],
      ["date", "clean.targetTypeDate"],
    ];
    for (const [val, key] of types) {
      const opt = el("option", "", t(key));
      opt.value = val;
      sel.appendChild(opt);
    }
    sel.value = draftTargetType;
    sel.addEventListener("change", () => {
      draftTargetType = sel.value as TargetType;
      state.cleanPreviewStep = buildDraftStep(columns);
      renderApp();
    });
    wrap.append(lbl, sel);
    return wrap;
  }
  if (draftKind === "mergeSpellings" && draftCol) {
    return renderSpellingPicker(columns, sessionId);
  }
  return null;
}

function renderStepBuilder(columns: string[], sessionId: string): HTMLElement {
  const card = el("div", "card");
  card.appendChild(el("h2", "card-title", t("clean.addStepHeading")));

  if (draftKind === "mergeSpellings" && (!draftCol || !columns.includes(draftCol))) {
    draftCol = defaultTextColumn(columns);
  }

  const kindLabel = el("label", "field-label", t("clean.stepKindLabel"));
  kindLabel.htmlFor = "clean-step-kind";
  const kindSel = el("select", "inp");
  kindSel.id = "clean-step-kind";
  for (const [val, key] of STEP_OPTIONS) {
    const opt = el("option", "", t(key));
    opt.value = val;
    kindSel.appendChild(opt);
  }
  kindSel.value = draftKind;
  kindSel.addEventListener("change", () => {
    draftKind = kindSel.value as StepKind;
    loadedGroupsFor = "";
    if (draftKind === "mergeSpellings" && !isTextColumn(draftCol)) {
      draftCol = defaultTextColumn(columns);
    }
    state.cleanPreviewStep = buildDraftStep(columns);
    renderApp();
  });
  card.append(kindLabel, kindSel);

  const needsCol = draftKind !== "removeDuplicateRows" && draftKind !== "removeEmptyRows";
  if (needsCol && columns.length > 0) {
    const colLabel = el("label", "field-label mt-sm", t("clean.columnLabel"));
    colLabel.htmlFor = "clean-step-col";
    const colSel = el("select", "inp");
    colSel.id = "clean-step-col";
    if (draftKind === "trimSpaces") {
      const allOpt = el("option", "", t("clean.allColumns"));
      allOpt.value = "";
      colSel.appendChild(allOpt);
    }
    for (const c of columns) {
      const opt = el("option", "", c);
      opt.value = c;
      colSel.appendChild(opt);
    }
    if (draftKind !== "trimSpaces" && (!draftCol || !columns.includes(draftCol))) {
      draftCol = draftKind === "mergeSpellings" ? defaultTextColumn(columns) : columns[0];
    }
    colSel.value = draftKind === "trimSpaces" ? draftCol : draftCol;
    colSel.addEventListener("change", () => {
      draftCol = colSel.value;
      loadedGroupsFor = "";
      state.cleanPreviewStep = buildDraftStep(columns);
      renderApp();
    });
    card.append(colLabel, colSel);
  }

  const extra = renderKindExtras(columns, sessionId);
  if (extra) card.appendChild(extra);

  const actions = el("div", "btn-row mt-md");
  actions.appendChild(
    button(t("clean.previewStep"), "outline", () => {
      state.cleanPreviewStep = buildDraftStep(columns);
      renderApp();
    }),
  );
  actions.appendChild(
    button(t("clean.applyStep"), "primary", async () => {
      const step = buildDraftStep(columns);
      if (!step) return;
      try {
        const nextState = await api.cleanAddStep(sessionId, step);
        state.cleanSession!.state = nextState;
        state.cleanPreviewStep = null;
        loadedGroupsFor = "";
        await Promise.all([refreshCleanProfile(), refreshProjectData()]);
        const nextFix = pickNextUsefulStep(nextState.columns, nextState.steps);
        if (nextFix) {
          applyDraftFields(nextFix);
        }
        renderApp();
      } catch (err) {
        showError(err);
      }
    }, "check"),
  );
  if (state.cleanPreviewStep) {
    actions.appendChild(
      button(t("clean.clearPreview"), "ghost", () => {
        state.cleanPreviewStep = null;
        renderApp();
      }),
    );
  }
  card.appendChild(actions);
  return card;
}

function renderStepsList(sessionId: string): HTMLElement {
  const steps = state.cleanSession!.state.steps;
  const card = el("div", "card");
  card.appendChild(el("h2", "card-title", t("clean.stepsListHeading", { count: steps.length })));

  if (steps.length === 0) {
    card.appendChild(el("p", "muted", t("clean.stepsEmpty")));
    return card;
  }

  steps.forEach((entry, idx) => {
    const row = el("div", "lv row-between step-list-row");
    const left = el("label", "check-row step-list-label grow min-w-0");
    const toggle = el("input");
    toggle.type = "checkbox";
    toggle.checked = entry.enabled;
    toggle.addEventListener("change", async () => {
      try {
        state.cleanSession!.state = await api.cleanSetEnabled(sessionId, idx, toggle.checked);
        await Promise.all([refreshCleanProfile(), refreshProjectData()]);
        renderApp();
      } catch (err) {
        showError(err);
      }
    });
    const nameText = `${idx + 1}. ${formatStepName(entry.step)}`;
    const text = el("span", entry.enabled ? "bold step-name-text" : "muted step-name-text", nameText);
    left.append(toggle, text);

    const ctrls = el("div", "btn-row-tight");
    if (idx > 0) {
      ctrls.appendChild(
        button(t("clean.stepMoveUp"), "ghost", async () => {
          try {
            state.cleanSession!.state = await api.cleanMoveStep(sessionId, idx, idx - 1);
            await refreshCleanProfile();
            renderApp();
          } catch (err) {
            showError(err);
          }
        }),
      );
    }
    if (idx < steps.length - 1) {
      ctrls.appendChild(
        button(t("clean.stepMoveDown"), "ghost", async () => {
          try {
            state.cleanSession!.state = await api.cleanMoveStep(sessionId, idx, idx + 1);
            await refreshCleanProfile();
            renderApp();
          } catch (err) {
            showError(err);
          }
        }),
      );
    }
    ctrls.appendChild(
      button(t("clean.stepRemove"), "ghost", async () => {
        try {
          state.cleanSession!.state = await api.cleanRemoveStep(sessionId, idx);
          await Promise.all([refreshCleanProfile(), refreshProjectData()]);
          renderApp();
        } catch (err) {
          showError(err);
        }
      }),
    );
    row.append(left, ctrls);
    card.appendChild(row);
  });

  return card;
}

export function renderNeedsPersonCard(sessionId: string): HTMLElement | null {
  const { needsPerson, summary } = state.cleanSession!.state;
  if (needsPerson.length === 0) return null;

  const card = el("div", "card");
  card.append(
    el("h2", "card-title", t("clean.needsPersonHeading", { count: summary.rowsNeedingPerson })),
    el("p", "muted", t("clean.needsPersonNote")),
  );

  if (summary.rowsNeedingPerson > needsPerson.length) {
    card.appendChild(
      el(
        "div",
        "note note-amber mt-sm",
        t("clean.needsPersonCapped", { shown: needsPerson.length, total: summary.rowsNeedingPerson }),
      ),
    );
  }

  const list = el("div", "needs-list mt-sm");
  const visibleItems = needsPerson.slice(0, 25);
  for (const item of visibleItems) {
    const row = el("div", "lv needs-row");
    const info = el("div", "grow min-w-0");
    info.append(
      el("div", "bold", `Row ${item.row + 1} · ${item.columnName}`),
      el("div", "muted", `${item.value || "(empty)"} · ${reasonText(item.reason)}`),
    );
    const inp = el("input", "inp inp-xs");
    inp.type = "text";
    inp.value = item.value;
    inp.placeholder = t("clean.fixCellPlaceholder");
    inp.setAttribute("aria-label", `${item.columnName} row ${item.row + 1}`);
    const submitFix = async (): Promise<void> => {
      try {
        state.cleanSession!.state = await api.cleanFixCell(sessionId, item.row, item.column, inp.value);
        await refreshCleanProfile();
        setNotice("green", t("clean.cellFixed", { row: item.row + 1, column: item.columnName }));
      } catch (err) {
        showError(err);
      }
    };
    inp.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        void submitFix();
      }
    });
    const saveBtn = button(t("clean.fixCellSave"), "outline", () => void submitFix());
    row.append(info, inp, saveBtn);
    list.appendChild(row);
  }
  card.appendChild(list);
  return card;
}

export function renderCleanFix(
  onGridMount: (grid: VirtualGridHandle) => void,
  onSave: () => void,
): HTMLElement {
  const session = state.cleanSession!;
  const split = el("div", "clean-split");

  const left = el("div", "stack-sm min-w-0");
  const headingRow = el("div", "row-between");
  headingRow.appendChild(el("h2", "section-title", t("clean.fixPreviewHeading")));
  if (state.cleanPreviewStep) {
    const stepLabel = formatStepName(state.cleanPreviewStep);
    headingRow.appendChild(badge(t("clean.fixPreviewActive", { name: stepLabel }), "blue"));
  }
  left.appendChild(headingRow);

  const grid = createVirtualGrid({
    sessionId: session.sessionId,
    rowCount: session.state.rowCount,
    columns: session.state.columns,
    columnProfiles: state.cleanProfile?.columns,
    previewStep: state.cleanPreviewStep,
  });
  onGridMount(grid);
  left.appendChild(grid.element);
  left.appendChild(el("div", "note note-neutral", t("clean.fixPreviewNote")));

  const saveCard = el("div", "card row-between");
  const saveText = el("div");
  saveText.append(
    el("div", "card-title-sm", t("clean.saveNew")),
    el("div", "muted", t("clean.saveHint")),
  );
  saveCard.append(saveText, button(t("clean.saveNew"), "primary", onSave, "save"));
  left.appendChild(saveCard);

  const right = el("div", "stack-md");
  right.append(renderStepBuilder(session.state.columns, session.sessionId));
  right.append(renderStepsList(session.sessionId));
  const needsCard = renderNeedsPersonCard(session.sessionId);
  if (needsCard) right.append(needsCard);

  split.append(left, right);
  return split;
}

export function resetFixDraft(): void {
  loadedGroupsFor = "";
  suggestedGroups = [];
  selectedGroupIdx = new Set();
}
