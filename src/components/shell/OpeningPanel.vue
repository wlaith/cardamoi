<script setup lang="ts">
import { stopOpening } from "../../store";
import Icon from "../ui/Icon.vue";
import Loader from "../ui/Loader.vue";

defineProps<{ label: string }>();
</script>

<template>
  <section class="hud fixed top-[68px] right-21 z-30 flex w-[377px] items-center gap-13 py-13 pr-2 pl-13" aria-live="polite">
    <div class="size-12 shrink-0"><Loader :size="48" /></div>
    <div class="min-w-0 flex-1">
      <p class="eyebrow">Opening</p>
      <p class="truncate font-semibold">{{ label }}</p>
      <p class="truncate text-ink-2">Starting playback.</p>
    </div>
    <button class="btn-icon self-start" aria-label="Dismiss" @click="stopOpening()"><Icon name="close" /></button>
  </section>
</template>
