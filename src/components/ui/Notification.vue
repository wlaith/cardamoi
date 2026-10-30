<script setup lang="ts">
import Icon from "./Icon.vue";

withDefaults(defineProps<{ kind?: "info" | "success" | "warning"; title?: string }>(), { kind: "info" });
</script>

<template>
  <div class="notification" :class="`notification-${kind}`" role="status">
    <Icon :name="kind === 'success' ? 'checkmark-filled' : kind === 'warning' ? 'warning-filled' : 'info-filled'" class="notification-icon" />
    <div class="min-w-0 flex-1">
      <p v-if="title" class="font-semibold">{{ title }}</p>
      <p><slot /></p>
    </div>
    <slot name="action" />
  </div>
</template>
