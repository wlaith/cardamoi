<script setup lang="ts">
defineProps<{ size?: number; light?: boolean }>();
</script>

<template>
  <span class="slot-pending" :style="{ width: `${Math.max(21, (size ?? 16) * 2)}px` }" aria-hidden="true"><span /></span>
</template>

<style scoped>
.slot-pending {
  position: relative;
  display: inline-block;
  flex-shrink: 0;
  height: 4px;
  overflow: hidden;
  vertical-align: middle;
  background: var(--c-line);
}
.slot-pending > span {
  position: absolute;
  inset: 0 auto 0 0;
  width: 40%;
  background: var(--c-red);
  animation: travel 1s linear infinite;
}

@keyframes travel {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(250%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .slot-pending > span {
    animation: none;
    transform: translateX(75%);
  }
}
</style>
