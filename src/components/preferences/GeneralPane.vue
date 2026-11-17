<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { api, formatBytes, type TrashRule } from "../../api";
import { checkFolder, openFolder } from "../../commands";
import { theme, type Theme } from "../../store";
import {
  checkError,
  checkForUpdate,
  checkState,
  currentVersion,
  installing,
  installUpdate,
  openAllReleases,
  openRelease,
  readyUpdate,
} from "../../updates";
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

const afterWatching: { id: TrashRule["afterWatching"]; label: string; icon: "checkmark" | "trash" | "recently-viewed" }[] = [
  { id: "keep", label: "Keep it", icon: "checkmark" },
  { id: "now", label: "Trash right away", icon: "trash" },
  { id: "countdown", label: "Trash after a countdown", icon: "recently-viewed" },
];
const rule = ref<TrashRule | null>(null);
const backlog = ref<{ count: number; bytes: number } | null>(null);
const ruleError = ref<unknown>(null);

async function saveRule() {
  try {
    await api.saveTrashRule(rule.value!);
    ruleError.value = null;
  } catch (e) {
    ruleError.value = e;
  }
}

async function chooseRule(id: TrashRule["afterWatching"]) {
  const wasKeep = rule.value!.afterWatching === "keep";
  rule.value!.afterWatching = id;
  await saveRule();
  backlog.value = null;
  if (wasKeep && id !== "keep") {
    const found = await api.trashBacklog().catch(() => null);
    if (found?.count) backlog.value = found;
  }
}

function stepDays(by: number) {
  rule.value!.days = Math.min(365, Math.max(1, rule.value!.days + by));
  saveRule();
}

async function trashBacklog() {
  backlog.value = null;
  try {
    await api.scheduleBacklog();
  } catch (e) {
    ruleError.value = e;
  }
}

const autoplayNext = ref<boolean | null>(null);
const ownMpv = ref(false);
const error = ref<unknown>(null);

const seed = ref<boolean | null>(null);
const seedError = ref<unknown>(null);

async function saveSeed() {
  try {
    await api.setSeeding(seed.value!);
    seedError.value = null;
  } catch (e) {
    seedError.value = e;
    seed.value = !seed.value;
  }
}

const magnetIsUs = ref<boolean | null>(null);
const magnetSettingsOpened = ref(false);
const magnetError = ref<unknown>(null);

async function checkMagnet() {
  try {
    magnetIsUs.value = await api.magnetHandlerIsUs();
    if (magnetIsUs.value) magnetSettingsOpened.value = false;
  } catch (e) {
    magnetError.value = e;
  }
}

async function useForMagnets() {
  try {
    magnetSettingsOpened.value = await api.makeMagnetHandler();
    magnetError.value = null;
    await checkMagnet();
  } catch (e) {
    magnetError.value = e;
  }
}

// Focus picks up a change made in the system's own settings window.
onUnmounted(() => window.removeEventListener("focus", checkMagnet));
onMounted(async () => {
  window.addEventListener("focus", checkMagnet);
  checkMagnet();
  api.trashRule().then((r) => (rule.value = r), (e) => (ruleError.value = e));
  api.seeding().then((on) => (seed.value = on), (e) => (seedError.value = e));
  try {
    const settings = await api.playbackSettings();
    autoplayNext.value = settings.autoplayNext;
    ownMpv.value = settings.ownMpv;
  } catch (e) {
    error.value = e;
  }
});

async function save(changed: "autoplayNext" | "ownMpv") {
  if (autoplayNext.value == null) return;
  try {
    await api.savePlaybackSettings({ autoplayNext: autoplayNext.value, ownMpv: ownMpv.value });
    error.value = null;
  } catch (e) {
    error.value = e;
    if (changed === "autoplayNext") autoplayNext.value = !autoplayNext.value;
    else ownMpv.value = !ownMpv.value;
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

    <section class="space-y-21" aria-labelledby="magnet-heading">
      <SectionHeader
        id="magnet-heading"
        title="Magnet links"
        subtitle="Clicking a magnet link in your browser opens it here, ready to download."
      />
      <p v-if="magnetIsUs">Cardamoi opens magnet links.</p>
      <div v-else-if="magnetIsUs === false" class="space-y-8">
        <button class="btn btn-secondary" @click="useForMagnets">
          Use Cardamoi for magnet links <Icon name="magnet" />
        </button>
        <p v-if="magnetSettingsOpened" class="text-ink-2">
          Windows needs you to finish this in Settings: choose Cardamoi for MAGNET in the window that opened.
        </p>
      </div>
      <ErrorNote v-if="magnetError" :error="magnetError" />
    </section>

    <section class="space-y-21" aria-labelledby="updates-heading">
      <SectionHeader
        id="updates-heading"
        title="Updates"
        subtitle="Cardamoi checks for a new version when it opens and every few hours, and downloads it in the background."
      />
      <div v-if="readyUpdate" class="space-y-8">
        <p>
          Cardamoi {{ readyUpdate.version }} is ready.
          <button type="button" class="link inline-flex items-center gap-1" @click="openRelease(readyUpdate.version)">
            What's new <Icon name="external-link" />
          </button>
        </p>
        <button class="btn btn-secondary" :disabled="installing" @click="installUpdate">
          {{ installing ? "Installing…" : "Restart to Update" }} <Icon name="renew" />
        </button>
      </div>
      <div v-else class="space-y-8">
        <p>
          <template v-if="checkState === 'upToDate'">You're on the latest version, </template>
          <template v-else>You're on </template>
          <button v-if="currentVersion" type="button" class="link" @click="openRelease(currentVersion)">
            Cardamoi {{ currentVersion }}</button
          >.
        </p>
        <button class="btn btn-ghost" :disabled="checkState === 'checking'" @click="checkForUpdate">
          {{ checkState === "checking" ? "Checking…" : "Check for updates" }} <Icon name="renew" />
        </button>
        <p v-if="checkState === 'dev'" class="text-ink-2">Dev builds don't update themselves.</p>
      </div>
      <p class="text-ink-2">
        Every version and its release notes are on GitHub.
        <button type="button" class="link inline-flex items-center gap-1" @click="openAllReleases">
          See all releases <Icon name="external-link" />
        </button>
      </p>
      <ErrorNote v-if="checkState === 'failed'" :error="`Couldn't check for updates: ${checkError}`" />
    </section>

    <section class="space-y-21" aria-labelledby="storage-heading">
      <SectionHeader id="storage-heading" title="Storage" subtitle="Media in the Trash uses disk space until you empty it." />
      <div v-if="rule" class="space-y-13">
        <p id="after-watching" class="font-semibold">After I finish something</p>
        <div class="flex flex-wrap gap-2" role="group" aria-labelledby="after-watching">
          <button
            v-for="a in afterWatching"
            :key="a.id"
            class="rule-choice"
            :aria-pressed="rule.afterWatching === a.id"
            @click="chooseRule(a.id)"
          >
            <Icon :name="a.icon" /> {{ a.label }}
          </button>
        </div>
        <div v-if="rule.afterWatching === 'countdown'" class="flex items-center gap-2">
          <div class="stepper tabular-nums" role="group" aria-label="Days">
            <button class="btn-icon" aria-label="Fewer days" @click="stepDays(-1)"><Icon name="subtract" /></button>
            <span>{{ rule.days }} {{ rule.days === 1 ? "day" : "days" }}</span>
            <button class="btn-icon" aria-label="More days" @click="stepDays(1)"><Icon name="add" /></button>
          </div>
          <span class="text-ink-2">after I finish it</span>
        </div>
        <div v-if="backlog" class="space-y-13 border border-line bg-background p-21" role="group" aria-labelledby="backlog-title">
          <div>
            <p id="backlog-title" class="font-semibold">
              Also trash the <span class="tabular-nums">{{ backlog.count }}</span> you've already finished?
            </p>
            <p class="text-ink-2">They take <span class="tabular-nums">{{ formatBytes(backlog.bytes) }}</span>.</p>
          </div>
          <div class="flex gap-2">
            <button class="btn btn-primary" @click="trashBacklog">Trash them too <Icon name="trash" /></button>
            <button class="btn btn-secondary" @click="backlog = null">Only new ones</button>
          </div>
        </div>
      </div>
      <ErrorNote v-if="ruleError" :error="ruleError" />
    </section>

    <section class="space-y-21" aria-labelledby="downloads-heading">
      <SectionHeader id="downloads-heading" title="Downloads" />
      <label v-if="seed != null" class="flex items-start gap-4">
        <input v-model="seed" type="checkbox" class="toggle" @change="saveSeed" />
        <span>
          Seed finished downloads
          <span class="mt-1 block text-ink-2">
            Keeps uploading finished torrents to other people while cardamoi is open. Seeding uses network, memory and
            battery.
          </span>
        </span>
      </label>
      <ErrorNote v-if="seedError" :error="seedError" />
    </section>

    <section class="space-y-21" aria-labelledby="playback-heading">
      <SectionHeader id="playback-heading" title="Playback" />
      <label v-if="autoplayNext != null" class="flex items-start gap-4">
        <input v-model="autoplayNext" type="checkbox" class="toggle" @change="save('autoplayNext')" />
        <span>
          Play the next episode automatically
          <span class="mt-1 block text-ink-2">
            When an episode ends and the next one is downloaded, it starts after 10 seconds. Press Esc to stay, or Return
            to start it straight away.
          </span>
        </span>
      </label>
      <label v-if="autoplayNext != null" class="flex items-start gap-4">
        <input v-model="ownMpv" type="checkbox" class="toggle" @change="save('ownMpv')" />
        <span>
          Use my own mpv
          <span class="mt-1 block text-ink-2">
            Plays with the mpv installed on this computer, with your mpv.conf and scripts, in its own window. Off,
            cardamoi plays videos itself and needs nothing installed.
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
.rule-choice {
  display: inline-flex;
  height: 40px;
  align-items: center;
  gap: 8px;
  padding: 0 13px;
  border: 1px solid var(--c-line);
  color: var(--c-ink-muted);
}
.rule-choice:hover {
  color: var(--c-ink);
}
.rule-choice[aria-pressed="true"] {
  color: var(--c-surface);
  background: var(--c-ink);
  border-color: var(--c-ink);
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
