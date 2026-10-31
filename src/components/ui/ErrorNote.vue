<script setup lang="ts">
import { computed, ref } from "vue";
import { plainError } from "../../api";
import Icon from "./Icon.vue";

const props = defineProps<{ error: unknown; title?: string }>();
const shown = computed(() => plainError(props.error));
const open = ref(false);
</script>

<template>
  <div class="notification notification-error" role="alert">
    <Icon name="error-filled" class="notification-icon" />
    <div class="min-w-0 flex-1">
      <p class="selectable">
        <strong v-if="title" class="mr-1 font-semibold">{{ title }}</strong>
        {{ shown.message }}
      </p>
      <template v-if="shown.details">
        <button class="mt-1 flex items-center gap-1 text-small tabular-nums text-link" :aria-expanded="open" @click="open = !open">
          <Icon name="chevron-right" :size="12" :class="{ 'rotate-90': open }" />
          Details
        </button>
        <p v-if="open" class="mt-1 text-small break-all text-ink-2 selectable">{{ shown.details }}</p>
      </template>
    </div>
    <slot />
  </div>
</template>
