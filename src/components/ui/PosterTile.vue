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
  playable?: boolean;
}>();
const emit = defineEmits<{ select: []; open: []; play: []; menu: [event: MouseEvent] }>();

function onMenu(event: MouseEvent) {
  emit("select");
  emit("menu", event);
}
</script>

<template>
  <li class="flex min-w-0 flex-col" :class="{ 'tile-selected': selected }">
    <div class="group relative aspect-[2/3]">
      <button
        data-nav-item
        class="poster-frame absolute inset-0 overflow-hidden text-left"
        :aria-label="[title, line].filter(Boolean).join('. ')"
        :aria-selected="selected"
        @click="emit('select')"
        @focus="emit('select')"
        @dblclick="emit('open')"
        @keydown.enter.prevent="emit('open')"
        @contextmenu="onMenu"
      >
        <img v-if="poster" :src="poster" alt="" loading="lazy" draggable="false" class="absolute inset-0 size-full object-cover" />
        <span v-else class="absolute inset-0 grid place-items-center p-4 text-center heading-03 text-ink-2">{{ title }}</span>
        <span v-if="fill != null || played" class="absolute inset-x-0 bottom-0 h-1 bg-line" aria-hidden="true">
          <span class="block h-full bg-red transition-[width] duration-700" :style="{ width: `${(fill ?? played ?? 0) * 100}%` }" />
        </span>
        <span v-if="tag" class="tag absolute top-2 left-2" :class="tagKind === 'new' ? 'tag-new' : 'tag-solid'">{{ tag }}</span>
      </button>
      <button
        v-if="playable"
        tabindex="-1"
        class="absolute top-1/2 left-1/2 grid size-10 -translate-x-1/2 -translate-y-1/2 place-items-center bg-red text-on-red opacity-0 transition-opacity duration-[120ms] group-hover:opacity-100 hover:bg-red-deep hover:text-[var(--brand-paper)]"
        :aria-label="`Play ${title}`"
        @click="emit('play')"
      >
        <Icon name="play" />
      </button>
    </div>
    <button tabindex="-1" class="mt-2 truncate text-left heading-compact-01 underline-offset-4 hover:underline" :title="title" @click="emit('open')">
      {{ title }}
    </button>
    <p v-if="line" class="truncate label-01 text-[13px] leading-5 text-ink-2" :title="line">{{ line }}</p>
  </li>
</template>
