import { badge, button, el, icon } from "../dom.js";
import { t } from "../i18n.js";
import { navigate } from "../state.js";

export function renderAnnotateScreen(): { header: HTMLElement; body: HTMLElement } {
  const header = el("div", "topbar");
  const headText = el("div", "topbar-text");
  headText.append(
    el("h1", "topbar-title", t("annotate.title")),
    el("div", "topbar-sub", t("annotate.subtitle")),
  );
  header.appendChild(headText);

  const body = el("div", "page-body");
  const card = el("div", "card placeholder-card");
  const top = el("div", "row-gap");
  top.append(icon("tag", "ic-md"), badge(t("common.notBuiltYet"), "amber"));
  card.append(
    top,
    el("h2", "card-title mt-sm", t("annotate.heading")),
    el("p", "muted", t("annotate.body")),
  );
  const actions = el("div", "mt-md");
  actions.appendChild(button(t("annotate.goClean"), "primary", () => navigate("clean"), "clean"));
  card.appendChild(actions);
  body.appendChild(card);

  return { header, body };
}
