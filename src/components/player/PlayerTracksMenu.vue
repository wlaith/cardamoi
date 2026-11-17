<script setup lang="ts">
import { computed } from "vue";
import { api, type Track } from "../../api";
import { trackName as name } from "../../player/tracks";
import Icon from "../ui/Icon.vue";

const props = defineProps<{ tracks: Track[] }>();
const emit = defineEmits<{ findSubtitles: [] }>();

const subtitles = computed(() => props.tracks.filter((t) => t.kind === "sub"));
const audio = computed(() => props.tracks.filter((t) => t.kind === "audio"));
const subtitlesOff = computed(() => !subtitles.value.some((t) => t.selected));

// A change keeps the menu open, so the next track is one click away.
const pickSubtitle = (t: Track | null) => api.playerCommand(["set", "sid", t ? t.id : "no"]);
const pickAudio = (t: Track) => api.playerCommand(["set", "aid", t.id]);
</script>

<template>
  <div class="menu" role="menu" aria-label="Subtitles and audio">
    <p class="head">Subtitles</p>
    <button class="row" role="menuitemradio" :aria-checked="subtitlesOff" @click="pickSubtitle(null)">
      <span class="check"><Icon v-if="subtitlesOff" name="checkmark" /></span>
      <span class="name">Off</span>
    </button>
    <button v-for="t in subtitles" :key="`s${t.id}`" class="row" role="menuitemradio" :aria-checked="t.selected" @click="pickSubtitle(t)">
      <span class="check"><Icon v-if="t.selected" name="checkmark" /></span>
      <span class="name">{{ name(t) }}</span>
      <span v-if="t.hearingImpaired" class="tag">SDH</span>
      <span v-if="t.forced" class="tag">Forced</span>
    </button>
    <template v-if="audio.length > 1">
      <p class="head">Audio</p>
      <button v-for="t in audio" :key="`a${t.id}`" class="row" role="menuitemradio" :aria-checked="t.selected" @click="pickAudio(t)">
        <span class="check"><Icon v-if="t.selected" name="checkmark" /></span>
        <span class="name">{{ name(t) }}</span>
      </button>
    </template>
    <button class="row find" role="menuitem" @click="emit('findSubtitles')">
      <span class="check"><Icon name="search" /></span>
      <span class="name">Find subtitles…</span>
    </button>
  </div>
</template>

<style scoped>
.menu {
  display: flex;
  width: 280px;
  max-height: 377px;
  flex-direction: column;
  overflow-y: auto;
  color: var(--c-ink);
  background: var(--c-surface-raised);
  border: 1px solid var(--c-line);
}
.head {
  padding: 13px 13px 5px;
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--c-ink-muted);
}
.row {
  display: flex;
  height: 34px;
  flex-shrink: 0;
  align-items: center;
  gap: 8px;
  padding: 0 13px;
  text-align: left;
  font-size: 14px;
}
.row:hover {
  background: var(--c-red-tint);
}
.row[aria-checked="true"] .name {
  font-weight: 600;
}
.check {
  display: grid;
  width: 16px;
  place-items: center;
}
.name {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tag {
  padding: 0 5px;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--c-ink-muted);
  border: 1px solid var(--c-line);
  border-radius: 2px;
}
.find {
  margin-top: 5px;
  border-top: 1px solid var(--c-line);
}
</style>
