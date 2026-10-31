<script setup lang="ts">
defineProps<{ keys: string }>();
</script>

<template>
  <kbd class="kbd">{{ keys }}</kbd>
</template>
