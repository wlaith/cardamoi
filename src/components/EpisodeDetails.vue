<script setup lang="ts">
import { computed, ref } from "vue";
import { formatBytes, timeLeft } from "../api";
import { getInfo, moveToTrash, pause, play, resume, reveal, setWatched, subtitles } from "../commands";
import type { LocalEpisode } from "../library";
import { readyIn } from "../status";
import { openMenuBelow, type MenuEntry } from "../ui";
import Icon from "./ui/Icon.vue";

const props = defineProps<{
  local: LocalEpisode;
  still?: string | null;
  overview?: string;
  noReleases?: boolean;
}>();
const emit = defineEmits<{ findReleases: [] }>();

const v = computed(() => props.local.video);
const entry = computed(() => props.local.entry);
const downloading = computed(() => entry.value.status === "downloading" || entry.value.status === "starting");

const meta = computed(() => {
  const parts = [formatBytes(v.value.len)];
  if (v.value.watched) parts.push("Watched");
  else if (v.value.resumeAt != null) parts.push(timeLeft(v.value));
  else if (v.value.playable) parts.push(entry.value.status === "done" ? "Downloaded" : "Ready to watch while it downloads");
  else if (entry.value.status === "paused") parts.push(`Download paused at ${Math.floor(v.value.progress * 100)} %`);
  else parts.push(`Downloading, ${Math.floor(v.value.progress * 100)} %`, readyIn(v.value.readyInSecs) || "Finding peers");
  return parts.filter(Boolean).join(" · ");
});

const more = ref<HTMLElement>();

function moreMenu() {
  const items: MenuEntry[] = [
    { label: v.value.watched ? "Mark as Unwatched" : "Mark as Watched", action: () => setWatched(props.local, !v.value.watched) },
    { label: "Get Info", action: () => getInfo(props.local) },
  ];
  if (downloading.value) items.push({ label: "Pause Download", action: () => pause(entry.value) });
  else if (entry.value.status === "paused") items.push({ label: "Resume Download", action: () => resume(entry.value) });
  if (!props.noReleases) items.push("separator", { label: "Find Another Release", action: () => emit("findReleases") });
  openMenuBelow(more.value!, items);
}
</script>

<template>
  <div class="flex gap-6 py-6 pr-4 pl-[72px]">
    <img v-if="still" :src="still" alt="" class="poster-frame aspect-video w-[320px] shrink-0 object-cover" />
    <div class="min-w-0 flex-1">
      <p v-if="overview" class="max-w-[72ch] text-ink selectable">{{ overview }}</p>
      <p class="mt-4 text-small tabular-nums text-ink-2">{{ meta }}</p>
      <div class="mt-6 flex flex-wrap items-center gap-x-2 gap-y-2">
        <button v-if="v.playable" class="btn btn-primary" @click="play(local, v.watched)">
          {{ v.resumeAt != null ? "Continue" : v.watched ? "Play again" : "Play" }} <Icon name="play" />
        </button>
        <button v-if="v.resumeAt != null" class="btn btn-ghost" @click="play(local, true)">Play from beginning</button>
        <button class="btn btn-ghost" @click="subtitles(local)">Subtitles <Icon name="captions" /></button>
        <button v-if="entry.status !== 'missing' && entry.status !== 'resolving'" class="btn btn-ghost" @click="reveal(entry)">
          Show in Finder <Icon name="folder" />
        </button>
        <button class="btn btn-danger-ghost" @click="moveToTrash(entry)">Move to Trash <Icon name="trash" /></button>
        <button ref="more" class="btn-icon" aria-label="More" aria-haspopup="menu" @click="moreMenu"><Icon name="overflow" /></button>
      </div>
    </div>
  </div>
</template>
