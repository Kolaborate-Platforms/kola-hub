import { clear, el } from "./dom.js";
import { t } from "./i18n.js";

export interface PaletteCommand {
  id: string;
  label: string;
  shortcut?: string;
  run: () => void;
}

let overlayEl: HTMLElement | null = null;
let previousFocus: HTMLElement | null = null;

export function isPaletteOpen(): boolean {
  return overlayEl !== null;
}

export function closePalette(): void {
  if (!overlayEl) return;
  overlayEl.remove();
  overlayEl = null;
  previousFocus?.focus();
  previousFocus = null;
}

export function openPalette(commands: PaletteCommand[]): void {
  if (overlayEl) {
    closePalette();
    return;
  }
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const overlay = el("div", "palette-overlay");
  overlay.setAttribute("role", "dialog");
  overlay.setAttribute("aria-modal", "true");
  overlay.setAttribute("aria-label", t("palette.title"));

  const dialog = el("div", "palette-box");
  const input = el("input", "inp palette-input");
  input.type = "text";
  input.placeholder = t("palette.placeholder");
  input.setAttribute("aria-label", t("palette.placeholder"));

  const list = el("div", "palette-list");
  list.setAttribute("role", "listbox");

  let filtered = [...commands];
  let selected = 0;

  const renderList = (): void => {
    clear(list);
    if (filtered.length === 0) {
      list.appendChild(el("div", "palette-empty", t("palette.empty")));
      return;
    }
    filtered.forEach((cmd, idx) => {
      const item = el("button", idx === selected ? "palette-item active" : "palette-item");
      item.type = "button";
      item.setAttribute("role", "option");
      item.setAttribute("aria-selected", String(idx === selected));
      item.appendChild(el("span", "", cmd.label));
      if (cmd.shortcut) {
        item.appendChild(el("span", "key", cmd.shortcut));
      }
      item.addEventListener("click", () => {
        closePalette();
        cmd.run();
      });
      list.appendChild(item);
    });
  };

  input.addEventListener("input", () => {
    const q = input.value.trim().toLowerCase();
    filtered = q ? commands.filter((c) => c.label.toLowerCase().includes(q)) : [...commands];
    selected = 0;
    renderList();
  });

  dialog.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      closePalette();
    } else if (ev.key === "ArrowDown") {
      ev.preventDefault();
      if (filtered.length > 0) {
        selected = (selected + 1) % filtered.length;
        renderList();
      }
    } else if (ev.key === "ArrowUp") {
      ev.preventDefault();
      if (filtered.length > 0) {
        selected = (selected - 1 + filtered.length) % filtered.length;
        renderList();
      }
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      const target = filtered[selected];
      if (target) {
        closePalette();
        target.run();
      }
    }
  });

  overlay.addEventListener("click", (ev) => {
    if (ev.target === overlay) closePalette();
  });

  dialog.append(input, list);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);
  overlayEl = overlay;
  renderList();
  input.focus();
}
