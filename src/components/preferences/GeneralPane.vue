<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "../../api";
import { checkFolder, openFolder } from "../../commands";
import { theme, type Theme } from "../../store";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import SectionHeader from "../ui/SectionHeader.vue";

const themes: { id: Theme; label: string; ground: string; accent: string; ink: string }[] = [
  { id: "gray", label: "Gray", ground: "#f5f5f7", accent: "#1d1d1f", ink: "#1d1d1f" },
  { id: "ink", label: "Ink", ground: "#131313", accent: "#ff2d1a", ink: "#eeebe4" },
  { id: "paper", label: "Paper", ground: "#f1eee8", accent: "#ff2d1a", ink: "#141414" },
];

function onThemeKey(event: KeyboardEvent) {
  const step = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key];
  if (!step) return;
  event.preventDefault();
  const at = themes.findIndex((t) => t.id === theme.value);
  theme.value = themes[(at + step + themes.length) % themes.length].id;
  (event.currentTarget as HTMLElement).querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
}

const autoplayNext = ref<boolean | null>(null);
const error = ref<unknown>(null);

onMounted(async () => {
  try {
    autoplayNext.value = (await api.playbackSettings()).autoplayNext;
  } catch (e) {
    error.value = e;
  }
});

async function save() {
  if (autoplayNext.value == null) return;
  try {
    await api.savePlaybackSettings({ autoplayNext: autoplayNext.value });
    error.value = null;
  } catch (e) {
    error.value = e;
    autoplayNext.value = !autoplayNext.value;
  }
}
</script>

<template>
  <div class="max-w-[640px] space-y-55">
    <section class="space-y-21" aria-labelledby="appearance-heading">
      <SectionHeader id="appearance-heading" title="Appearance" subtitle="Changes as soon as you choose." />
      <div class="flex gap-13" role="radiogroup" aria-labelledby="appearance-heading" @keydown="onThemeKey">
        <button
          v-for="t in themes"
          :key="t.id"
          class="theme-choice"
          role="radio"
          :aria-checked="theme === t.id"
          :tabindex="theme === t.id ? 0 : -1"
          @click="theme = t.id"
        >
          <span class="theme-swatch" :style="{ background: t.ground, borderColor: t.ink }" aria-hidden="true">
            <span :style="{ background: t.accent, borderColor: t.ink }" />
          </span>
          {{ t.label }}
        </button>
      </div>
    </section>

    <section class="space-y-21" aria-labelledby="folder-heading">
      <SectionHeader
        id="folder-heading"
        title="Library folder"
        subtitle="Videos you put in the cardamoi folder in Movies show up in your library within a minute."
      />
      <div class="flex gap-4">
        <button class="btn btn-secondary" @click="openFolder">Open folder <Icon name="folder" /></button>
        <button class="btn btn-ghost" @click="checkFolder">Check for new videos <Icon name="renew" /></button>
      </div>
    </section>

    <section class="space-y-21" aria-labelledby="playback-heading">
      <SectionHeader id="playback-heading" title="Playback" />
      <label v-if="autoplayNext != null" class="flex items-start gap-4">
        <input v-model="autoplayNext" type="checkbox" class="toggle" @change="save" />
        <span>
          Play the next episode automatically
          <span class="mt-1 block text-ink-2">
            When an episode ends and the next one is downloaded, it starts after 10 seconds. Press Esc to stay, or Return
            to start it straight away.
          </span>
        </span>
      </label>
      <ErrorNote v-if="error" :error="error" />
    </section>
  </div>
</template>

<style scoped>
.theme-choice {
  display: flex;
  width: 144px;
  flex-direction: column;
  gap: 8px;
  padding: 8px 8px 13px;
  text-align: left;
  border: 1px solid var(--c-line);
  transition:
    border-color var(--dur-1) var(--ease-out),
    box-shadow var(--dur-1) var(--ease-out);
}
.theme-choice:hover {
  border-color: var(--c-ink-muted);
}
.theme-choice[aria-checked="true"] {
  font-weight: 600;
  border-color: var(--c-ink);
  box-shadow: inset 0 0 0 1px var(--c-ink);
}
.theme-swatch {
  display: grid;
  height: 55px;
  place-items: end start;
  padding: 8px;
  border: 1px solid;
}
.theme-swatch > span {
  width: 34px;
  height: 13px;
  border: 1px solid;
}
</style>
