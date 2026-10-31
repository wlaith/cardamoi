<script setup lang="ts">
import { computed } from "vue";
import { go, route } from "../../nav";
import { bannerDismissed, tmdbStatus } from "../../tmdb";
import Icon from "../ui/Icon.vue";

const state = computed(() => tmdbStatus.value?.state);
const shown = computed(
  () =>
    route.value.name !== "setup" &&
    !(route.value.name === "preferences" && route.value.pane === "tmdb") &&
    (state.value === "rejected" || (state.value === "missing" && !bannerDismissed.value)),
);
</script>

<template>
  <div v-if="shown" class="tmdb-banner" :class="{ 'tmdb-banner-error': state === 'rejected' }" role="status">
    <Icon :name="state === 'rejected' ? 'warning-filled' : 'key'" class="tmdb-banner-icon" />
    <p class="min-w-0 flex-1">
      <template v-if="state === 'rejected'">TMDB turned down your key. Search and matching are paused.</template>
      <template v-else>Search, artwork and matching are off until you add a TMDB key.</template>
    </p>
    <button
      type="button"
      class="btn btn-secondary btn-sm"
      @click="state === 'rejected' ? go({ name: 'preferences', pane: 'tmdb' }) : go({ name: 'setup' })"
    >
      {{ state === "rejected" ? "Update key" : "Add key" }}
    </button>
    <button
      v-if="state === 'missing'"
      type="button"
      class="btn btn-ghost btn-sm btn-icon"
      aria-label="Hide until next launch"
      title="Hide until next launch"
      @click="bannerDismissed = true"
    >
      <Icon name="close" />
    </button>
  </div>
</template>

<style scoped>
.tmdb-banner {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 13px;
  padding: 5px 13px 5px 21px;
  color: var(--c-ink);
  background: var(--c-surface-raised);
  border-bottom: 1px solid var(--c-line);
  box-shadow: inset 3px 0 0 var(--c-ultramarine);
}
.tmdb-banner-error {
  box-shadow: inset 3px 0 0 var(--c-red-text);
}
.tmdb-banner-error .tmdb-banner-icon {
  color: var(--c-red-text);
}
</style>
