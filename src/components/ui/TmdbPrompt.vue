<script setup lang="ts">
import { computed } from "vue";
import { go } from "../../nav";
import { tmdbStatus } from "../../tmdb";
import Icon from "./Icon.vue";

const props = defineProps<{ feature: string; compact?: boolean }>();
const rejected = computed(() => tmdbStatus.value?.state === "rejected");
const line = computed(() =>
  rejected.value ? `TMDB turned down your key, so ${props.feature} is paused.` : `${props.feature[0].toUpperCase()}${props.feature.slice(1)} needs a TMDB key.`,
);

function fix() {
  if (rejected.value) go({ name: "preferences", pane: "tmdb" });
  else go({ name: "setup" });
}
</script>

<template>
  <div :class="compact ? 'space-y-2' : 'max-w-md space-y-13 py-8'">
    <Icon v-if="!compact" name="key" :size="21" class="text-ink-2" />
    <p :class="compact ? '' : 'text-heading'">{{ line }}</p>
    <p v-if="!compact" class="text-ink-2">It's free and takes about two minutes. cardamoi walks you through it.</p>
    <button type="button" :class="compact ? 'btn btn-ghost -ml-2' : 'btn btn-secondary'" @click="fix">
      {{ rejected ? "Update key" : "Add a TMDB key" }} <Icon name="arrow-right" />
    </button>
  </div>
</template>
