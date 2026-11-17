import { ref } from "vue";
import { api, type Saved } from "./api";
import { showError } from "./commands";
import { go } from "./nav";
import { announce } from "./store";

/** Titles saved to watch later, newest first. */
export const saved = ref<Saved[]>([]);

export async function loadWatchlist() {
  saved.value = await api.watchlist().catch(() => saved.value);
}

export function isSaved(t: Pick<Saved, "id" | "kind">) {
  return saved.value.some((s) => s.id === t.id && s.kind === t.kind);
}

const add = (t: Saved) => api.saveToWatchlist(t).then(loadWatchlist);
const remove = (t: Saved) => api.removeFromWatchlist(t.id, t.kind).then(loadWatchlist);

export async function save(t: Saved) {
  try {
    await add(t);
    announce(`Saved “${t.title}” to your watchlist`, () => remove(t));
  } catch (e) {
    showError(`“${t.title}” couldn't be saved.`, e);
  }
}

export async function unsave(t: Saved) {
  try {
    await remove(t);
    announce(`Removed “${t.title}” from your watchlist`, () => add(t));
  } catch (e) {
    showError(`“${t.title}” couldn't be removed.`, e);
  }
}

export function openSaved(t: Saved) {
  go({ name: "title", title: { ...t, backdrop: null, overview: "", rating: null } });
}

export function savedLine(t: Saved) {
  const kind = t.kind === "movie" ? "Film" : "Show";
  return t.year ? `${kind}, ${t.year}` : kind;
}
