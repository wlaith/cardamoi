<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { api, episodeLabel } from "../../api";
import { stop, subtitles } from "../../commands";
import { delayed } from "../../delay";
import { controlsVisible } from "../../player/controls";
import { playerKey, type PlayerAction } from "../../player/keys";
import { formatTime } from "../../player/time";
import { trackName } from "../../player/tracks";
import { entries, player } from "../../store";
import { hud } from "../../ui";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
import Loader from "../ui/Loader.vue";
import PlayerBand from "./PlayerBand.vue";
import PlayerTracksMenu from "./PlayerTracksMenu.vue";
import PlayerUpNext from "./PlayerUpNext.vue";

const p = computed(() => player.value!);

const now = ref(Date.now());
const lastActivity = ref(Date.now());
const menuOpen = ref(false);
const onControls = ref(false);
const fullscreen = ref(false);
const notice = ref<string | null>(null);
const resumed = ref(false);

const idleMs = computed(() => now.value - lastActivity.value);
const visible = computed(() =>
  controlsVisible({
    status: p.value.status,
    paused: p.value.paused,
    buffering: p.value.buffering,
    menuOpen: menuOpen.value,
    pointerOnControls: onControls.value,
    idleMs: idleMs.value,
  }),
);
// Buffering shows only after a short wait, so a brief stall doesn't flash a card.
const buffering = delayed(() => p.value.buffering, 377);

const ep = computed(() => {
  const entry = entries.value.find((e) => e.id === p.value.itemId);
  const video = entry?.videos.find((v) => v.fileIndex === p.value.fileIndex);
  return entry && video ? { entry, video } : null;
});
const episode = computed(() => (ep.value ? episodeLabel(ep.value.video.season, ep.value.video.episode) : null));

const wake = () => (lastActivity.value = Date.now());

let ticker: number | undefined;
onMounted(() => {
  ticker = window.setInterval(() => (now.value = Date.now()), 250);
  window.addEventListener("keydown", onKey, true);
  getCurrentWindow().isFullscreen().then((on) => (fullscreen.value = on), () => {});
});
onUnmounted(() => {
  window.clearInterval(ticker);
  window.removeEventListener("keydown", onKey, true);
  window.clearTimeout(noticeTimer);
  window.clearTimeout(clickTimer);
  if (fullscreen.value) getCurrentWindow().setFullscreen(false).catch(() => {});
});

// "Resumed at" shows for a few seconds after a resumed start.
watch(
  () => `${p.value.itemId}:${p.value.fileIndex}:${p.value.resumedFrom}`,
  () => {
    resumed.value = p.value.resumedFrom != null;
    if (resumed.value) window.setTimeout(() => (resumed.value = false), 4000);
  },
  { immediate: true },
);

let noticeTimer: number | undefined;
function say(text: string) {
  notice.value = text;
  window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => (notice.value = null), 987);
}

// Keys that change something the picture doesn't show say what they did.
let armed = 0;
const arm = () => (armed = Date.now());
const selectedSubtitle = computed(() => p.value.tracks.find((t) => t.kind === "sub" && t.selected));
watch(selectedSubtitle, (t) => {
  if (Date.now() - armed < 1000) say(t ? `Subtitles: ${trackName(t)}` : "Subtitles off");
});
watch(
  () => p.value.subDelay,
  (d) => {
    if (Date.now() - armed < 1000) say(`Subtitle delay ${d >= 0 ? "+" : "−"}${Math.abs(d).toFixed(1)} s`);
  },
);

async function toggleFullscreen() {
  try {
    await getCurrentWindow().setFullscreen(!fullscreen.value);
    fullscreen.value = !fullscreen.value;
  } catch {
    /* the window stays as it was */
  }
}

function leave() {
  stop();
}

function escape() {
  // The app's own panels (the subtitle search) sit above the player and close first.
  if (hud.value) hud.value = null;
  else if (menuOpen.value) menuOpen.value = false;
  else if (p.value.nextUp) api.playerCommand(["script-message", "cardamoi-cancel"]);
  else if (fullscreen.value) toggleFullscreen();
  else leave();
}

function run(action: PlayerAction) {
  switch (action.kind) {
    case "mpv":
      if (action.notice) arm();
      api.playerCommand(action.args);
      break;
    case "fullscreen":
      toggleFullscreen();
      break;
    case "next":
      if (p.value.nextUp) api.playerCommand(["script-message", "cardamoi-next"]);
      break;
    case "escape":
      escape();
      break;
  }
}

function onKey(event: KeyboardEvent) {
  wake();
  if (event.defaultPrevented) return;
  const action = playerKey(event);
  if (!action) return;
  // Space on a focused button presses it; every other key is the player's.
  if (event.key === " " && (event.target as HTMLElement | null)?.closest("button, [role=slider]")) return;
  event.preventDefault();
  event.stopPropagation();
  run(action);
}

// A click on the picture pauses; a second one in quick succession is a double click, which goes full screen.
let clickTimer: number | undefined;
function onStageClick() {
  menuOpen.value = false;
  window.clearTimeout(clickTimer);
  clickTimer = window.setTimeout(() => api.playerCommand(["cycle", "pause"]), 233);
}
function onStageDoubleClick() {
  window.clearTimeout(clickTimer);
  toggleFullscreen();
}

function seek(secs: number, exact: boolean) {
  api.playerCommand(["seek", Math.max(0, secs), exact ? "absolute+exact" : "absolute+keyframes"]);
}

function findSubtitles() {
  menuOpen.value = false;
  if (ep.value) subtitles(ep.value);
}
</script>

<template>
  <section
    class="player"
    :class="{ quiet: !visible }"
    data-theme="ink"
    aria-label="Player"
    @pointermove="wake"
    @pointerdown="wake"
    @click="onStageClick"
    @dblclick="onStageDoubleClick"
  >
    <!-- Opening -->
    <div v-if="p.status === 'opening'" class="centre opening" @click.stop @dblclick.stop>
      <Loader :size="55" />
      <p class="headline">{{ p.title }}</p>
      <p class="muted">Starting playback</p>
      <button class="btn btn-ghost" @click="leave">Cancel <Kbd keys="Esc" /></button>
    </div>

    <!-- The last episode, or an ending with nothing after it -->
    <div v-else-if="p.status === 'ended' && !p.nextUp && p.error" class="centre card" @click.stop @dblclick.stop>
      <p>{{ p.error }}</p>
      <button class="btn btn-secondary" @click="leave">Back</button>
    </div>

    <template v-else>
      <!-- Top: back -->
      <div class="top" :class="{ hidden: !visible }" @click.stop @dblclick.stop @pointerenter="onControls = true" @pointerleave="onControls = false">
        <button class="btn btn-ghost back" @click="leave">
          <Icon name="chevron-left" :size="21" />
          <span>{{ p.title.split(" · ")[0] }}</span>
        </button>
      </div>

      <!-- Resumed -->
      <div v-if="resumed && p.resumedFrom != null" class="resumed" @click.stop @dblclick.stop>
        <span class="mono">Resumed at {{ formatTime(p.resumedFrom) }}</span>
        <button class="btn btn-ghost" @click="api.playerCommand(['seek', 0, 'absolute']); resumed = false">
          Start over <Kbd keys="Home" />
        </button>
      </div>

      <p v-if="notice" class="notice mono" aria-live="polite">{{ notice }}</p>

      <!-- Centre: transport -->
      <div class="transport" :class="{ hidden: !visible }" @click.stop @dblclick.stop @pointerenter="onControls = true" @pointerleave="onControls = false">
        <button class="square" aria-label="Back 10 seconds" @click="api.playerCommand(['seek', -10])"><Icon name="rotate-back" :size="24" /></button>
        <button class="square" :aria-label="p.paused ? 'Play' : 'Pause'" @click="api.playerCommand(['cycle', 'pause'])">
          <Icon :name="p.paused ? 'play' : 'pause'" :size="24" />
        </button>
        <button class="square" aria-label="Forward 10 seconds" @click="api.playerCommand(['seek', 10])"><Icon name="renew" :size="24" /></button>
      </div>

      <div v-if="buffering" class="centre card" aria-live="polite" @click.stop>
        <p class="mono">Buffering<span v-if="p.bufferPercent != null"> {{ Math.round(p.bufferPercent) }} %</span></p>
      </div>

      <PlayerUpNext v-if="p.nextUp" class="up-next" :title="p.nextUp.title" :seconds-left="p.nextUp.secondsLeft" @click.stop @dblclick.stop />

      <div v-if="menuOpen" class="menu" @click.stop @dblclick.stop @pointerenter="onControls = true" @pointerleave="onControls = false">
        <PlayerTracksMenu :tracks="p.tracks" @find-subtitles="findSubtitles" />
      </div>

      <!-- Bottom -->
      <div class="bottom" :class="{ hidden: !visible }" @click.stop @dblclick.stop @pointerenter="onControls = true" @pointerleave="onControls = false">
        <PlayerBand
          :p="p"
          :episode="episode"
          :fullscreen="fullscreen"
          :menu-open="menuOpen"
          @seek="seek"
          @toggle-fullscreen="toggleFullscreen"
          @toggle-menu="menuOpen = !menuOpen"
        />
      </div>
    </template>
  </section>
</template>

<style scoped>
/* Under the app's own panels (z-30), over everything else. The page is transparent, so the picture shows through. */
.player {
  position: fixed;
  inset: 0;
  z-index: 20;
  color: var(--c-ink);
  user-select: none;
}
.player.quiet {
  cursor: none;
}
.top,
.transport,
.bottom {
  transition: opacity var(--dur-3) var(--ease-out);
}
.hidden {
  opacity: 0;
  pointer-events: none;
}
.top {
  position: absolute;
  top: 21px;
  left: 21px;
}
.back {
  height: 40px;
  gap: 5px;
  padding: 0 13px 0 8px;
  background: color-mix(in srgb, var(--c-surface) 88%, transparent);
  border: 1px solid var(--c-line);
}
.transport {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 34px;
  pointer-events: none;
}
.transport > * {
  pointer-events: auto;
}
.transport.hidden > * {
  pointer-events: none;
}
.square {
  display: grid;
  width: 55px;
  height: 55px;
  place-items: center;
  color: var(--c-ink);
  background: color-mix(in srgb, var(--c-surface) 88%, transparent);
  border: 1px solid var(--c-line);
  transition: background var(--dur-1) var(--ease-out), scale var(--dur-2) var(--ease-spring);
}
.square:hover {
  background: var(--c-surface-raised);
}
.square:active {
  scale: 0.94;
}
.bottom {
  position: absolute;
  inset: auto 0 0 0;
}
.menu {
  position: absolute;
  right: 34px;
  bottom: 110px;
}
.up-next {
  position: absolute;
  right: 34px;
  bottom: 144px;
}
.centre {
  position: absolute;
  top: 50%;
  left: 50%;
  translate: -50% -50%;
}
.opening {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 13px;
  padding: 34px;
  background: color-mix(in srgb, var(--c-surface) 88%, transparent);
}
.headline {
  font-size: 24px;
  line-height: 28px;
  font-weight: 600;
}
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 13px;
  padding: 21px 34px;
  background: var(--c-surface-raised);
  border: 1px solid var(--c-line);
}
.muted {
  color: var(--c-ink-muted);
}
.mono {
  font-family: var(--font-mono);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}
.resumed {
  position: absolute;
  top: 21px;
  left: 50%;
  translate: -50% 0;
  display: flex;
  align-items: center;
  gap: 13px;
  padding: 0 8px 0 21px;
  height: 40px;
  background: color-mix(in srgb, var(--c-surface) 88%, transparent);
  border: 1px solid var(--c-line);
}
.notice {
  position: absolute;
  top: 76px;
  left: 50%;
  translate: -50% 0;
  padding: 8px 13px;
  background: var(--c-surface-raised);
  border: 1px solid var(--c-line);
}
</style>
