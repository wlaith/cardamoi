<script setup lang="ts">
import { computed, ref } from "vue";
import { episodeLabel } from "../../api";
import { play } from "../../commands";
import { nextEpisode, showKey } from "../../library";
import { shelf } from "../../store";
import EpisodeDetails from "../EpisodeDetails.vue";
import EpisodeTable, { type EpisodeRowData } from "../EpisodeTable.vue";
import Icon from "../ui/Icon.vue";
import { go } from "../../nav";

const props = defineProps<{ showKey: string }>();

const show = computed(() => {
  const found = shelf.value.find((s) => s.kind === "show" && showKey(s.show) === props.showKey);
  return found?.kind === "show" ? found.show : null;
});
const next = computed(() => (show.value ? nextEpisode(show.value) : null));
const openRow = ref<string | null>(null);

const rows = computed<EpisodeRowData[]>(
  () =>
    show.value?.episodes.map((ep) => ({
      key: `${ep.entry.id}:${ep.video.fileIndex}`,
      number: ep.video.episode ?? 0,
      name: episodeLabel(ep.video.season, ep.video.episode) || ep.video.path,
      local: ep,
    })) ?? [],
);
</script>

<template>
  <div v-if="!show" class="grid h-full place-items-center p-34">
    <p class="empty-state">This show is no longer in your library</p>
  </div>
  <article v-else class="space-y-55 p-34">
    <div class="space-y-21">
      <nav class="breadcrumb" aria-label="Breadcrumb">
        <button @click="go({ name: 'shows' })">TV Shows</button>
        <span class="text-ink-2" aria-hidden="true">/</span>
        <span class="truncate" aria-current="page">{{ show.title }}</span>
      </nav>
      <div class="flex gap-34">
        <img v-if="show.poster" :src="show.poster" alt="" class="poster-frame h-[300px] w-[200px] shrink-0 object-cover" />
        <div v-else class="poster-frame grid h-[300px] w-[200px] shrink-0 place-items-center p-4 text-center heading-03 text-ink-2">
          {{ show.title }}
        </div>
        <div class="min-w-0 flex-1 space-y-4">
          <h1 class="heading-07">{{ show.title }}</h1>
          <p class="body-02 text-ink-2">Not matched to the TMDB catalog, so only the episodes on disk are listed.</p>
          <div class="pt-2">
            <button v-if="next" class="btn btn-primary btn-lg" @click="play(next)">
              {{ next.video.resumeAt != null ? "Continue" : "Play" }} {{ episodeLabel(next.video.season, next.video.episode) }}
              <Icon name="play" />
            </button>
          </div>
        </div>
      </div>
    </div>
    <EpisodeTable :rows="rows" :selected="openRow" @select="openRow = $event">
      <template #detail="{ row }">
        <EpisodeDetails v-if="row.local" :local="row.local" no-releases />
      </template>
    </EpisodeTable>
  </article>
</template>
