import { api } from "../api.js";
import { button, el, icon, progressBar } from "../dom.js";
import { formatBytes, t } from "../i18n.js";
import { setNotice, showError, state } from "../state.js";

const GB = 1024 * 1024 * 1024;
const LIMIT_OPTIONS_GB = [1, 2, 5, 10, 20];
const RETENTION_OPTIONS_DAYS = [7, 14, 30, 60, 90];

function settingRow(label: string, desc: string, control: HTMLElement): HTMLElement {
  const row = el("div", "setting-row");
  const info = el("div", "setting-info");
  info.append(el("div", "setting-label", label), el("div", "muted", desc));
  const ctrlWrap = el("div", "setting-ctrl");
  ctrlWrap.appendChild(control);
  row.append(info, ctrlWrap);
  return row;
}

function renderShortcutsCard(): HTMLElement {
  const card = el("div", "card");
  card.appendChild(el("h2", "card-title", t("settings.shortcutsHeading")));

  const rows: [string, string[]][] = [
    [t("settings.shortcutPalette"), ["Ctrl", "K"]],
    [t("settings.shortcutUndoRedo"), ["Ctrl", "Z", "Ctrl", "Y"]],
    [t("settings.shortcutTools"), ["Ctrl", "1", "Ctrl", "6"]],
  ];

  for (const [label, keys] of rows) {
    const row = el("div", "lv row-between");
    row.appendChild(el("span", "", label));
    const keyWrap = el("span", "key-group");
    for (const k of keys) {
      keyWrap.appendChild(el("span", "key", k));
    }
    row.appendChild(keyWrap);
    card.appendChild(row);
  }
  return card;
}

export function renderSettingsScreen(): { header: HTMLElement; body: HTMLElement } {
  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("settings.title")),
    el("div", "topbar-sub", t("settings.subtitle")),
  );
  header.appendChild(headText);

  const body = el("div", "page-body");
  const cols = el("div", "grid-2");

  const leftCol = el("div", "stack-md");
  const rightCol = el("div", "stack-md");

  // General card
  const generalCard = el("div", "card");
  generalCard.appendChild(el("h2", "card-title", t("settings.generalHeading")));

  const langSelect = el("select", "inp inp-sm");
  langSelect.setAttribute("aria-label", t("settings.languageLabel"));
  const optEn = el("option", "", t("settings.languageEn"));
  optEn.value = "en";
  const optLg = el("option", "", t("settings.languageLg"));
  optLg.value = "lg";
  optLg.disabled = true;
  langSelect.append(optEn, optLg);
  langSelect.value = state.settings.language || "en";

  const labellerInput = el("input", "inp inp-md");
  labellerInput.type = "text";
  labellerInput.setAttribute("aria-label", t("settings.labellerLabel"));
  labellerInput.placeholder = t("settings.labellerPlaceholder");
  labellerInput.value = state.settings.labellerName ?? "";

  generalCard.append(
    settingRow(t("settings.languageLabel"), t("settings.languageDesc"), langSelect),
    settingRow(t("settings.labellerLabel"), t("settings.labellerDesc"), labellerInput),
  );
  leftCol.appendChild(generalCard);

  // Storage card
  const storageCard = el("div", "card");
  storageCard.appendChild(el("h2", "card-title", t("settings.storageHeading")));

  const limitSelect = el("select", "inp inp-sm");
  limitSelect.setAttribute("aria-label", t("settings.limitLabel"));
  const currentGb = Math.max(1, Math.round(state.settings.storageLimit / GB));
  const gbChoices = LIMIT_OPTIONS_GB.includes(currentGb)
    ? LIMIT_OPTIONS_GB
    : [...LIMIT_OPTIONS_GB, currentGb].sort((a, b) => a - b);
  for (const gb of gbChoices) {
    const opt = el("option", "", t("settings.limitOption", { gb }));
    opt.value = String(gb * GB);
    limitSelect.appendChild(opt);
  }
  limitSelect.value = String(currentGb * GB);

  const retentionSelect = el("select", "inp inp-sm");
  retentionSelect.setAttribute("aria-label", t("settings.retentionLabel"));
  const daysChoices = RETENTION_OPTIONS_DAYS.includes(state.settings.trashRetentionDays)
    ? RETENTION_OPTIONS_DAYS
    : [...RETENTION_OPTIONS_DAYS, state.settings.trashRetentionDays].sort((a, b) => a - b);
  for (const days of daysChoices) {
    const opt = el("option", "", t("days.count", undefined, days));
    opt.value = String(days);
    retentionSelect.appendChild(opt);
  }
  retentionSelect.value = String(state.settings.trashRetentionDays);

  storageCard.append(
    settingRow(t("settings.limitLabel"), t("settings.limitDesc"), limitSelect),
    settingRow(t("settings.retentionLabel"), t("settings.retentionDesc"), retentionSelect),
  );

  if (state.activeProject) {
    const { usedBytes, limitBytes } = state.activeProject.budget;
    const pct = limitBytes > 0 ? Math.round((usedBytes / limitBytes) * 100) : 0;
    const usage = el("div", "mt-sm");
    usage.append(
      el("div", "muted", t("home.diskUsage", { used: formatBytes(usedBytes), limit: formatBytes(limitBytes) })),
      progressBar(pct),
    );
    storageCard.appendChild(usage);
  }

  const saveRow = el("div", "mt-md");
  saveRow.appendChild(
    button(t("settings.saveButton"), "primary", async () => {
      const labeller = labellerInput.value.trim();
      const next = {
        language: langSelect.value,
        checkForUpdates: state.settings.checkForUpdates,
        aiAccess: state.settings.aiAccess,
        storageLimit: Number(limitSelect.value),
        trashRetentionDays: Number(retentionSelect.value),
        labellerName: labeller.length > 0 ? labeller : null,
      };
      try {
        state.settings = await api.settingsSave(next);
        setNotice("green", t("settings.savedNotice"));
      } catch (err) {
        showError(err);
      }
    }, "check"),
  );
  storageCard.appendChild(saveRow);
  leftCol.appendChild(storageCard);

  // Right column: privacy, shortcuts, about
  const privacyCard = el("div", "card card-soft-green");
  const privHead = el("div", "row-gap");
  privHead.append(icon("lock", "ic-md ic-green"), el("h2", "card-title text-green-dark", t("settings.privacyHeading")));
  privacyCard.append(privHead, el("p", "text-green-dark body-sm", t("settings.privacyBody")));

  const aboutCard = el("div", "card");
  aboutCard.append(
    el("h2", "card-title", t("settings.aboutHeading")),
    el("p", "muted", t("settings.aboutBody", { version: state.version })),
  );

  rightCol.append(privacyCard, renderShortcutsCard(), aboutCard);
  cols.append(leftCol, rightCol);
  body.appendChild(cols);

  return { header, body };
}
