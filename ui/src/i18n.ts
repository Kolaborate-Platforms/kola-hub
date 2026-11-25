import type { StringCatalogue } from "./types.js";

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;
const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_DAY = 86_400;

let catalogue: StringCatalogue = {};

export function initStrings(next: StringCatalogue): void {
  catalogue = next;
}

/**
 * Matches `kh_settings::translate`: selects English one/other plural form and
 * replaces `{placeholder}` in a single pass so values with braces never re-expand.
 */
export function t(
  key: string,
  values?: Record<string, string | number>,
  count?: number,
): string {
  const entry = catalogue[key];
  if (entry === undefined) {
    return key;
  }
  let text: string;
  if (typeof entry === "string") {
    text = entry;
  } else {
    const form = count === 1 ? entry.one : entry.other;
    if (form === undefined) {
      return key;
    }
    text = form;
  }
  return fill(text, values, count);
}

function fill(
  text: string,
  values?: Record<string, string | number>,
  count?: number,
): string {
  let out = "";
  let pos = 0;
  while (pos < text.length) {
    const open = text.indexOf("{", pos);
    if (open === -1) {
      out += text.slice(pos);
      break;
    }
    out += text.slice(pos, open);
    const close = text.indexOf("}", open + 1);
    if (close === -1) {
      out += "{";
      pos = open + 1;
      continue;
    }
    const name = text.slice(open + 1, close);
    if (values && Object.prototype.hasOwnProperty.call(values, name)) {
      out += String(values[name]);
    } else if (name === "count" && count !== undefined) {
      out += String(count);
    } else {
      out += `{${name}}`;
    }
    pos = close + 1;
  }
  return out;
}

export function formatBytes(bytes: number): string {
  if (bytes >= GB) {
    const val = bytes / GB;
    return `${val >= 10 ? Math.round(val) : val.toFixed(1)} GB`;
  }
  if (bytes >= MB) {
    const val = bytes / MB;
    return `${val >= 10 ? Math.round(val) : val.toFixed(1)} MB`;
  }
  if (bytes >= KB) {
    return `${Math.round(bytes / KB)} KB`;
  }
  return `${bytes} B`;
}

export function formatNumber(value: number): string {
  return value.toLocaleString("en-US");
}

export function formatRelativeTime(unixSeconds: number): string {
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (diff < SECONDS_PER_HOUR) {
    return t("time.today");
  }
  if (diff < SECONDS_PER_DAY) {
    const hours = Math.floor(diff / SECONDS_PER_HOUR);
    return t("time.hoursAgo", undefined, hours);
  }
  const days = Math.floor(diff / SECONDS_PER_DAY);
  if (days === 1) {
    return t("time.yesterday");
  }
  if (days < 14) {
    return t("time.daysAgo", undefined, days);
  }
  const d = new Date(unixSeconds * 1000);
  return d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });
}
