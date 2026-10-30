<script setup lang="ts">

defineProps<{ title: string; subtitle?: string; id?: string }>();
</script>

<template>
  <div>
    <div class="flex min-h-10 items-center gap-21">
      <h2 :id="id" class="heading-03 min-w-0 flex-1 truncate">{{ title }}</h2>
      <slot name="action" />
    </div>
    <p v-if="subtitle" class="mt-2 text-ink-2">{{ subtitle }}</p>
  </div>
</template>
