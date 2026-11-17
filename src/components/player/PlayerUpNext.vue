<script setup lang="ts">
import { api } from "../../api";
import Kbd from "../ui/Kbd.vue";

defineProps<{ title: string; secondsLeft: number }>();
</script>

<template>
  <section class="card" aria-live="polite" aria-label="Up next">
    <p class="eyebrow">Up next</p>
    <p class="title">{{ title }}</p>
    <p class="count">Playing in {{ secondsLeft }} s</p>
    <div class="actions">
      <button class="btn btn-primary" @click="api.playerCommand(['script-message', 'cardamoi-next'])">
        Play now <Kbd keys="↵" />
      </button>
      <button class="btn btn-ghost" @click="api.playerCommand(['script-message', 'cardamoi-cancel'])">
        Keep watching <Kbd keys="Esc" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.card {
  display: flex;
  width: 377px;
  flex-direction: column;
  gap: 8px;
  padding: 21px;
  color: var(--c-ink);
  background: var(--c-surface-raised);
  border: 1px solid var(--c-ink);
}
.eyebrow,
.count {
  font-family: var(--font-mono);
  font-size: 12px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--c-ink-muted);
}
.count {
  font-size: 13px;
  letter-spacing: 0;
  text-transform: none;
}
.title {
  font-size: 20px;
  line-height: 24px;
  font-weight: 600;
}
.actions {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 8px;
}
</style>
