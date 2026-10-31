<script setup lang="ts">
import Icon from "./Icon.vue";
defineProps<{
  poster: string | null;
  title: string;
  line?: string;
  tag?: string;
  tagKind?: "green" | "blue" | "gray" | "new";
  played?: number;
  fill?: number | null;
  selected?: boolean;
  checked?: boolean;
  playable?: boolean;
}>();
const emit = defineEmits<{ select: [event?: MouseEvent]; open: []; play: []; menu: [event: MouseEvent] }>();

function onMenu(event: MouseEvent) {
  emit("select");
  emit("menu", event);
}
</script>

<template>
  <li class="flex min-w-0 flex-col" :class="{ 'tile-selected': selected || checked }">
    <div class="group relative aspect-[2/3]">
      <button
        data-nav-item
        class="poster-frame absolute inset-0 overflow-hidden text-left"
        :aria-label="[title, line].filter(Boolean).join('. ')"
        :aria-selected="selected"
        @click="emit('select', $event)"
        @focus="emit('select')"
        @dblclick="emit('open')"
        @keydown.enter.prevent="emit('open')"
        @contextmenu="onMenu"
      >
        <img v-if="poster" :src="poster" alt="" loading="lazy" draggable="false" class="absolute inset-0 size-full object-cover" />
        <span v-else class="absolute inset-0 grid place-items-center p-4 text-center text-heading text-ink-2">{{ title }}</span>
        <span v-if="fill != null || played" class="absolute inset-x-0 bottom-0 h-1 bg-line" aria-hidden="true">
          <span class="block h-full bg-[var(--c-mark)] transition-[width] duration-700" :style="{ width: `${(fill ?? played ?? 0) * 100}%` }" />
        </span>
        <span v-if="tag" class="tag absolute top-2 left-2" :class="tagKind === 'new' ? 'tag-new' : 'tag-solid'">{{ tag }}</span>
      </button>
      <span v-if="checked" class="tile-check" aria-hidden="true"><Icon name="checkmark" :size="13" /></span>
      <button v-if="playable" tabindex="-1" class="tile-play" :aria-label="`Play ${title}`" @click="emit('play')">
        <Icon name="play" />
      </button>
    </div>
    <button tabindex="-1" class="mt-2 truncate text-left font-semibold underline-offset-4 hover:underline" :title="title" @click="emit('open')">
      {{ title }}
    </button>
    <p v-if="line" class="truncate text-small tabular-nums text-ink-2" :title="line">{{ line }}</p>
  </li>
</template>

<style scoped>
.tile-play {
  position: absolute;
  top: 50%;
  left: 50%;
  display: grid;
  width: 40px;
  height: 40px;
  place-items: center;
  color: var(--c-on-red);
  background: var(--c-red);
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, calc(-50% + 5px));
  transition:
    opacity var(--dur-1) var(--ease-out),
    transform var(--dur-2) var(--ease-out),
    background var(--dur-1) var(--ease-out);
}
.group:hover .tile-play {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, -50%);
}
.tile-play:hover {
  color: var(--brand-paper);
  background: var(--c-red-deep);
}
.tile-play:active {
  transform: translate(-50%, -50%) scale(0.92);
}

.tile-check {
  position: absolute;
  top: 8px;
  right: 8px;
  display: grid;
  width: 21px;
  height: 21px;
  place-items: center;
  color: var(--c-surface);
  background: var(--c-ink);
  animation: check-in var(--dur-1) var(--ease-out);
}
@keyframes check-in {
  from {
    opacity: 0;
    scale: 0.8;
  }
}
</style>
