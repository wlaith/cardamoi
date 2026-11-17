<script setup lang="ts">
import { computed } from "vue";
import { api, type PlayerState } from "../../api";
import Icon from "../ui/Icon.vue";
import PlayerScrubber from "./PlayerScrubber.vue";

const props = defineProps<{ p: PlayerState; episode: string | null; fullscreen: boolean; menuOpen: boolean }>();
const emit = defineEmits<{ seek: [secs: number, exact: boolean]; toggleFullscreen: []; toggleMenu: [] }>();

const volume = computed({
  get: () => (props.p.muted ? 0 : props.p.volume),
  set: (v: number) => api.playerCommand(["set", "volume", v]),
});

const title = computed(() => props.p.title.split(" · ")[0]);
</script>

<template>
  <div class="band">
    <div class="row">
      <p class="what">
        <span class="name">{{ title }}</span>
        <span v-if="episode" class="meta">{{ episode }}</span>
      </p>
      <div class="tools">
        <button
          class="btn-icon"
          :class="{ on: menuOpen }"
          aria-label="Subtitles and audio"
          aria-haspopup="menu"
          :aria-expanded="menuOpen"
          @click="emit('toggleMenu')"
        >
          <Icon name="captions" />
        </button>
        <button class="btn-icon" :aria-label="p.muted ? 'Unmute' : 'Mute'" @click="api.playerCommand(['cycle', 'mute'])">
          <Icon :name="p.muted || p.volume === 0 ? 'volume-off' : 'volume'" />
        </button>
        <input v-model.number="volume" class="volume" type="range" min="0" max="100" step="5" aria-label="Volume" />
        <button class="btn-icon" :aria-label="fullscreen ? 'Exit full screen' : 'Full screen'" @click="emit('toggleFullscreen')">
          <Icon :name="fullscreen ? 'minimize' : 'maximize'" />
        </button>
      </div>
    </div>
    <PlayerScrubber :position="p.position" :duration="p.duration" :cache-until="p.cacheUntil" @seek="(t, exact) => emit('seek', t, exact)" />
  </div>
</template>

<style scoped>
.band {
  display: flex;
  flex-direction: column;
  gap: 13px;
  padding: 21px 34px;
  background: color-mix(in srgb, var(--c-surface) 88%, transparent);
  border-top: 1px solid var(--c-line);
}
.row {
  display: flex;
  height: 40px;
  align-items: center;
  gap: 13px;
}
.what {
  display: flex;
  min-width: 0;
  flex: 1;
  align-items: baseline;
  gap: 13px;
}
.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
  font-weight: 600;
}
.meta {
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--c-ink-muted);
}
.tools {
  display: flex;
  align-items: center;
  gap: 5px;
}
.btn-icon.on {
  background: var(--c-surface-raised);
  border-bottom: 2px solid var(--c-ink);
}
.volume {
  width: 89px;
  accent-color: var(--c-ink);
}
</style>
