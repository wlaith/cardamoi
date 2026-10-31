<script setup lang="ts" generic="T extends string | number">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    items: { id: T; label: string; count?: number; title?: string }[];
    label: string;
    nav?: boolean;
  }>(),
  { nav: false },
);
const model = defineModel<T | null>({ required: true });

const list = ref<HTMLElement>();
const bar = ref({ x: 0, width: 0 });
const placed = ref(false);

function measure() {
  const el = list.value?.querySelector<HTMLElement>(`[data-tab="${model.value}"]`);
  bar.value = el ? { x: el.offsetLeft, width: el.offsetWidth } : { x: bar.value.x, width: 0 };
  if (el && !placed.value) requestAnimationFrame(() => (placed.value = true));
}

let observer: ResizeObserver | undefined;
onMounted(() => {
  measure();
  observer = new ResizeObserver(measure);
  observer.observe(list.value!);
});
onBeforeUnmount(() => observer?.disconnect());
watch([model, () => props.items], () => nextTick(measure), { deep: true });

function onKey(event: KeyboardEvent) {
  if (props.nav) return;
  const at = props.items.findIndex((t) => t.id === model.value);
  const to = { ArrowLeft: at - 1, ArrowRight: at + 1, Home: 0, End: props.items.length - 1 }[event.key];
  if (to == null) return;
  event.preventDefault();
  const next = props.items[(to + props.items.length) % props.items.length];
  model.value = next.id;
  nextTick(() => list.value?.querySelector<HTMLElement>(`[data-tab="${next.id}"]`)?.focus());
}
</script>

<template>
  <div ref="list" class="tabs" :class="{ 'tabs-nav': nav }" :role="nav ? undefined : 'tablist'" :aria-label="label" @keydown="onKey">
    <button
      v-for="t in items"
      :key="t.id"
      :data-tab="t.id"
      class="tab"
      :role="nav ? undefined : 'tab'"
      :aria-selected="nav ? undefined : model === t.id"
      :aria-current="nav && model === t.id ? 'page' : undefined"
      :tabindex="nav || model === t.id ? 0 : -1"
      :title="t.title"
      @click="model = t.id"
    >
      {{ t.label }}<span v-if="t.count != null" class="tab-count">{{ t.count }}</span>
    </button>
    <span
      class="tab-indicator"
      :class="{ placed }"
      :style="{ width: `${bar.width}px`, transform: `translateX(${bar.x}px)` }"
      aria-hidden="true"
    />
  </div>
</template>
