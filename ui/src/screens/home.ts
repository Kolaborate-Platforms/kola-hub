import { api } from "../api.js";
import { badge, button, el, icon, progressBar } from "../dom.js";
import { formatBytes, formatRelativeTime, t } from "../i18n.js";
import {
  navigate,
  openProjectView,
  pickAndOpenProject,
  renderApp,
  showError,
  state,
} from "../state.js";
import type { RecentProject } from "../types.js";

function hasRecovery(): boolean {
  const rec = state.activeProject?.recovered;
  if (!rec) return false;
  return rec.finishedMoves > 0 || rec.rolledBack > 0 || rec.recovered > 0 || rec.dropped > 0;
}

function renderRecentCard(proj: RecentProject): HTMLElement {
  const isActive = state.activeProject?.id === proj.id;
  const card = el("div", isActive ? "card card-active" : "card project-card");

  const top = el("div", "row-between");
  top.appendChild(icon("folder", "ic-md"));
  if (isActive) {
    top.appendChild(badge(t("home.projectActive"), "green"));
  } else if (proj.available) {
    top.appendChild(badge(t("home.projectAvailable"), "grey"));
  } else {
    top.appendChild(badge(t("home.projectMissing"), "amber"));
  }

  const name = el("div", "card-title-sm", proj.name);
  const when = el("div", "muted", t("home.lastOpened", { when: formatRelativeTime(proj.lastOpened) }));

  const actions = el("div", "btn-row");
  if (!isActive && proj.available) {
    actions.appendChild(
      button(t("home.projectOpen"), "primary", async () => {
        try {
          await openProjectView(await api.projectOpen(proj.id));
        } catch (err) {
          showError(err);
        }
      }),
    );
  }
  actions.appendChild(
    button(t("home.projectForget"), "ghost", async () => {
      try {
        await api.projectForget(proj.id);
        if (state.activeProject?.id === proj.id) {
          state.activeProject = null;
          state.files = [];
          state.trash = [];
        }
        state.recentProjects = await api.projectsRecent();
        renderApp();
      } catch (err) {
        showError(err);
      }
    }),
  );

  card.append(top, name, when, actions);
  return card;
}

function renderToolsSection(): HTMLElement {
  const section = el("section", "section");
  section.appendChild(el("h2", "section-title", t("home.toolsHeading")));
  const grid = el("div", "grid-2");

  const cleanTile = el("button", "tool-tile");
  cleanTile.type = "button";
  cleanTile.appendChild(icon("clean", "ic-md"));
  const cleanInfo = el("div", "tile-body");
  cleanInfo.append(
    el("div", "card-title-sm", t("rail.clean")),
    el("div", "muted", t("home.toolCleanDesc")),
  );
  cleanTile.appendChild(cleanInfo);
  cleanTile.addEventListener("click", () => navigate("clean"));

  const annTile = el("button", "tool-tile tool-tile-dashed");
  annTile.type = "button";
  annTile.appendChild(icon("tag", "ic-md"));
  const annInfo = el("div", "tile-body");
  annInfo.append(
    el("div", "card-title-sm", t("rail.annotate")),
    el("div", "muted", t("home.toolAnnotateDesc")),
  );
  annTile.append(annInfo, badge(t("rail.notBuiltTag"), "grey"));
  annTile.addEventListener("click", () => navigate("annotate"));

  grid.append(cleanTile, annTile);
  section.appendChild(grid);
  return section;
}

function renderBottomCards(): HTMLElement {
  const grid = el("div", "grid-2");

  const diskCard = el("div", "card");
  diskCard.appendChild(el("h2", "card-title", t("home.diskHeading")));
  if (state.activeProject) {
    const { usedBytes, limitBytes, status } = state.activeProject.budget;
    const pct = limitBytes > 0 ? Math.round((usedBytes / limitBytes) * 100) : 0;
    diskCard.appendChild(
      el("p", "muted", t("home.diskUsage", { used: formatBytes(usedBytes), limit: formatBytes(limitBytes) })),
    );
    diskCard.appendChild(progressBar(pct));
    if (status === "warn") {
      diskCard.appendChild(el("div", "note note-amber mt-sm", t("home.diskWarn")));
    } else if (status === "over") {
      diskCard.appendChild(el("div", "note note-amber mt-sm", t("home.diskOver")));
    }
  } else {
    diskCard.appendChild(el("p", "muted", t("home.diskNoProject")));
  }

  const trashCard = el("div", "card");
  trashCard.appendChild(el("h2", "card-title", t("home.trashHeading")));
  if (state.trash.length > 0) {
    const itemsText = t("items.count", undefined, state.trash.length);
    const daysText = t("days.count", undefined, state.settings.trashRetentionDays);
    trashCard.appendChild(el("p", "muted", t("home.trashSummary", { items: itemsText, days: daysText })));
  } else {
    trashCard.appendChild(el("p", "muted", t("home.trashEmpty")));
  }
  const trashActions = el("div", "mt-sm");
  trashActions.appendChild(button(t("home.trashOpen"), "outline", () => navigate("backup"), "trash"));
  trashCard.appendChild(trashActions);

  grid.append(diskCard, trashCard);
  return grid;
}

function renderFirstRunScreen(): { header: HTMLElement; body: HTMLElement } {
  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("home.title")),
    el("div", "topbar-sub", t("home.subtitle", { version: state.version })),
  );
  header.appendChild(headText);

  const body = el("div", "page-body");
  const card = el("div", "card first-run-card");
  card.append(
    badge(t("home.firstRunTag"), "grey"),
    el("h2", "hero-title mt-sm", t("home.firstRunTitle")),
    el("p", "hero-sub mt-sm", t("home.firstRunBody")),
  );

  const folderRow = el("div", "lv mt-md");
  const folderText = el("div");
  folderText.append(
    el("div", "bold", t("home.firstRunFolderHeading")),
    el("div", "muted", t("home.firstRunFolderBody")),
  );
  folderRow.append(icon("folder", "ic-md"), folderText);

  const offlineRow = el("div", "lv");
  const offlineText = el("div");
  offlineText.append(
    el("div", "bold", t("home.firstRunOfflineHeading")),
    el("div", "muted", t("home.firstRunOfflineBody")),
  );
  offlineRow.append(icon("offline", "ic-md"), offlineText);

  const actions = el("div", "mt-md");
  actions.appendChild(
    button(t("home.openFolder"), "primary", () => {
      void pickAndOpenProject();
    }, "folder"),
  );

  card.append(folderRow, offlineRow, actions);
  body.appendChild(card);
  return { header, body };
}

export function renderHomeScreen(): { header: HTMLElement; body: HTMLElement } {
  if (state.recentProjects.length === 0 && !state.activeProject) {
    return renderFirstRunScreen();
  }

  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("home.title")),
    el("div", "topbar-sub", t("home.subtitle", { version: state.version })),
  );
  const headActions = el("div", "topbar-actions");
  headActions.appendChild(
    button(t("home.openFolder"), "primary", () => {
      void pickAndOpenProject();
    }, "folder"),
  );
  header.append(headText, headActions);

  const body = el("div", "page-body");
  if (hasRecovery()) {
    body.appendChild(el("div", "note note-green", t("home.recoveredNotice")));
  }

  const hero = el("div", "hero");
  const summaryText =
    state.recentProjects.length === 0
      ? t("home.summaryEmpty")
      : t("home.summaryProjects", undefined, state.recentProjects.length);
  hero.append(el("div", "hero-title", t("home.greeting")), el("div", "hero-sub", summaryText));
  body.appendChild(hero);

  const recentSec = el("section", "section");
  recentSec.appendChild(el("h2", "section-title", t("home.recentHeading")));
  if (state.recentProjects.length === 0) {
    const emptyCard = el("div", "card");
    emptyCard.appendChild(el("p", "muted", t("home.recentEmpty")));
    const row = el("div", "mt-sm");
    row.appendChild(
      button(t("home.openFolder"), "primary", () => {
        void pickAndOpenProject();
      }, "folder"),
    );
    emptyCard.appendChild(row);
    recentSec.appendChild(emptyCard);
  } else {
    const grid = el("div", "grid-3");
    for (const proj of state.recentProjects) {
      grid.appendChild(renderRecentCard(proj));
    }
    recentSec.appendChild(grid);
  }
  body.appendChild(recentSec);
  body.appendChild(renderToolsSection());
  body.appendChild(renderBottomCards());

  return { header, body };
}
