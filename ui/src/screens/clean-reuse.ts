import { api } from "../api.js";
import { badge, button, el } from "../dom.js";
import { formatNumber, t } from "../i18n.js";
import { refreshCleanProfile, renderApp, showError, state } from "../state.js";
import { renderNeedsPersonCard } from "./clean-fix.js";

let chosenSourceFileId = "";

export function renderCleanReuse(onSave: () => void): HTMLElement {
  const session = state.cleanSession!;
  const otherCsvs = state.files.filter(
    (f) => f.kind === "csv" && f.name !== session.fileName,
  );
  if (otherCsvs.length > 0 && !otherCsvs.some((f) => f.id === chosenSourceFileId)) {
    chosenSourceFileId = otherCsvs[0].id;
  }

  const grid = el("div", "grid-2");

  // Card 1: pick source file whose saved steps to reuse
  const pickCard = el("div", "card");
  pickCard.append(
    el("h2", "card-title", t("clean.reuseHeading")),
    el("p", "muted", t("clean.reuseDesc", { file: session.fileName })),
  );

  if (otherCsvs.length === 0) {
    pickCard.appendChild(el("div", "note note-neutral mt-sm", t("clean.reuseNoOtherFiles")));
  } else {
    const sel = el("select", "inp mt-sm");
    sel.setAttribute("aria-label", t("clean.reuseHeading"));
    for (const f of otherCsvs) {
      const opt = el("option", "", f.name);
      opt.value = f.id;
      sel.appendChild(opt);
    }
    sel.value = chosenSourceFileId;
    sel.addEventListener("change", () => {
      chosenSourceFileId = sel.value;
    });

    const runWrap = el("div", "mt-md");
    runWrap.appendChild(
      button(t("clean.reuseRun"), "primary", async () => {
        if (!chosenSourceFileId) return;
        try {
          const result = await api.cleanReuse(session.sessionId, chosenSourceFileId);
          state.cleanReuseResult = result;
          if (result.state) {
            state.cleanSession!.state = result.state;
            await refreshCleanProfile();
          }
          renderApp();
        } catch (err) {
          showError(err);
        }
      }, "arrow"),
    );
    pickCard.append(sel, runWrap);
  }
  grid.appendChild(pickCard);

  // Card 2: Match report
  const matchCard = el("div", "card");
  matchCard.appendChild(el("h2", "card-title", t("clean.reuseMatchHeading")));
  const reuse = state.cleanReuseResult;
  if (!reuse) {
    matchCard.appendChild(el("p", "muted", t("clean.reuseDesc", { file: session.fileName })));
  } else {
    for (const col of reuse.report.columns) {
      const row = el("div", "lv row-between");
      row.appendChild(el("span", "bold", col.name));
      if (col.found) {
        row.appendChild(badge(t("clean.reuseMatchOk"), "green"));
      } else if (col.repeated) {
        row.appendChild(badge(t("clean.reuseMatchRepeated"), "red"));
      } else {
        row.appendChild(badge(t("clean.reuseMatchMissing"), "red"));
      }
      matchCard.appendChild(row);
    }
    if (!reuse.state) {
      matchCard.appendChild(el("div", "note note-amber mt-sm", t("clean.reuseStopped")));
    }
  }
  grid.appendChild(matchCard);

  // Card 3: Result summary
  const summary = reuse?.state?.summary ?? session.state.summary;
  const resultCard = el("div", "card");
  resultCard.appendChild(el("h2", "card-title", t("clean.reuseResultHeading")));
  const stats = el("div", "stats-row mt-sm");
  const r1 = el("div");
  r1.append(el("div", "muted", t("clean.reuseRowsIn")), el("div", "stat-val", formatNumber(summary.rowsIn)));
  const r2 = el("div");
  r2.append(el("div", "muted", t("clean.reuseRowsOut")), el("div", "stat-val", formatNumber(summary.rowsOut)));
  const r3 = el("div");
  r3.append(
    el("div", "muted", t("clean.reuseNeedPerson")),
    el("div", "stat-val text-amber", formatNumber(summary.rowsNeedingPerson)),
  );
  stats.append(r1, r2, r3);
  resultCard.appendChild(stats);
  grid.appendChild(resultCard);

  // Card 4: Save
  const saveCard = el("div", "card");
  saveCard.append(
    el("h2", "card-title", t("clean.reuseSaveHeading")),
    el("p", "muted", t("clean.reuseSaveDesc", { file: session.fileName })),
  );
  const saveRow = el("div", "mt-md");
  saveRow.appendChild(button(t("clean.saveNew"), "primary", onSave, "save"));
  saveCard.appendChild(saveRow);
  grid.appendChild(saveCard);

  const wrap = el("div", "stack-md");
  wrap.appendChild(grid);

  const needsCard = renderNeedsPersonCard(session.sessionId);
  if (needsCard) {
    wrap.appendChild(needsCard);
  }
  return wrap;
}
