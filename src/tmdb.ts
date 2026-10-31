import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, ref } from "vue";
import { api, type TmdbStatus } from "./api";

export const TMDB_SIGNUP = "https://www.themoviedb.org/signup";
export const TMDB_API_SETTINGS = "https://www.themoviedb.org/settings/api";

export const tmdbStatus = ref<TmdbStatus | null>(null);
export const tmdbReady = computed(() => tmdbStatus.value?.state === "ok");
export const tmdbBlocked = computed(() => !!tmdbStatus.value && tmdbStatus.value.state !== "ok");

export const bannerDismissed = ref(false);

export const matching = ref<{ done: number; total: number } | null>(null);

export async function loadTmdbStatus() {
  tmdbStatus.value = await api.tmdbStatus().catch(() => tmdbStatus.value);
}

listen("tmdb-status", loadTmdbStatus);
listen<{ done: number; total: number }>("tmdb-matching", (event) => {
  matching.value = event.payload.done >= event.payload.total ? null : event.payload;
});

export function noteTmdbError(e: unknown) {
  if (/tmdb:(missing|rejected)/.test(String(e))) loadTmdbStatus();
}

export function isTmdbKeyError(e: unknown) {
  return /tmdb:(missing|rejected)/.test(String(e));
}

export function keyFormat(text: string): "token" | "key" | "unknown" | "" {
  const key = text.trim().replace(/^["'`]+|["'`]+$/g, "").trim();
  if (!key) return "";
  if (/^eyJ[\w-]*\.[\w-]+\.[\w-]+$/.test(key)) return "token";
  if (/^[0-9a-f]{32}$/i.test(key)) return "key";
  return "unknown";
}

export function openTmdb(url: string) {
  openUrl(url).catch(() => {});
}
