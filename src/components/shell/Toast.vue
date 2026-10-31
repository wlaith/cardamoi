<script setup lang="ts">
import { dismissNotice, holdNotice, notice, releaseNotice, undoLast } from "../../store";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
</script>

<template>
  <div class="pointer-events-none fixed right-21 bottom-21 z-30" role="status" aria-live="polite">
    <Transition name="toast">
      <div
        v-if="notice"
        :key="notice.id"
        class="toast pointer-events-auto w-[420px] max-w-[calc(100vw-42px)]"
        :style="{ '--toast-life': `${notice.duration}ms` }"
        @pointerenter="holdNotice"
        @pointerleave="releaseNotice"
        @focusin="holdNotice"
        @focusout="releaseNotice"
      >
        <Icon :name="notice.commit ? 'trash' : 'checkmark-filled'" />
        <p class="min-w-0 flex-1 py-13">{{ notice.text }}</p>
        <template v-if="notice.undo">
          <button class="btn btn-ghost" @click="undoLast"><Icon name="undo" /> Undo</button>
          <Kbd keys="⌘Z" />
        </template>
        <button class="btn-icon" aria-label="Close" @click="dismissNotice"><Icon name="close" /></button>
        <span v-if="notice.undo" class="toast-life" aria-hidden="true" />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.toast {
  position: relative;
}
.toast-life {
  position: absolute;
  inset: auto 0 0;
  height: 2px;
  background: var(--c-ink);
  transform-origin: 0 50%;
  animation: life var(--toast-life) linear forwards;
}
.toast:hover .toast-life,
.toast:focus-within .toast-life {
  animation-play-state: paused;
}
@keyframes life {
  to {
    transform: scaleX(0);
  }
}

.toast-enter-active {
  transition:
    opacity var(--dur-3) var(--ease-out),
    translate var(--dur-3) var(--ease-out);
}
.toast-leave-active {
  transition: opacity var(--dur-2) var(--ease-out);
}
.toast-enter-from {
  opacity: 0;
  translate: 0 21px;
}
.toast-leave-to {
  opacity: 0;
}
.toast-leave-active {
  position: absolute;
  right: 0;
  bottom: 0;
}
</style>
