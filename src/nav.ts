import { computed, ref } from "vue";
import type { CatalogTitle } from "./api";

export type PreferencesPane = "general" | "subtitles" | "sources";

export type Route =
  | { name: "upnext" }
  | { name: "films" }
  | { name: "shows" }
  | { name: "downloads" }
  | { name: "search"; query: string }
  | { name: "title"; title: CatalogTitle }
  | { name: "localShow"; key: string }
  | { name: "preferences"; pane: PreferencesPane }
  | { name: "help"; topic?: string };

export type Section = "upnext" | "films" | "shows" | "downloads" | "search" | "preferences" | "help";

interface Visit {
  route: Route;
  section: Section;
}

const history = ref<Visit[]>([{ route: { name: "upnext" }, section: "upnext" }]);
const index = ref(0);

export const current = computed(() => history.value[index.value]);
export const route = computed(() => current.value.route);
export const section = computed(() => current.value.section);
export const canGoBack = computed(() => index.value > 0);
export const canGoForward = computed(() => index.value < history.value.length - 1);

function sectionOf(r: Route): Section | null {
  return r.name === "title" || r.name === "localShow" ? null : r.name;
}

function same(a: Route, b: Route) {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function go(r: Route) {
  if (same(r, route.value)) return;
  const visit = { route: r, section: sectionOf(r) ?? section.value };
  history.value = [...history.value.slice(0, index.value + 1), visit];
  index.value++;
}

export function replace(r: Route) {
  history.value[index.value] = { route: r, section: sectionOf(r) ?? section.value };
}

export function back() {
  if (canGoBack.value) index.value--;
}

export function forward() {
  if (canGoForward.value) index.value++;
}

export const lastQuery = computed(() => {
  for (let i = index.value; i >= 0; i--) {
    const r = history.value[i].route;
    if (r.name === "search") return r.query;
  }
  return "";
});
