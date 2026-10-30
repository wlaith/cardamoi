import { listen } from "@tauri-apps/api/event";
import { computed, ref, watch } from "vue";
import { api, plainError, type LibraryEntry, type PlayerState } from "./api";
import { groupLibrary, type LocalEpisode, type Show } from "./library";

export const entries = ref<LibraryEntry[]>([]);
export const loadError = ref("");
/** False until the first read, so an unread library isn't shown as empty. */
export const loaded = ref(false);
export const shelf = computed(() => groupLibrary(entries.value));

export async function refresh() {
  try {
    entries.value = await api.library();
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

export const player = ref<PlayerState | null>(null);
export const playing = computed(
  () => !!player.value && (player.value.status === "opening" || player.value.status === "playing"),
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

export const notice = ref<{ text: string; undo?: () => Promise<unknown> } | null>(null);
let noticeTimer: number | undefined;

export function announce(text: string, undo?: () => Promise<unknown>) {
  notice.value = { text, undo };
  window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => (notice.value = null), undo ? 10000 : 6000);
}

export async function undoLast() {
  const undo = notice.value?.undo;
  notice.value = null;
  if (undo) {
    await undo();
    await refresh();
  }
}

export type Selected = { kind: "video"; ep: LocalEpisode } | { kind: "show"; show: Show };
export const selection = ref<Selected | null>(null);

export type ViewMode = "grid" | "list";
function savedViewMode(): ViewMode {
  try {
    return localStorage.getItem("viewMode") === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
}
export const viewMode = ref<ViewMode>(savedViewMode());
watch(viewMode, (mode) => {
  try {
    localStorage.setItem("viewMode", mode);
  } catch {
    // Not remembering the view is fine.
  }
});

export function startSync(): () => void {
  refresh();
  const timer = window.setInterval(refresh, 1000);
  api.playerState().then((p) => (player.value = p), () => {});
  const unlisten = listen<PlayerState>("player://state", (event) => (player.value = event.payload));
  return () => {
    window.clearInterval(timer);
    unlisten.then((stop) => stop());
  };
}
