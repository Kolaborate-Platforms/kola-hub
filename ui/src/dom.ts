const SVG_NS = "http://www.w3.org/2000/svg";

export type IconName =
  | "grid"
  | "home"
  | "folder"
  | "clean"
  | "tag"
  | "backup"
  | "settings"
  | "offline"
  | "swap"
  | "undo"
  | "redo"
  | "save"
  | "trash"
  | "lock"
  | "refresh"
  | "arrow"
  | "check"
  | "restore";

interface SvgShape {
  tag: string;
  attrs: Record<string, string>;
}

const ICONS: Record<IconName, SvgShape[]> = {
  grid: [
    { tag: "rect", attrs: { x: "3", y: "3", width: "7", height: "7" } },
    { tag: "rect", attrs: { x: "14", y: "3", width: "7", height: "7" } },
    { tag: "rect", attrs: { x: "3", y: "14", width: "7", height: "7" } },
    { tag: "rect", attrs: { x: "14", y: "14", width: "7", height: "7" } },
  ],
  home: [
    { tag: "path", attrs: { d: "m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" } },
    { tag: "polyline", attrs: { points: "9 22 9 12 15 12 15 22" } },
  ],
  folder: [
    { tag: "path", attrs: { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" } },
  ],
  clean: [
    { tag: "polygon", attrs: { points: "22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" } },
  ],
  tag: [
    { tag: "path", attrs: { d: "M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" } },
    { tag: "line", attrs: { x1: "7", y1: "7", x2: "7.01", y2: "7" } },
  ],
  backup: [
    { tag: "polyline", attrs: { points: "21 8 21 21 3 21 3 8" } },
    { tag: "rect", attrs: { x: "1", y: "3", width: "22", height: "5" } },
    { tag: "line", attrs: { x1: "10", y1: "12", x2: "14", y2: "12" } },
  ],
  settings: [
    { tag: "line", attrs: { x1: "4", y1: "21", x2: "4", y2: "14" } },
    { tag: "line", attrs: { x1: "4", y1: "10", x2: "4", y2: "3" } },
    { tag: "line", attrs: { x1: "12", y1: "21", x2: "12", y2: "12" } },
    { tag: "line", attrs: { x1: "12", y1: "8", x2: "12", y2: "3" } },
    { tag: "line", attrs: { x1: "20", y1: "21", x2: "20", y2: "16" } },
    { tag: "line", attrs: { x1: "20", y1: "12", x2: "20", y2: "3" } },
    { tag: "line", attrs: { x1: "1", y1: "14", x2: "7", y2: "14" } },
    { tag: "line", attrs: { x1: "9", y1: "8", x2: "15", y2: "8" } },
    { tag: "line", attrs: { x1: "17", y1: "16", x2: "23", y2: "16" } },
  ],
  offline: [
    { tag: "line", attrs: { x1: "1", y1: "1", x2: "23", y2: "23" } },
    { tag: "path", attrs: { d: "M16.72 11.06A10.94 10.94 0 0 1 19 12.55" } },
    { tag: "path", attrs: { d: "M5 12.55a10.94 10.94 0 0 1 5.17-2.39" } },
    { tag: "path", attrs: { d: "M8.53 16.11a6 6 0 0 1 6.95 0" } },
    { tag: "line", attrs: { x1: "12", y1: "20", x2: "12.01", y2: "20" } },
  ],
  swap: [
    { tag: "polyline", attrs: { points: "7 9 12 4 17 9" } },
    { tag: "polyline", attrs: { points: "7 15 12 20 17 15" } },
  ],
  undo: [
    { tag: "polyline", attrs: { points: "1 4 1 10 7 10" } },
    { tag: "path", attrs: { d: "M3.51 15a9 9 0 1 0 2.13-9.36L1 10" } },
  ],
  redo: [
    { tag: "polyline", attrs: { points: "23 4 23 10 17 10" } },
    { tag: "path", attrs: { d: "M20.49 15a9 9 0 1 1-2.12-9.36L23 10" } },
  ],
  save: [
    { tag: "path", attrs: { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" } },
    { tag: "polyline", attrs: { points: "7 10 12 15 17 10" } },
    { tag: "line", attrs: { x1: "12", y1: "15", x2: "12", y2: "3" } },
  ],
  trash: [
    { tag: "polyline", attrs: { points: "3 6 5 6 21 6" } },
    { tag: "path", attrs: { d: "M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" } },
    { tag: "path", attrs: { d: "M10 11v6M14 11v6" } },
  ],
  lock: [
    { tag: "rect", attrs: { x: "3", y: "11", width: "18", height: "11", rx: "2" } },
    { tag: "path", attrs: { d: "M7 11V7a5 5 0 0 1 10 0v4" } },
  ],
  refresh: [
    { tag: "polyline", attrs: { points: "23 4 23 10 17 10" } },
    { tag: "path", attrs: { d: "M20.49 15a9 9 0 1 1-2.12-9.36L23 10" } },
  ],
  arrow: [
    { tag: "line", attrs: { x1: "5", y1: "12", x2: "19", y2: "12" } },
    { tag: "polyline", attrs: { points: "12 5 19 12 12 19" } },
  ],
  check: [
    { tag: "polyline", attrs: { points: "20 6 9 17 4 12" } },
  ],
  restore: [
    { tag: "path", attrs: { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" } },
    { tag: "polyline", attrs: { points: "17 8 12 3 7 8" } },
    { tag: "line", attrs: { x1: "12", y1: "3", x2: "12", y2: "15" } },
  ],
};

export function clear(node: HTMLElement): void {
  node.textContent = "";
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  if (text !== undefined) {
    node.textContent = text;
  }
  return node;
}

export function icon(name: IconName, className = "ic-sm"): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", className);
  for (const shape of ICONS[name]) {
    const child = document.createElementNS(SVG_NS, shape.tag);
    for (const [k, v] of Object.entries(shape.attrs)) {
      child.setAttribute(k, v);
    }
    svg.appendChild(child);
  }
  return svg;
}

export function button(
  label: string,
  variant: "primary" | "outline" | "ghost",
  onClick: () => void,
  iconName?: IconName,
): HTMLButtonElement {
  const cls =
    variant === "primary"
      ? "btn btn-primary"
      : variant === "outline"
        ? "btn btn-outline"
        : "btn btn-ghost";
  const btn = el("button", cls);
  btn.type = "button";
  if (iconName) {
    btn.appendChild(icon(iconName, "ic-sm"));
  }
  btn.appendChild(el("span", "", label));
  btn.addEventListener("click", onClick);
  return btn;
}

export function badge(
  text: string,
  tone: "green" | "amber" | "grey" | "blue" | "red" | "teal" = "grey",
): HTMLSpanElement {
  return el("span", `tag tag-${tone}`, text);
}

export function progressBar(percent: number): HTMLDivElement {
  const clamped = Math.max(0, Math.min(100, percent));
  const track = el("div", "bar");
  const fill = el("div", "bar-fill");
  fill.style.width = `${clamped}%`;
  track.appendChild(fill);
  return track;
}
