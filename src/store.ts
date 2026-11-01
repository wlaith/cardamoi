import { listen } from "@tauri-apps/api/event";
import { computed, ref, watch, type Ref } from "vue";
import { api, formatBytes, plainError, type LibraryEntry, type PlayerState } from "./api";
import { groupLibrary, type LocalEpisode, type Show } from "./library";

export const entries = ref<LibraryEntry[]>([]);
export const loadError = ref("");
/** False until the first read, so an unread library isn't shown as empty. */
export const loaded = ref(false);
export const shelf = computed(() => groupLibrary(entries.value));

const leaving = new Set<number>();

export async function refresh() {
  try {
    const all = await api.library();
    entries.value = leaving.size ? all.filter((e) => !leaving.has(e.id)) : all;
    loadError.value = "";
  } catch (e) {
    loadError.value = plainError(e).message;
  }
  loaded.value = true;
}

export const activeDownloads = computed(() =>
  entries.value.filter((e) => ["resolving", "starting", "downloading"].includes(e.status)),
);

export const unfinished = computed(() => entries.value.filter((e) => e.status !== "done"));

export const librarySummary = computed(() => {
  if (!entries.value.length) return "";
  const films = shelf.value.filter((s) => s.kind === "film").length;
  const shows = shelf.value.length - films;
  const size = formatBytes(entries.value.reduce((sum, e) => sum + e.totalBytes, 0));
  return `${shows} ${shows === 1 ? "show" : "shows"} · ${films} ${films === 1 ? "film" : "films"} · ${size}`;
});

export const player = ref<PlayerState | null>(null);
export const playing = computed(
  () => !!player.value && (player.value.status === "opening" || player.value.status === "playing"),
);

/** The built-in player is up, drawing its own controls over the picture. */
export const embedded = computed(
  () => player.value?.mode === "embedded" && ["opening", "playing", "ended"].includes(player.value.status),
);

export const opening = ref<{ itemId: number; fileIndex: number; label: string } | null>(null);
let openingTimer: number | undefined;

export function startOpening(itemId: number, fileIndex: number, label: string) {
  opening.value = { itemId, fileIndex, label };
  window.clearTimeout(openingTimer);
  openingTimer = window.setTimeout(() => (opening.value = null), 15000);
}

export function stopOpening() {
  opening.value = null;
  window.clearTimeout(openingTimer);
}

watch(player, (p) => {
  if (p && opening.value && p.itemId === opening.value.itemId && p.status !== "opening") stopOpening();
});

export function hideEntries(ids: number[]) {
  ids.forEach((id) => leaving.add(id));
  entries.value = entries.value.filter((e) => !leaving.has(e.id));
}

export function showEntries(ids: number[]) {
  ids.forEach((id) => leaving.delete(id));
}

export interface Notice {
  id: number;
  text: string;
  undo?: () => Promise<unknown>;
  commit?: () => Promise<unknown>;
  duration: number;
}

export const notice = ref<Notice | null>(null);
let noticeId = 0;
let noticeTimer: number | undefined;
let noticeLeft = 0;
let noticeSince = 0;

function runNoticeTimer(ms: number) {
  window.clearTimeout(noticeTimer);
  noticeLeft = ms;
  noticeSince = Date.now();
  noticeTimer = window.setTimeout(dismissNotice, ms);
}

export function announce(text: string, undo?: () => Promise<unknown>, commit?: () => Promise<unknown>) {
  dismissNotice();
  const duration = undo ? 10000 : 6000;
  notice.value = { id: ++noticeId, text, undo, commit, duration };
  runNoticeTimer(duration);
}

export function dismissNotice() {
  const open = notice.value;
  notice.value = null;
  window.clearTimeout(noticeTimer);
  open?.commit?.().finally(refresh);
}

export function holdNotice() {
  if (!notice.value) return;
  window.clearTimeout(noticeTimer);
  noticeLeft -= Date.now() - noticeSince;
}

export function releaseNotice() {
  if (notice.value) runNoticeTimer(Math.max(noticeLeft, 1500));
}

export async function undoLast() {
  const open = notice.value;
  notice.value = null;
  window.clearTimeout(noticeTimer);
  if (open?.undo) {
    await open.undo();
    await refresh();
  }
}

export type Selected = { kind: "video"; ep: LocalEpisode } | { kind: "show"; show: Show };
export const selection = ref<Selected | null>(null);

function remembered<T>(key: string, fallback: T, valid: (value: unknown) => value is T) {
  let initial = fallback;
  try {
    const raw = localStorage.getItem(key);
    let stored: unknown = raw;
    try {
      stored = JSON.parse(raw ?? "null");
    } catch {
    }
    if (valid(stored)) initial = stored;
  } catch {
  }
  const value = ref(initial) as Ref<T>;
  watch(value, (v) => {
    try {
      localStorage.setItem(key, JSON.stringify(v));
    } catch {
    }
  });
  return value;
}

const oneOf =
  <T extends string>(...values: T[]) =>
  (v: unknown): v is T =>
    values.includes(v as T);

export type ViewMode = "grid" | "list";
export const viewMode = remembered<ViewMode>("viewMode", "grid", oneOf("grid", "list"));

export type HomeMode = "stack" | "grid";
export const homeMode = remembered<HomeMode>("homeMode", "stack", oneOf("stack", "grid"));

export type Theme = "gray" | "ink" | "paper";
export const theme = remembered<Theme>("theme", "gray", oneOf("gray", "ink", "paper"));
watch(theme, (t) => (document.documentElement.dataset.theme = t), { immediate: true });

export const COLUMNS = { min: 5, max: 9 };
export const columns = remembered<number>(
  "columns",
  7,
  (v): v is number => Number.isInteger(v) && (v as number) >= COLUMNS.min && (v as number) <= COLUMNS.max,
);

export function startSync(): () => void {
  refresh();
  const timer = window.setInterval(refresh, 1000);
  api.playerState().then((p) => (player.value = p), () => {});
  const unlisten = listen<PlayerState>("player://state", (event) => (player.value = event.payload));
  window.addEventListener("beforeunload", dismissNotice);
  return () => {
    window.removeEventListener("beforeunload", dismissNotice);
    window.clearInterval(timer);
    unlisten.then((stop) => stop());
  };
}
