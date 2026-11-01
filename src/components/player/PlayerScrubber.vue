<script setup lang="ts">
import { computed, ref } from "vue";
import { formatTime, remaining } from "../../player/time";

const props = defineProps<{ position: number; duration: number | null; cacheUntil: number | null }>();
/** `exact` is false while dragging (mpv seeks to keyframes, which is quick) and true on release. */
const emit = defineEmits<{ seek: [secs: number, exact: boolean] }>();

const track = ref<HTMLElement>();
const hover = ref<number | null>(null);
const dragging = ref(false);
const dragAt = ref(0);
let pending = 0;

const total = computed(() => props.duration ?? 0);
const share = (secs: number) => (total.value > 0 ? Math.min(1, Math.max(0, secs / total.value)) : 0);
const shown = computed(() => (dragging.value ? dragAt.value : props.position));
const played = computed(() => share(shown.value) * 100);
const cached = computed(() => share(props.cacheUntil ?? 0) * 100);

function secsAt(event: PointerEvent) {
  const box = track.value!.getBoundingClientRect();
  return Math.min(1, Math.max(0, (event.clientX - box.left) / box.width)) * total.value;
}

function down(event: PointerEvent) {
  if (total.value <= 0) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  dragging.value = true;
  dragAt.value = secsAt(event);
  emit("seek", dragAt.value, false);
}

function move(event: PointerEvent) {
  if (total.value <= 0) return;
  hover.value = secsAt(event);
  if (!dragging.value) return;
  dragAt.value = hover.value;
  // One seek per frame while dragging.
  cancelAnimationFrame(pending);
  pending = requestAnimationFrame(() => emit("seek", dragAt.value, false));
}

function up(event: PointerEvent) {
  if (!dragging.value) return;
  cancelAnimationFrame(pending);
  dragging.value = false;
  emit("seek", secsAt(event), true);
}

/** The label sits over the pointer, but never past the ends of the track. */
const labelLeft = computed(() => `clamp(21px, ${share(hover.value ?? 0) * 100}%, calc(100% - 21px))`);
</script>

<template>
  <div class="scrubber">
    <span class="time">{{ formatTime(shown) }}</span>
    <div
      ref="track"
      class="track"
      :class="{ active: hover !== null || dragging }"
      role="slider"
      aria-label="Seek"
      :aria-valuemin="0"
      :aria-valuemax="Math.round(total)"
      :aria-valuenow="Math.round(shown)"
      :aria-valuetext="formatTime(shown)"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="up"
      @pointercancel="up"
      @pointerleave="hover = null"
    >
      <div class="rail">
        <div class="cached" :style="{ width: `${cached}%` }" />
        <div class="played" :style="{ width: `${played}%` }" />
      </div>
      <div class="playhead" :style="{ left: `${played}%` }" />
      <span v-if="hover !== null" class="tip" :style="{ left: labelLeft }">{{ formatTime(hover) }}</span>
    </div>
    <span class="time muted">{{ remaining(shown, total) }}</span>
  </div>
</template>

<style scoped>
.scrubber {
  display: flex;
  height: 21px;
  align-items: center;
  gap: 13px;
}
.time {
  width: 55px;
  font-family: var(--font-mono);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}
.time.muted {
  text-align: right;
  color: var(--c-ink-muted);
}
.track {
  position: relative;
  flex: 1;
  height: 21px;
  cursor: pointer;
  touch-action: none;
}
.rail {
  position: absolute;
  inset: 0;
  top: 8.5px;
  height: 4px;
  background: var(--c-line);
  transition: height var(--dur-1) var(--ease-out), top var(--dur-1) var(--ease-out);
}
.track.active .rail {
  top: 6.5px;
  height: 8px;
}
.cached,
.played {
  position: absolute;
  inset: 0 auto 0 0;
}
.cached {
  background: color-mix(in srgb, var(--c-ink-muted) 55%, transparent);
}
.played {
  background: var(--c-red);
}
.playhead {
  position: absolute;
  top: 0;
  width: 2px;
  height: 21px;
  margin-left: -1px;
  background: var(--c-ink);
}
.tip {
  position: absolute;
  bottom: 29px;
  translate: -50% 0;
  padding: 2px 8px;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--c-ink);
  background: var(--c-surface-raised);
  border: 1px solid var(--c-line);
  pointer-events: none;
}
</style>
