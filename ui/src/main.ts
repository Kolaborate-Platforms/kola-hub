import { api, isTauriRuntime } from "./api.js";
import { runBenchMode } from "./bench.js";
import { button, clear, el, icon, type IconName } from "./dom.js";
import { initStrings, t } from "./i18n.js";
import { closePalette, isPaletteOpen, openPalette, type PaletteCommand } from "./palette.js";
import { renderAnnotateScreen } from "./screens/annotate.js";
import { renderBackupScreen } from "./screens/backup.js";
import {
  destroyActiveCleanGrid,
  renderCleanScreen,
  runCleanRedo,
  runCleanSave,
  runCleanUndo,
} from "./screens/clean.js";
import { renderFilesScreen } from "./screens/files.js";
import { renderHomeScreen } from "./screens/home.js";
import { renderSettingsScreen } from "./screens/settings.js";
import {
  clearNotice,
  navigate,
  openProjectView,
  pickAndOpenProject,
  renderApp,
  setRenderer,
  showError,
  state,
} from "./state.js";
import type { Route } from "./types.js";

const ROUTE_ORDER: Route[] = ["home", "files", "clean", "annotate", "backup", "settings"];

function navButton(
  route: Route,
  labelKey: string,
  iconName: IconName,
  badgeKey?: string,
): HTMLButtonElement {
  const active = state.route === route;
  const btn = el("button", active ? "rail-nav active" : "rail-nav");
  btn.type = "button";
  btn.setAttribute("aria-current", active ? "page" : "false");
  btn.append(icon(iconName, "ic-md"), el("span", "", t(labelKey)));
  if (badgeKey) {
    btn.appendChild(el("span", "rail-badge", t(badgeKey)));
  }
  btn.addEventListener("click", () => navigate(route));
  return btn;
}

function renderRail(): HTMLElement {
  const rail = el("nav", "rail");
  rail.setAttribute("aria-label", t("app.name"));

  const brand = el("div", "rail-brand");
  const logo = el("div", "rail-logo");
  logo.appendChild(icon("grid", ""));
  const brandText = el("div");
  brandText.append(
    el("div", "rail-app-name", t("app.name")),
    el("div", "rail-tagline", t("app.tagline")),
  );
  brand.append(logo, brandText);

  const projBtn = el("button", "rail-project-btn");
  projBtn.type = "button";
  projBtn.setAttribute("aria-label", t("rail.switchProject"));
  const projText = el("span", "rail-project-text");
  projText.append(
    el("span", "rail-project-label", t("rail.projectLabel")),
    el("span", "rail-project-name", state.activeProject?.name ?? t("rail.noProject")),
  );
  projBtn.append(projText, icon("swap", "ic-sm"));
  projBtn.addEventListener("click", () => void pickAndOpenProject());

  const bottom = el("div", "rail-bottom");
  bottom.append(
    navButton("backup", "rail.backup", "backup"),
    navButton("settings", "rail.settings", "settings"),
  );

  const offline = el("div", "rail-offline");
  const offText = el("div");
  offText.append(
    el("span", "", t("status.offline")),
    el("span", "rail-offline-sub", t("status.offlineSub")),
  );
  offline.append(icon("offline", "ic-sm"), offText);
  bottom.appendChild(offline);

  rail.append(
    brand,
    projBtn,
    navButton("home", "rail.home", "home"),
    navButton("files", "rail.files", "folder"),
    el("div", "rail-section", t("rail.tools")),
    navButton("clean", "rail.clean", "clean"),
    navButton("annotate", "rail.annotate", "tag", "rail.notBuiltTag"),
    bottom,
  );
  return rail;
}

function renderNoticeBanner(): HTMLElement | null {
  if (!state.notice) return null;
  const bar = el("div", `notice-bar notice-${state.notice.tone}`);
  bar.setAttribute("role", "status");
  bar.append(
    el("span", "", state.notice.text),
    button(t("common.dismiss"), "ghost", () => clearNotice()),
  );
  return bar;
}

function renderCurrentScreen(): { header: HTMLElement; body: HTMLElement } {
  if (state.route !== "clean") {
    destroyActiveCleanGrid();
  }
  switch (state.route) {
    case "home":
      return renderHomeScreen();
    case "files":
      return renderFilesScreen();
    case "clean":
      return renderCleanScreen();
    case "annotate":
      return renderAnnotateScreen();
    case "backup":
      return renderBackupScreen();
    case "settings":
      return renderSettingsScreen();
  }
}

function buildPaletteCommands(): PaletteCommand[] {
  const cmds: PaletteCommand[] = [
    { id: "home", label: t("palette.goHome"), shortcut: "Ctrl+1", run: () => navigate("home") },
    { id: "files", label: t("palette.goFiles"), shortcut: "Ctrl+2", run: () => navigate("files") },
    { id: "clean", label: t("palette.goClean"), shortcut: "Ctrl+3", run: () => navigate("clean") },
    { id: "annotate", label: t("palette.goAnnotate"), shortcut: "Ctrl+4", run: () => navigate("annotate") },
    { id: "backup", label: t("palette.goBackup"), shortcut: "Ctrl+5", run: () => navigate("backup") },
    { id: "settings", label: t("palette.goSettings"), shortcut: "Ctrl+6", run: () => navigate("settings") },
    { id: "open", label: t("palette.openFolder"), run: () => void pickAndOpenProject() },
  ];
  if (state.cleanSession) {
    cmds.push(
      { id: "undo", label: t("palette.cleanUndo"), shortcut: "Ctrl+Z", run: () => void runCleanUndo() },
      { id: "redo", label: t("palette.cleanRedo"), shortcut: "Ctrl+Y", run: () => void runCleanRedo() },
      { id: "save", label: t("palette.cleanSave"), run: () => void runCleanSave() },
    );
  }
  return cmds;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

function installShortcuts(): void {
  window.addEventListener("keydown", (ev) => {
    const mod = ev.ctrlKey || ev.metaKey;
    if (!mod) return;
    const key = ev.key.toLowerCase();

    if (key === "k") {
      ev.preventDefault();
      if (isPaletteOpen()) closePalette();
      else openPalette(buildPaletteCommands());
      return;
    }

    if (key >= "1" && key <= "6") {
      ev.preventDefault();
      closePalette();
      navigate(ROUTE_ORDER[Number(key) - 1]);
      return;
    }

    if (state.route === "clean" && state.cleanSession && !isEditableTarget(ev.target)) {
      if (key === "z") {
        ev.preventDefault();
        void runCleanUndo();
      } else if (key === "y") {
        ev.preventDefault();
        void runCleanRedo();
      }
    }
  });
}

async function bootstrap(): Promise<void> {
  const root = document.getElementById("app") as HTMLElement;

  if (isTauriRuntime()) {
    try {
      const meta = await api.getMeta();
      if (meta.benchMode) {
        await runBenchMode(root, meta);
        return;
      }
    } catch {
      // Proceed to normal app shell when get_meta is unavailable or not in bench mode.
    }
  }

  try {
    const [catalogue, info, settings, recent] = await Promise.all([
      api.strings(),
      api.appInfo(),
      api.settingsGet(),
      api.projectsRecent(),
    ]);
    initStrings(catalogue);
    state.version = info.version;
    state.settings = settings;
    state.recentProjects = recent;
    document.title = t("app.title");

    const firstAvailable = recent.find((p) => p.available);
    if (firstAvailable) {
      await openProjectView(await api.projectOpen(firstAvailable.id));
    }
  } catch (err) {
    showError(err);
  }

  setRenderer(() => {
    clear(root);
    const shell = el("div", "app-shell");
    const main = el("main", "main-col");
    const { header, body } = renderCurrentScreen();
    main.appendChild(header);
    const notice = renderNoticeBanner();
    if (notice) main.appendChild(notice);
    main.appendChild(body);
    shell.append(renderRail(), main);
    root.appendChild(shell);
  });

  installShortcuts();
  renderApp();
}

void bootstrap();
