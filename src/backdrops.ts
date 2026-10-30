import { reactive } from "vue";
import { api, type Kind } from "./api";

const found = reactive(new Map<string, string | null>());

export function backdropOf(kind: Kind | null, tmdbId: number | null): string | null {
  if (tmdbId == null) return null;
  const key = `${kind ?? "movie"}:${tmdbId}`;
  if (!found.has(key)) {
    found.set(key, null);
    api
      .titleDetails(kind ?? "movie", tmdbId)
      .then((d) => found.set(key, d.backdrop))
      .catch(() => {});
  }
  return found.get(key) ?? null;
}
