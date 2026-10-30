<script setup lang="ts">
import { computed } from "vue";
import { moveToTrash, openFilm, openShow, play, showMenu, videoMenu } from "../commands";
import { go } from "../nav";
import { filmVideo, nextEpisode, playedShare, showKey, watchedShare, type Shelf } from "../library";
import { selection } from "../store";
import { filmLine, showLine } from "../status";
import { openMenu } from "../ui";
import PosterTile from "./ui/PosterTile.vue";

const props = defineProps<{ item: Shelf }>();

const film = computed(() => (props.item.kind === "film" ? filmVideo(props.item.entry) : null));
const show = computed(() => (props.item.kind === "show" ? props.item.show : null));
const toPlay = computed(() => film.value ?? (show.value ? nextEpisode(show.value) : null));

const selected = computed(() => {
  const s = selection.value;
  if (!s) return false;
  if (show.value) return s.kind === "show" && showKey(s.show) === showKey(show.value);
  return s.kind === "video" && props.item.kind === "film" && s.ep.entry.id === props.item.entry.id;
});

const line = computed(() =>
  props.item.kind === "film" ? filmLine(props.item.entry) : { text: showLine(props.item.show), accent: false },
);

const tag = computed<{ text: string; kind: "green" | "blue" } | null>(() => {
  if (props.item.kind !== "film") return null;
  const entry = props.item.entry;
  if (entry.status === "done") return film.value?.video.watched ? { text: "Watched", kind: "blue" } : null;
  if (film.value?.video.playable) return { text: "Ready", kind: "green" };
  if (entry.status === "downloading" || entry.status === "paused") return { text: `${Math.floor(entry.progress * 100)} %`, kind: "blue" };
  return null;
});

const fill = computed(() => {
  if (props.item.kind !== "film" || props.item.entry.status === "done") return null;
  return props.item.entry.progress;
});

function select() {
  if (show.value) selection.value = { kind: "show", show: show.value };
  else if (film.value) selection.value = { kind: "video", ep: film.value };
}

function open() {
  if (show.value) openShow(show.value);
  else if (film.value) openFilm(film.value);
  else go({ name: "downloads" });
}

function menu(event: MouseEvent) {
  if (show.value) openMenu(event, showMenu(show.value));
  else if (film.value) openMenu(event, videoMenu(film.value, { open }));
  else if (props.item.kind === "film") {
    const entry = props.item.entry;
    openMenu(event, [
      { label: "Show in Downloads", action: open },
      { label: "Cancel Download…", action: () => moveToTrash(entry), danger: true },
    ]);
  }
}
</script>

<template>
  <PosterTile
    :poster="item.kind === 'film' ? item.entry.poster : item.show.poster"
    :title="item.kind === 'film' ? item.entry.title : item.show.title"
    :line="line.text"
    :tag="tag?.text"
    :tag-kind="tag?.kind"
    :played="film ? (film.video.resumeAt != null ? playedShare(film.video) : 0) : show ? watchedShare(show) : 0"
    :fill="fill"
    :selected="selected"
    :playable="!!toPlay?.video.playable"
    @select="select"
    @open="open"
    @play="toPlay && play(toPlay, toPlay.video.watched)"
    @menu="menu"
  />
</template>
