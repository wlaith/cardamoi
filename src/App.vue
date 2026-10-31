<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { api } from "./api";
import {
  checkFolder,
  getInfo,
  moveToTrash,
  openExternal,
  openFolder,
  play,
  reveal,
  setWatched,
  stop,
  subtitles,
  togglePause,
  videoOf,
} from "./commands";
import { isTyping } from "./keys";
import { back, forward, go, route } from "./nav";
import { loadError, loaded, opening, playing, selection, startSync, undoLast, viewMode } from "./store";
import { contextMenu, hud, palette, sheet } from "./ui";
import CaptionsPanel from "./components/CaptionsPanel.vue";
import AppToolbar from "./components/shell/AppToolbar.vue";
import CommandPalette from "./components/shell/CommandPalette.vue";
import ContextMenu from "./components/shell/ContextMenu.vue";
import NowPlayingHud from "./components/shell/NowPlayingHud.vue";
import SheetHost from "./components/shell/SheetHost.vue";
import Toast from "./components/shell/Toast.vue";
import ErrorNote from "./components/ui/ErrorNote.vue";
import Loader from "./components/ui/Loader.vue";
import OpeningPanel from "./components/shell/OpeningPanel.vue";
import DownloadsView from "./components/views/DownloadsView.vue";
import HelpView from "./components/views/HelpView.vue";
import LibraryView from "./components/views/LibraryView.vue";
import LocalShowView from "./components/views/LocalShowView.vue";
import PreferencesView from "./components/views/PreferencesView.vue";
import SearchView from "./components/views/SearchView.vue";
import TitleView from "./components/views/TitleView.vue";
import UpNextView from "./components/views/UpNextView.vue";

const main = ref<HTMLElement>();
const selected = computed(() => videoOf(selection.value));

watch(route, () => {
  selection.value = null;
  main.value?.scrollTo({ top: 0 });
});

function playOrPause(fromStart = false) {
  if (playing.value && !fromStart) return togglePause();
  const ep = selected.value;
  if (ep?.video.playable) play(ep, fromStart || ep.video.watched);
}

/** Menu ids, see build_menu in lib.rs. */
function onMenu(id: string) {
  const ep = selected.value;
  switch (id) {
    case "preferences":
      return go({ name: "preferences", pane: "general" });
    case "help":
      return go({ name: "help" });
    case "add-magnet":
      sheet.value = { kind: "magnet", link: "" };
      return;
    case "find":
      palette.value = true;
      return;
    case "open-folder":
      return openFolder();
    case "check-folder":
      return checkFolder();
    case "view-grid":
    case "view-list":
      viewMode.value = id === "view-grid" ? "grid" : "list";
      return;
    case "go-upnext":
      return go({ name: "upnext" });
    case "go-films":
      return go({ name: "films" });
    case "go-shows":
      return go({ name: "shows" });
    case "go-downloads":
      return go({ name: "downloads" });
    case "back":
      return back();
    case "forward":
      return forward();
    case "play":
      return playOrPause();
    case "play-start":
      return playOrPause(true);
    case "stop":
      return stop();
  }
  if (!ep) return;
  switch (id) {
    case "get-info":
      return getInfo(ep);
    case "reveal":
      return reveal(ep.entry);
    case "open-external":
      return openExternal(ep);
    case "subtitles":
      return subtitles(ep);
    case "toggle-watched":
      return setWatched(ep, !ep.video.watched);
  }
}

/** Keys the menu bar can't own without breaking text fields. */
function onKey(event: KeyboardEvent) {
  if (event.defaultPrevented || sheet.value || contextMenu.value || palette.value || isTyping(event.target)) return;
  const onButton = (event.target as HTMLElement | null)?.closest("button:not([data-nav-item])");
  if (event.key === " " && !onButton) {
    event.preventDefault();
    playOrPause(event.shiftKey);
  } else if (event.key === "Backspace" && event.metaKey && selected.value) {
    event.preventDefault();
    moveToTrash(selected.value.entry);
  } else if (event.key === "z" && event.metaKey && !event.shiftKey) {
    event.preventDefault();
    undoLast();
  } else if (event.key === "Escape" && hud.value) {
    hud.value = null;
  }
}

function onPaste(event: ClipboardEvent) {
  if (isTyping(event.target) || sheet.value) return;
  const text = event.clipboardData?.getData("text")?.trim() ?? "";
  if (text.startsWith("magnet:")) {
    event.preventDefault();
    sheet.value = { kind: "magnet", link: text };
  }
}

const setActive = (active: boolean) => document.documentElement.classList.toggle("inactive", !active);
const onFocus = () => setActive(true);
const onBlur = () => setActive(false);

let stopSync: () => void = () => {};
const unlistenMenu = listen<string>("menu", (event) => onMenu(event.payload));

// A magnet link clicked in the browser. It replaces any open sheet: the click was on purpose.
async function openPendingMagnet() {
  const link = await api.takePendingMagnet().catch(() => null);
  if (link) sheet.value = { kind: "magnet", link };
}
const unlistenMagnet = listen("open-magnet", openPendingMagnet);

onMounted(() => {
  openPendingMagnet();
  document.documentElement.classList.toggle("mac", navigator.userAgent.includes("Mac"));
  stopSync = startSync();
  window.addEventListener("keydown", onKey);
  window.addEventListener("paste", onPaste);
  window.addEventListener("focus", onFocus);
  window.addEventListener("blur", onBlur);
});
onUnmounted(() => {
  stopSync();
  unlistenMenu.then((stop) => stop());
  unlistenMagnet.then((stop) => stop());
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("focus", onFocus);
  window.removeEventListener("blur", onBlur);
});
</script>

<template>
  <div class="flex h-screen flex-col">
    <AppToolbar />
    <div class="flex min-h-0 flex-1">
      <main ref="main" class="relative min-w-0 flex-1 overflow-y-auto bg-background">
        <ErrorNote v-if="loadError" :error="`Couldn't read the library: ${loadError}`" class="m-34" />
        <div v-if="!loaded" class="grid h-full place-items-center p-34">
          <Loader :size="89" text="Loading your library" />
        </div>
        <UpNextView v-else-if="route.name === 'upnext'" />
        <LibraryView v-else-if="route.name === 'films'" kind="film" />
        <LibraryView v-else-if="route.name === 'shows'" kind="show" />
        <DownloadsView v-else-if="route.name === 'downloads'" />
        <SearchView v-else-if="route.name === 'search'" :query="route.query" />
        <TitleView v-else-if="route.name === 'title'" :key="`${route.title.kind}:${route.title.id}`" :title="route.title" />
        <LocalShowView v-else-if="route.name === 'localShow'" :show-key="route.key" />
        <PreferencesView v-else-if="route.name === 'preferences'" :pane="route.pane" />
        <HelpView v-else-if="route.name === 'help'" :topic="route.topic" />
      </main>
    </div>
    <Toast />

    <OpeningPanel v-if="opening" :key="`${opening.itemId}:${opening.fileIndex}`" :label="opening.label" />
    <NowPlayingHud v-if="hud?.kind === 'nowPlaying'" />
    <CaptionsPanel
      v-else-if="hud?.kind === 'captions'"
      :key="`${hud.ep.entry.id}:${hud.ep.video.fileIndex}`"
      :entry="hud.ep.entry"
      :video="hud.ep.video"
      @close="hud = null"
    />
    <SheetHost />
    <CommandPalette />
    <ContextMenu />
  </div>
</template>
