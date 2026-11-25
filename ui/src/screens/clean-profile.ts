import { button, el } from "../dom.js";
import { createVirtualGrid, type VirtualGridHandle } from "../grid.js";
import { formatNumber, t } from "../i18n.js";
import { clearSuccessNotice, renderApp, state } from "../state.js";
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

function renderIssueRow(
  index: number,
  title: string,
  meta: string,
  suggested: Step | null,
  onSelectStep: (step: Step | null) => void,
): HTMLElement {
  const row = el("div", "lv row-between");
  const leftInfo = el("div", "row-gap-sm grow min-w-0");
  leftInfo.appendChild(el("span", "step-num step-num-amber", String(index)));
  const textWrap = el("div", "min-w-0");
  textWrap.append(el("div", "bold", title), el("div", "muted", meta));
  leftInfo.appendChild(textWrap);
  row.appendChild(leftInfo);

  if (suggested) {
    row.appendChild(
      button(t("clean.issueFix"), "outline", () => {
        clearSuccessNotice();
        onSelectStep(suggested);
        state.cleanTab = "fix";
        renderApp();
      }),
    );
  }
  return row;
}

export function renderCleanProfile(
  onGridMount: (grid: VirtualGridHandle) => void,
  onSelectStep: (step: Step | null) => void,
): HTMLElement {
  const session = state.cleanSession!;
  const profile = state.cleanProfile;
  const spelling = state.cleanSpellingSuggestions;
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

  const enabledSteps = session.state.steps.filter((s) => s.enabled).length;
  const noteText =
    enabledSteps > 0
      ? t("clean.profileNoteWithSteps", { steps: t("steps.count", undefined, enabledSteps) })
      : t("clean.profileNote");
  left.appendChild(el("div", "note note-neutral", noteText));

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
  const totalIssues = (profile?.issues.length ?? 0) + spelling.length;
  if (!profile || totalIssues === 0) {
    right.appendChild(el("p", "muted", t("clean.issuesEmpty")));
  } else {
    right.appendChild(el("p", "muted", t("clean.issuesSub", undefined, totalIssues)));
    let num = 1;
    for (const issue of profile.issues) {
      const colText = issue.column ?? t("clean.allColumns");
      const countText =
        issue.kind === "duplicateRows"
          ? t("rows.count", undefined, issue.count)
          : t("cells.count", undefined, issue.count);
      const meta = t("clean.issueMeta", { column: colText, count: countText });
      right.appendChild(renderIssueRow(num++, t(issueTitleKey(issue.kind)), meta, stepForIssue(issue), onSelectStep));
    }
    for (const item of spelling) {
      const meta = t("clean.issueMeta", {
        column: item.column,
        count: t("groups.count", undefined, item.groups.length),
      });
      const step: Step = { type: "mergeSpellings", column: item.column, groups: item.groups };
      right.appendChild(renderIssueRow(num++, t("clean.issueLookAlikeSpellings"), meta, step, onSelectStep));
    }

    const reviewWrap = el("div", "mt-md");
    reviewWrap.appendChild(
      button(t("clean.reviewFixes"), "primary", () => {
        clearSuccessNotice();
        const firstIssue = profile.issues.map(stepForIssue).find((s) => s !== null) ?? null;
        const firstSpelling: Step | null =
          spelling.length > 0
            ? { type: "mergeSpellings", column: spelling[0].column, groups: spelling[0].groups }
            : null;
        onSelectStep(firstIssue ?? firstSpelling);
        state.cleanTab = "fix";
        renderApp();
      }, "arrow"),
    );
    right.appendChild(reviewWrap);
  }

  split.append(left, right);
  return split;
}
