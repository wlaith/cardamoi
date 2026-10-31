<script setup lang="ts">
import { computed } from "vue";
import { replace, type PreferencesPane } from "../../nav";
import GeneralPane from "../preferences/GeneralPane.vue";
import SourcesPane from "../preferences/SourcesPane.vue";
import SubtitlesPane from "../preferences/SubtitlesPane.vue";
import TmdbPane from "../preferences/TmdbPane.vue";
import PageHeader from "../ui/PageHeader.vue";
import TabList from "../ui/TabList.vue";

const props = defineProps<{ pane: PreferencesPane }>();

const panes: { id: PreferencesPane; label: string }[] = [
  { id: "general", label: "General" },
  { id: "tmdb", label: "TMDB" },
  { id: "subtitles", label: "Subtitles" },
  { id: "sources", label: "Sources" },
];

const current = computed<PreferencesPane | null>({
  get: () => props.pane,
  set: (pane) => pane && replace({ name: "preferences", pane }),
});
</script>

<template>
  <div class="space-y-55 p-34">
    <div class="space-y-21">
      <PageHeader title="Preferences" subtitle="Where cardamoi looks up titles and finds releases, subtitles and playback." />
      <TabList v-model="current" :items="panes" label="Preferences" />
    </div>
    <GeneralPane v-if="pane === 'general'" />
    <TmdbPane v-else-if="pane === 'tmdb'" />
    <SubtitlesPane v-else-if="pane === 'subtitles'" />
    <SourcesPane v-else />
  </div>
</template>
