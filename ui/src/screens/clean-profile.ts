import { button, el } from "../dom.js";
import { createVirtualGrid, type VirtualGridHandle } from "../grid.js";
import { formatNumber, t } from "../i18n.js";
import { renderApp, state } from "../state.js";
import type { Issue, Step } from "../types.js";

function issueTitleKey(kind: Issue["kind"]): string {
  switch (kind) {
    case "duplicateRows":
      return "clean.issueDuplicateRows";
    case "extraSpaces":
      return "clean.issueExtraSpaces";
    case "datesNotReal":
      return "clean.issueDatesNotReal";
    case "unreadableDates":
      return "clean.issueUnreadableDates";
    case "numbersWithText":
      return "clean.issueNumbersWithText";
    case "unreadableNumbers":
      return "clean.issueUnreadableNumbers";
    case "emptyCells":
      return "clean.issueEmptyCells";
  }
}

export function stepForIssue(issue: Issue): Step | null {
  switch (issue.kind) {
    case "duplicateRows":
      return { type: "removeDuplicateRows" };
    case "extraSpaces":
      return { type: "trimSpaces", columns: issue.column ? [issue.column] : null };
    case "datesNotReal":
    case "unreadableDates":
      return issue.column ? { type: "makeDates", column: issue.column, dayFirst: true } : null;
    case "numbersWithText":
    case "unreadableNumbers":
    case "emptyCells":
      return issue.column ? { type: "textToNumbers", column: issue.column } : null;
  }
}

function collectIssueCells(issues: Issue[]): Set<string> {
  const set = new Set<string>();
  for (const issue of issues) {
    for (const cell of issue.cells) {
      set.add(`${cell.row}:${cell.column}`);
    }
  }
  return set;
}

export function renderCleanProfile(
  onGridMount: (grid: VirtualGridHandle) => void,
  onSelectStep: (step: Step | null) => void,
): HTMLElement {
  const session = state.cleanSession!;
  const profile = state.cleanProfile;
  const split = el("div", "clean-split");

  const left = el("div", "stack-sm min-w-0");
  left.appendChild(
    el("h2", "section-title", t("clean.profileHeading", { rows: t("rows.count", undefined, session.state.rowCount) })),
  );

  const grid = createVirtualGrid({
    sessionId: session.sessionId,
    rowCount: session.state.rowCount,
    columns: session.state.columns,
    columnProfiles: profile?.columns,
    issueCells: profile ? collectIssueCells(profile.issues) : undefined,
  });
  onGridMount(grid);
  left.appendChild(grid.element);
  left.appendChild(el("div", "note note-neutral", t("clean.profileNote")));

  if (profile) {
    const stats = el("div", "card stats-row");
    const s1 = el("div");
    s1.append(el("div", "muted", t("clean.statRows")), el("div", "stat-val", formatNumber(profile.rows)));
    const s2 = el("div");
    s2.append(
      el("div", "muted", t("clean.statWrong")),
      el("div", "stat-val text-amber", formatNumber(profile.wrongCells)),
    );
    const s3 = el("div");
    s3.append(
      el("div", "muted", t("clean.statDuplicates")),
      el("div", "stat-val text-amber", formatNumber(profile.duplicateRows)),
    );
    stats.append(s1, s2, s3);
    left.appendChild(stats);
  }

  const right = el("div", "card");
  right.appendChild(el("h2", "card-title", t("clean.issuesHeading")));
  if (!profile || profile.issues.length === 0) {
    right.appendChild(el("p", "muted", t("clean.issuesEmpty")));
  } else {
    right.appendChild(el("p", "muted", t("clean.issuesSub", undefined, profile.issues.length)));
    profile.issues.forEach((issue, idx) => {
      const row = el("div", "lv row-between");
      const leftInfo = el("div", "row-gap-sm grow min-w-0");
      leftInfo.appendChild(el("span", "step-num step-num-amber", String(idx + 1)));
      const textWrap = el("div", "min-w-0");
      const colText = issue.column ?? t("clean.allColumns");
      const countText =
        issue.kind === "duplicateRows"
          ? t("rows.count", undefined, issue.count)
          : t("cells.count", undefined, issue.count);
      textWrap.append(
        el("div", "bold", t(issueTitleKey(issue.kind))),
        el("div", "muted", t("clean.issueMeta", { column: colText, count: countText })),
      );
      leftInfo.appendChild(textWrap);
      row.appendChild(leftInfo);

      const suggested = stepForIssue(issue);
      if (suggested) {
        row.appendChild(
          button(t("clean.issueFix"), "outline", () => {
            onSelectStep(suggested);
            state.cleanTab = "fix";
            renderApp();
          }),
        );
      }
      right.appendChild(row);
    });

    const reviewWrap = el("div", "mt-md");
    reviewWrap.appendChild(
      button(t("clean.reviewFixes"), "primary", () => {
        const first = profile.issues.map(stepForIssue).find((s) => s !== null) ?? null;
        onSelectStep(first);
        state.cleanTab = "fix";
        renderApp();
      }, "arrow"),
    );
    right.appendChild(reviewWrap);
  }

  split.append(left, right);
  return split;
}
