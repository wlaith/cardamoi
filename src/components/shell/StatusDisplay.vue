<script setup lang="ts">
import { computed, ref } from "vue";
import { episodeLabel, formatBytes } from "../../api";
import { activeDownloads, entries, opening, player, playing } from "../../store";
import { downloadsOpen, hud } from "../../ui";
import Icon from "../ui/Icon.vue";
import Spinner from "../ui/Spinner.vue";
import DownloadsPopover from "./DownloadsPopover.vue";

function clock(secs: number) {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

const downloads = computed(() => {
  const list = activeDownloads.value;
  const total = list.reduce((sum, e) => sum + e.totalBytes, 0);
  const done = list.reduce((sum, e) => sum + e.downloadedBytes, 0);
  const speed = list.reduce((sum, e) => sum + e.speedMibps, 0);
  return { count: list.length, total, done, speed };
});

const mode = computed(() => {
  if (opening.value) return "opening";
  if (playing.value) return "playing";
  if (downloads.value.count) return "downloads";
  return "idle";
});

const playingLabel = computed(() => {
  const p = player.value;
  if (!p) return "";
  const entry = entries.value.find((e) => e.id === p.itemId);
  const video = entry?.videos.find((v) => v.fileIndex === p.fileIndex);
  const ep = video ? episodeLabel(video.season, video.episode) : "";
  return entry ? [entry.title, ep].filter(Boolean).join(" · ") : p.title;
});

const share = computed(() => {
  if (mode.value === "playing") return player.value!.duration ? player.value!.position / player.value!.duration : 0;
  if (mode.value === "downloads") return downloads.value.total ? downloads.value.done / downloads.value.total : 0;
  return 0;
});

const button = ref<HTMLButtonElement>();

function click() {
  if (mode.value === "playing") hud.value = hud.value?.kind === "nowPlaying" ? null : { kind: "nowPlaying" };
  else downloadsOpen.value = !downloadsOpen.value;
}
</script>

<template>
  <div class="relative flex shrink-0 self-stretch">
    <button
      ref="button"
      class="status-button my-2 flex items-center gap-13 px-13"
      :aria-label="mode === 'playing' ? 'Now playing. Show controls' : 'Downloads'"
      :aria-haspopup="mode === 'playing' ? undefined : 'dialog'"
      :aria-expanded="mode === 'playing' ? undefined : downloadsOpen"
      @click="click"
    >
      <Spinner v-if="mode === 'opening'" :size="16" />
      <Icon v-else :name="mode === 'playing' ? 'play' : 'download'" :class="mode === 'idle' ? 'text-ink-2' : 'text-ink'" />
      <span class="flex max-w-[280px] min-w-36 flex-col gap-1 pt-1 text-small tabular-nums" :class="{ 'text-ink-2': mode === 'idle' }" role="status">
        <span class="truncate">
          <template v-if="mode === 'opening'">Opening {{ opening!.label }}</template>
          <template v-else-if="mode === 'playing'">
            {{ playingLabel }} · {{ clock(player!.position) }}
          </template>
          <template v-else-if="mode === 'downloads'">
            {{ downloads.count }} downloading · {{ downloads.speed.toFixed(1) }} MB/s
          </template>
          <template v-else>No downloads</template>
        </span>
        <span class="block h-1 bg-line" :class="{ invisible: mode === 'idle' || mode === 'opening' }" aria-hidden="true">
          <span class="block h-full bg-[var(--c-mark)] transition-[width] duration-[600ms]" :style="{ width: `${share * 100}%` }" />
        </span>
      </span>
      <span v-if="mode === 'downloads' && downloads.total" class="sr-only">
        {{ formatBytes(downloads.done) }} of {{ formatBytes(downloads.total) }}
      </span>
    </button>
    <DownloadsPopover :trigger="button" />
  </div>
</template>
