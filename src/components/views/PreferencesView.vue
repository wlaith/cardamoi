<script setup lang="ts">
import { replace, type PreferencesPane } from "../../nav";
import GeneralPane from "../preferences/GeneralPane.vue";
import SourcesPane from "../preferences/SourcesPane.vue";
import SubtitlesPane from "../preferences/SubtitlesPane.vue";
import PageHeader from "../ui/PageHeader.vue";

defineProps<{ pane: PreferencesPane }>();

const panes: { id: PreferencesPane; label: string }[] = [
  { id: "general", label: "General" },
  { id: "subtitles", label: "Subtitles" },
  { id: "sources", label: "Sources" },
];
</script>

<template>
  <div class="space-y-55 p-34">
    <div class="space-y-21">
      <PageHeader title="Preferences" subtitle="Where cardamoi looks for releases, subtitles and playback." />
      <nav class="tabs" role="tablist" aria-label="Preferences">
        <button
          v-for="p in panes"
          :key="p.id"
          class="tab"
          role="tab"
          :aria-selected="pane === p.id"
          @click="replace({ name: 'preferences', pane: p.id })"
        >
          {{ p.label }}
        </button>
      </nav>
    </div>
    <GeneralPane v-if="pane === 'general'" />
    <SubtitlesPane v-else-if="pane === 'subtitles'" />
    <SourcesPane v-else />
  </div>
</template>
