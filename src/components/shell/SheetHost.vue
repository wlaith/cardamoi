<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { closeSheet, sheet } from "../../ui";
import Icon from "../ui/Icon.vue";
import InfoSheet from "./InfoSheet.vue";
import MagnetSheet from "./MagnetSheet.vue";

const panel = ref<HTMLElement>();
const showDetails = ref(false);
let returnFocus: HTMLElement | null = null;

watch(
  () => sheet.value?.kind,
  async (kind, was) => {
    showDetails.value = false;
    if (kind && !was) returnFocus = document.activeElement as HTMLElement | null;
    if (!kind) {
      returnFocus?.focus();
      return;
    }
    await nextTick();
    const target =
      panel.value?.querySelector<HTMLElement>("input, textarea, select") ??
      [...(panel.value?.querySelectorAll<HTMLElement>(".modal-footer button") ?? [])].pop();
    target?.focus();
  },
);

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    closeSheet();
    return;
  }
  if (event.key !== "Tab" || !panel.value) return;
  const focusable = [...panel.value.querySelectorAll<HTMLElement>("button, input, select, textarea, a[href]")].filter(
    (el) => !el.hasAttribute("disabled"),
  );
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
</script>

<template>
  <div v-if="sheet" class="scrim fixed inset-0 z-40 grid place-items-center p-6" @keydown="onKey" @mousedown.self="closeSheet()">
    <div
      ref="panel"
      class="modal relative w-[560px] max-w-full"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="sheet.kind === 'alert' ? 'alert-title' : undefined"
    >
      <button class="btn-icon absolute top-13 right-13 z-10" aria-label="Close" @click="closeSheet()"><Icon name="close" :size="20" /></button>
      <template v-if="sheet.kind === 'alert'">
        <div class="px-34 pt-34 pr-89">
          <p v-if="sheet.buttons.length > 1" class="eyebrow">{{ sheet.buttons[sheet.buttons.length - 1].label }}</p>
          <h2 id="alert-title" class="mt-2 text-heading">{{ sheet.title }}</h2>
        </div>
        <div class="px-34 pt-13 pr-[20%]">
          <p class="selectable text-ink-2">{{ sheet.message }}</p>
          <template v-if="sheet.details">
            <button class="mt-4 flex items-center gap-1 text-small tabular-nums text-link" :aria-expanded="showDetails" @click="showDetails = !showDetails">
              <Icon name="chevron-right" :size="12" :class="{ 'rotate-90': showDetails }" />
              Details
            </button>
            <p v-if="showDetails" class="mt-2 max-h-32 overflow-y-auto bg-background p-2 text-small break-all text-ink-2 selectable">
              {{ sheet.details }}
            </p>
          </template>
        </div>
        <div class="modal-footer">
          <button
            v-for="b in sheet.buttons"
            :key="b.value"
            class="btn"
            :class="b.kind === 'primary' ? 'btn-primary' : b.kind === 'danger' ? 'btn-danger' : 'btn-secondary'"
            @click="closeSheet(b.value)"
          >
            {{ b.label }}
          </button>
        </div>
      </template>
      <InfoSheet v-else-if="sheet.kind === 'info'" :ep="sheet.ep" />
      <MagnetSheet v-else-if="sheet.kind === 'magnet'" :link="sheet.link" />
    </div>
  </div>
</template>
