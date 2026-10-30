<script setup lang="ts">
defineProps<{
  value: number;
  state?: "active" | "paused" | "failed" | "idle";
  thin?: boolean;
  label?: string;
}>();
</script>

<template>
  <div
    class="progress"
    :class="{ 'progress-thin': thin }"
    :data-state="state ?? 'idle'"
    role="progressbar"
    :aria-label="label"
    :aria-valuenow="Math.round(value * 100)"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <div :style="{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }" />
  </div>
</template>
