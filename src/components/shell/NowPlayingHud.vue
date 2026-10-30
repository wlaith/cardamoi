<script setup lang="ts">
import { computed } from "vue";
import { subtitles, stop, togglePause } from "../../commands";
import { entries, player } from "../../store";
import { hud } from "../../ui";
import Icon from "../ui/Icon.vue";

const ep = computed(() => {
  const p = player.value;
  const entry = p && entries.value.find((e) => e.id === p.itemId);
  const video = entry?.videos.find((v) => v.fileIndex === p!.fileIndex);
  return entry && video ? { entry, video } : null;
});
</script>

<template>
  <section class="hud fixed top-14 right-24 z-30 w-[360px]" aria-labelledby="now-playing-title">
    <div class="hud-title flex items-center pl-4">
      <h2 id="now-playing-title" class="flex-1">Now playing</h2>
      <button class="btn-icon size-12" aria-label="Close" @click="hud = null"><Icon name="close" /></button>
    </div>
    <div v-if="player" class="p-4">
      <p class="truncate">{{ player.title }}</p>
      <div class="mt-4 flex gap-2">
        <button class="btn btn-primary btn-sm flex-1" @click="togglePause">
          {{ player.paused ? "Resume" : "Pause" }} <Icon :name="player.paused ? 'play' : 'pause'" />
        </button>
        <button class="btn-hud" @click="stop(); hud = null"><Icon name="stop" /> Stop</button>
        <button class="btn-hud" :disabled="!ep" @click="ep && subtitles(ep)"><Icon name="captions" /> Subtitles</button>
      </div>
    </div>
  </section>
</template>
