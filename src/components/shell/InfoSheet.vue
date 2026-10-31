<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api, episodeLabel, formatBytes, languageName, timeLeft, type Subtitle } from "../../api";
import { reveal, videoLabel } from "../../commands";
import type { LocalEpisode } from "../../library";
import { closeSheet } from "../../ui";

const props = defineProps<{ ep: LocalEpisode }>();
const subtitles = ref<Subtitle[] | null>(null);

onMounted(async () => {
  try {
    subtitles.value = await api.listSubtitles(props.ep.entry.id, props.ep.video.fileIndex);
  } catch {
    subtitles.value = [];
  }
});

const e = computed(() => props.ep.entry);
const v = computed(() => props.ep.video);

const status = computed(() => {
  if (v.value.watched) return "Watched";
  if (v.value.resumeAt != null) return `Stopped part way, ${timeLeft(v.value)}`;
  if (e.value.status === "done") return "Downloaded";
  if (e.value.status === "paused") return `Download paused at ${Math.floor(v.value.progress * 100)} %`;
  if (e.value.status === "error") return "Download failed";
  return `Downloading, ${Math.floor(v.value.progress * 100)} %`;
});

const rows = computed(() =>
  [
    ["Kind", e.value.kind === "tv" || v.value.season != null ? "TV episode" : "Film"],
    ["Episode", episodeLabel(v.value.season, v.value.episode)],
    ["Year", e.value.year ? String(e.value.year) : ""],
    ["Size", formatBytes(v.value.len)],
    ["Status", status.value],
    ["Came from", e.value.source],
    ["Added", new Date(e.value.added_at * 1000).toLocaleDateString(undefined, { dateStyle: "long" })],
    ["File", v.value.path],
  ].filter(([, value]) => value),
);
</script>

<template>
  <div class="px-4 pt-4 pr-16">
    <p class="text-small tabular-nums text-ink-2">Get info</p>
    <h2 class="mt-1 text-heading">{{ videoLabel(ep) }}</h2>
  </div>
  <div class="flex gap-6 px-4 pt-6">
    <img v-if="e.poster" :src="e.poster" alt="" class="poster-frame h-[150px] w-[100px] shrink-0 object-cover" />
    <dl class="grid min-w-0 flex-1 grid-cols-[auto_1fr] gap-x-4 gap-y-2">
      <template v-for="[label, value] in rows" :key="label">
        <dt class="text-ink-2">{{ label }}</dt>
        <dd class="min-w-0 break-words selectable">{{ value }}</dd>
      </template>
      <dt class="text-ink-2">Subtitles</dt>
      <dd>
        <template v-if="subtitles == null">Looking</template>
        <template v-else-if="!subtitles.length">None yet</template>
        <template v-else>{{ subtitles.map((s) => languageName(s.lang)).join(", ") }}</template>
      </dd>
    </dl>
  </div>
  <div class="modal-footer">
    <button class="btn btn-secondary" @click="reveal(e)">Show in Finder</button>
    <button class="btn btn-primary" @click="closeSheet()">Done</button>
  </div>
</template>
