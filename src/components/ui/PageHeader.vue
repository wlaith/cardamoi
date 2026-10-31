<script setup lang="ts">

defineProps<{ title: string; subtitle?: string; keepCase?: boolean }>();
</script>

<template>
  <header class="flex items-end gap-21">
    <div class="min-w-0 flex-1">
      <h1 class="text-title truncate" :class="{ lowercase: !keepCase }">{{ title }}</h1>
      <p v-if="subtitle" class="mt-13 text-ink-2">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" class="flex shrink-0 items-center gap-13">
      <slot name="actions" />
    </div>
  </header>
</template>
