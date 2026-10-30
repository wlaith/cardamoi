<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { contextMenu, type MenuEntry } from "../../ui";

const menu = ref<HTMLElement>();
const position = ref({ left: 0, top: 0 });

watch(contextMenu, async (open) => {
  if (!open) return;
  position.value = { left: open.x, top: open.y };
  await nextTick();
  const box = menu.value!.getBoundingClientRect();
  position.value = {
    left: Math.min(open.x, window.innerWidth - box.width - 4),
    top: open.above ? open.y - box.height : Math.min(open.y, window.innerHeight - box.height - 4),
  };
  menu.value!.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
});

function close() {
  contextMenu.value = null;
}

function choose(item: MenuEntry) {
  if (item === "separator" || item.disabled) return;
  close();
  item.action();
}

function onKey(event: KeyboardEvent) {
  const items = [...menu.value!.querySelectorAll<HTMLElement>("button:not(:disabled)")];
  const at = items.indexOf(document.activeElement as HTMLElement);
  if (event.key === "Escape") close();
  else if (event.key === "ArrowDown") items[(at + 1) % items.length]?.focus();
  else if (event.key === "ArrowUp") items[(at - 1 + items.length) % items.length]?.focus();
  else return;
  event.preventDefault();
}
</script>

<template>
  <div v-if="contextMenu" class="fixed inset-0 z-50" @mousedown.self="close" @contextmenu.prevent="close">
    <div
      ref="menu"
      role="menu"
      class="menu fixed min-w-56"
      :style="{ left: `${position.left}px`, top: `${position.top}px` }"
      @keydown="onKey"
    >
      <template v-for="(item, i) in contextMenu.items" :key="i">
        <div v-if="item === 'separator'" class="menu-separator" role="separator" />
        <button v-else role="menuitem" class="menu-item" :class="{ danger: item.danger }" :disabled="item.disabled" @click="choose(item)">
          {{ item.label }}
        </button>
      </template>
    </div>
  </div>
</template>
