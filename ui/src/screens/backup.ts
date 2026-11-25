import { api, onBackupProgress } from "../api.js";
import { badge, button, el, progressBar } from "../dom.js";
import { formatBytes, formatRelativeTime, t } from "../i18n.js";
import { refreshProjectData, renderApp, setNotice, showError, state } from "../state.js";

let confirmingEmpty = false;
let backupStatusText = "";
let testStatusText = "";
let restoreStatusText = "";

const SKIPPED_NAMES_SHOWN = 10;

function renderTrashCard(): HTMLElement {
  const card = el("div", "card");
  const top = el("div", "row-between");
  top.append(
    el("h2", "card-title", t("backup.trashHeading")),
    badge(t("items.count", undefined, state.trash.length), "grey"),
  );
  card.appendChild(top);

  if (!state.activeProject) {
    card.appendChild(el("p", "muted", t("backup.trashNoProject")));
    return card;
  }

  const daysText = t("days.count", undefined, state.settings.trashRetentionDays);
  card.appendChild(el("p", "muted", t("backup.trashNote", { days: daysText })));

  if (state.trash.length === 0) {
    card.appendChild(el("p", "muted mt-sm", t("backup.trashEmptyState")));
    return card;
  }

  for (const item of state.trash) {
    const row = el("div", "lv row-between");
    const info = el("div", "grow");
    const titleLine = el("div", "row-gap-sm");
    titleLine.appendChild(el("strong", "", item.name));
    if (item.recovered) {
      titleLine.appendChild(badge(t("backup.trashRecoveredTag"), "amber"));
    }
    const meta = el(
      "div",
      "muted",
      t("backup.trashItemMeta", { size: formatBytes(item.sizeBytes), when: formatRelativeTime(item.deletedAt) }),
    );
    info.append(titleLine, meta);

    const restoreBtn = button(t("backup.restoreOne"), "outline", async () => {
      if (!state.activeProject) return;
      try {
        await api.trashRestore(state.activeProject.id, item.id);
        await refreshProjectData();
        setNotice("green", t("backup.restoredOne", { name: item.name }));
      } catch (err) {
        showError(err);
      }
    });
    row.append(info, restoreBtn);
    card.appendChild(row);
  }

  const actions = el("div", "btn-row mt-md");
  actions.appendChild(
    button(t("backup.restoreAll"), "outline", async () => {
      if (!state.activeProject) return;
      try {
        for (const item of [...state.trash]) {
          await api.trashRestore(state.activeProject.id, item.id);
        }
        await refreshProjectData();
        setNotice("green", t("backup.restoredAll"));
      } catch (err) {
        showError(err);
      }
    }),
  );

  if (confirmingEmpty) {
    actions.appendChild(
      button(t("backup.emptyConfirm", { count: state.trash.length }), "primary", async () => {
        if (!state.activeProject) return;
        confirmingEmpty = false;
        try {
          const removed = await api.trashEmpty(state.activeProject.id);
          await refreshProjectData();
          setNotice("green", t("backup.emptied", undefined, removed));
        } catch (err) {
          showError(err);
        }
      }, "trash"),
    );
    actions.appendChild(
      button(t("common.cancel"), "ghost", () => {
        confirmingEmpty = false;
        renderApp();
      }),
    );
  } else {
    actions.appendChild(
      button(t("backup.emptyNow"), "outline", () => {
        confirmingEmpty = true;
        renderApp();
      }, "trash"),
    );
  }

  card.appendChild(actions);
  return card;
}

function renderDiskCard(): HTMLElement {
  const card = el("div", "card");
  card.appendChild(el("h2", "card-title", t("backup.diskHeading")));
  if (state.activeProject) {
    const { usedBytes, limitBytes } = state.activeProject.budget;
    const pct = limitBytes > 0 ? Math.round((usedBytes / limitBytes) * 100) : 0;
    card.appendChild(
      el("p", "muted", t("backup.diskSummary", { used: formatBytes(usedBytes), limit: formatBytes(limitBytes) })),
    );
    card.appendChild(progressBar(pct));
  } else {
    card.appendChild(el("p", "muted", t("home.diskNoProject")));
  }
  card.appendChild(el("div", "note note-amber mt-sm", t("backup.diskWarnNote")));
  return card;
}

function renderCreateBackupCard(): HTMLElement {
  const card = el("div", "card");
  card.append(
    el("h2", "card-title", t("backup.createHeading")),
    el("p", "muted", t("backup.createDesc")),
  );

  const selectedIds = new Set<string>(
    state.recentProjects.filter((p) => p.available).map((p) => p.id),
  );

  const listWrap = el("div", "stack-xs mt-sm");
  listWrap.appendChild(el("div", "field-label", t("backup.selectProjects")));
  if (state.recentProjects.length === 0) {
    listWrap.appendChild(el("div", "muted", t("backup.noProjectsToBackup")));
  } else {
    for (const proj of state.recentProjects) {
      const label = el("label", "check-row");
      const box = el("input");
      box.type = "checkbox";
      box.checked = proj.available;
      box.disabled = !proj.available;
      box.addEventListener("change", () => {
        if (box.checked) selectedIds.add(proj.id);
        else selectedIds.delete(proj.id);
      });
      label.append(box, el("span", "", proj.name));
      listWrap.appendChild(label);
    }
  }
  card.appendChild(listWrap);

  const pw1 = el("input", "inp");
  pw1.type = "password";
  pw1.id = "backup-pw-1";
  const pw2 = el("input", "inp");
  pw2.type = "password";
  pw2.id = "backup-pw-2";

  const l1 = el("label", "field-label mt-sm", t("backup.passphraseLabel"));
  l1.htmlFor = pw1.id;
  const l2 = el("label", "field-label mt-sm", t("backup.passphraseAgainLabel"));
  l2.htmlFor = pw2.id;

  const statusLine = el("div", "muted mt-sm", backupStatusText);

  const createBtn = button(t("backup.createButton"), "primary", async () => {
    const ids = [...selectedIds];
    if (ids.length === 0) {
      setNotice("amber", t("backup.selectAtLeastOne"));
      return;
    }
    if (!pw1.value) {
      setNotice("amber", t("backup.passphraseRequired"));
      return;
    }
    if (pw1.value !== pw2.value) {
      setNotice("amber", t("backup.passphraseMismatch"));
      return;
    }
    let unsub: (() => void) | null = null;
    try {
      unsub = await onBackupProgress((p) => {
        statusLine.textContent = t("backup.creating", { files: p.files, size: formatBytes(p.bytes) });
      });
      const report = await api.backupCreate(ids, pw1.value);
      if (!report) return;
      let msg = t("backup.createDone", { files: report.files, size: formatBytes(report.bytes) });
      if (report.skipped.length > 0) {
        msg = `${msg} ${t("backup.createSkipped", { count: report.skipped.length })}`;
        const names = report.skipped.slice(0, SKIPPED_NAMES_SHOWN).join(", ");
        msg = `${msg} ${t("backup.createSkippedNames", { names })}`;
      }
      backupStatusText = msg;
      setNotice("green", msg);
    } catch (err) {
      showError(err);
    } finally {
      unsub?.();
    }
  }, "lock");

  card.append(l1, pw1, l2, pw2, el("div", "note note-amber mt-sm", t("backup.passphraseWarn")));
  const btnRow = el("div", "mt-md");
  btnRow.appendChild(createBtn);
  card.append(btnRow, statusLine);
  return card;
}

function renderTestAndRestoreCard(): HTMLElement {
  const wrap = el("div", "stack-md");

  const testCard = el("div", "card");
  testCard.append(
    el("h2", "card-title", t("backup.testHeading")),
    el("p", "muted", t("backup.testDesc")),
  );
  const testPw = el("input", "inp");
  testPw.type = "password";
  testPw.id = "backup-test-pw";
  const testLabel = el("label", "field-label mt-sm", t("backup.testPassphraseLabel"));
  testLabel.htmlFor = testPw.id;
  const testBtn = button(t("backup.testButton"), "outline", async () => {
    if (!testPw.value) {
      setNotice("amber", t("backup.passphraseRequired"));
      return;
    }
    try {
      const rep = await api.backupTest(testPw.value);
      if (!rep) return;
      testStatusText = t("backup.testDone", {
        files: rep.files,
        size: formatBytes(rep.bytes),
        sampled: rep.sampled,
      });
      setNotice("green", testStatusText);
    } catch (err) {
      showError(err);
    }
  }, "refresh");
  testCard.append(testLabel, testPw, el("div", "mt-sm"));
  testCard.lastElementChild?.appendChild(testBtn);
  if (testStatusText) {
    testCard.appendChild(el("div", "note note-green mt-sm", testStatusText));
  }

  const restoreCard = el("div", "card");
  restoreCard.append(
    el("h2", "card-title", t("backup.restoreHeading")),
    el("p", "muted", t("backup.restoreDesc")),
  );
  const resPw = el("input", "inp");
  resPw.type = "password";
  resPw.id = "backup-restore-pw";
  const resLabel = el("label", "field-label mt-sm", t("backup.restorePassphraseLabel"));
  resLabel.htmlFor = resPw.id;
  const resBtn = button(t("backup.restoreButton"), "outline", async () => {
    if (!resPw.value) {
      setNotice("amber", t("backup.passphraseRequired"));
      return;
    }
    try {
      const rep = await api.backupRestore(resPw.value);
      if (!rep) return;
      restoreStatusText = t("backup.restoreDone", {
        files: rep.files,
        size: formatBytes(rep.bytes),
      });
      setNotice("green", restoreStatusText);
    } catch (err) {
      showError(err);
    }
  }, "restore");
  restoreCard.append(resLabel, resPw, el("div", "mt-sm"));
  restoreCard.lastElementChild?.appendChild(resBtn);
  if (restoreStatusText) {
    restoreCard.appendChild(el("div", "note note-green mt-sm", restoreStatusText));
  }

  wrap.append(testCard, restoreCard);
  return wrap;
}

export function renderBackupScreen(): { header: HTMLElement; body: HTMLElement } {
  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("backup.title")),
    el("div", "topbar-sub", t("backup.subtitle")),
  );
  header.appendChild(headText);

  const body = el("div", "page-body");
  const cols = el("div", "grid-2");
  const left = el("div", "stack-md");
  const right = el("div", "stack-md");

  left.append(renderTrashCard(), renderDiskCard());
  right.append(renderCreateBackupCard(), renderTestAndRestoreCard());
  cols.append(left, right);
  body.appendChild(cols);

  return { header, body };
}
