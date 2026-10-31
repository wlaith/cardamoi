<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  api,
  episodeLabel,
  errorMessage,
  languageName,
  SUBTITLE_LANGUAGES,
  systemLanguage,
  type Confidence,
  type LibraryEntry,
  type PlayerState,
  type Quota,
  type RankedSubtitle,
  type Subtitle,
  type VideoEntry,
} from "../api";
import Spinner from "./ui/Spinner.vue";
import Icon from "./ui/Icon.vue";

const props = defineProps<{ entry: LibraryEntry; video: VideoEntry }>();
const emit = defineEmits<{ close: [] }>();

const saved = ref<Subtitle[]>([]);
const player = ref<PlayerState | null>(null);
const languages = ref<string[]>([]);
const language = ref("");
const results = ref<RankedSubtitle[]>([]);
const errors = ref<string[]>([]);
const quota = ref<Quota | null>(null);
const searching = ref(false);
const searched = ref(false);
const showAll = ref(false);
const panelError = ref("");
const fetching = ref<Record<string, string>>({});
const lastUsed = ref(-1);

const dialog = ref<HTMLElement>();
let unlisten: UnlistenFn | undefined;

const playing = computed(
  () =>
    player.value?.itemId === props.entry.id && player.value.fileIndex === props.video.fileIndex && isLive(player.value),
);
const subTracks = computed(() => (playing.value ? player.value!.tracks.filter((t) => t.kind === "sub") : []));
const activeTrack = computed(() => subTracks.value.find((t) => t.selected));
const otherLanguages = computed(() => SUBTITLE_LANGUAGES.filter((l) => !languages.value.includes(l)));
const shown = computed(() => (showAll.value ? results.value : results.value.slice(0, 3)));
const what = computed(() => {
  const label = episodeLabel(props.video.season, props.video.episode);
  return label ? `${props.entry.title} ${label}` : props.entry.title;
});

function isLive(p: PlayerState) {
  return p.status === "opening" || p.status === "playing";
}

const confidenceLabels: Record<Confidence, string> = {
  file: "Matches your file",
  release: "Same release",
  maybe: "May need syncing",
};

function keyOf(r: { provider: string; key: string }) {
  return `${r.provider}:${r.key}`;
}

function alreadySaved(r: RankedSubtitle) {
  return saved.value.some((s) => s.source === r.provider && s.providerKey === r.key);
}

async function loadSaved() {
  try {
    saved.value = await api.listSubtitles(props.entry.id, props.video.fileIndex);
  } catch (e) {
    panelError.value = errorMessage(e);
  }
}

async function search() {
  searching.value = true;
  searched.value = true;
  showAll.value = false;
  lastUsed.value = -1;
  try {
    const response = await api.searchSubtitles(props.entry.id, props.video.fileIndex, [language.value]);
    results.value = response.results;
    errors.value = response.errors;
    quota.value = response.quota;
  } catch (e) {
    results.value = [];
    errors.value = [errorMessage(e)];
  } finally {
    searching.value = false;
  }
}

async function use(r: RankedSubtitle, index: number) {
  const key = keyOf(r);
  fetching.value[key] = "loading";
  try {
    await api.downloadSubtitle(props.entry.id, props.video.fileIndex, r);
    fetching.value[key] = "done";
    lastUsed.value = index;
    await loadSaved();
  } catch (e) {
    fetching.value[key] = errorMessage(e);
  }
}

function tryNext() {
  const next = results.value.findIndex((r, i) => i > lastUsed.value && !alreadySaved(r));
  if (next < 0) return;
  if (next >= 3) showAll.value = true;
  use(results.value[next], next);
}

async function command(args: (string | number)[]) {
  panelError.value = "";
  try {
    await api.playerCommand(args);
  } catch (e) {
    panelError.value = errorMessage(e);
  }
}

function timing(delay: number) {
  if (Math.abs(delay) < 0.05) return "on time";
  return `${Math.abs(delay).toFixed(1)} s ${delay > 0 ? "later" : "earlier"}`;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

onMounted(async () => {
  dialog.value?.focus();
  window.addEventListener("keydown", onKey);
  unlisten = await listen<PlayerState>("player://state", (event) => (player.value = event.payload));
  try {
    player.value = await api.playerState();
    const settings = await api.subtitleSettings();
    languages.value = settings.languages.length ? settings.languages : [systemLanguage()];
  } catch {
    languages.value = [systemLanguage()];
  }
  language.value = languages.value[0];
  await loadSaved();
  if (!saved.value.some((s) => s.lang?.startsWith(language.value))) search();
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  unlisten?.();
});
</script>

<template>
  <section
    ref="dialog"
    class="hud fixed top-[62px] right-4 z-30 flex max-h-[calc(100vh-100px)] w-[420px] flex-col focus:outline-none"
    role="dialog"
    aria-labelledby="captions-heading"
    tabindex="-1"
  >
    <div class="hud-title relative flex shrink-0 items-center justify-center px-6">
      <button class="btn-icon absolute right-0 size-12" aria-label="Close" @click="emit('close')">
        <Icon name="close" />
      </button>
      <h2 id="captions-heading" class="truncate">Subtitles for {{ what }}</h2>
    </div>

    <div class="overflow-y-auto p-4">
      <p v-if="panelError" class="mb-2 text-error">{{ panelError }}</p>

      <h3 class="text-small tabular-nums font-semibold text-ink-2">On this file</h3>
      <ul v-if="playing" class="mt-1 border-y border-subtle">
        <li v-for="t in subTracks" :key="t.id">
          <button
            class="flex w-full items-center gap-2 px-1 py-1.5 text-left hover:bg-layer-hover"
            :aria-pressed="t.selected"
            @click="command(['set', 'sid', t.id])"
          >
            <span class="w-3 text-ink">{{ t.selected ? "✓" : "" }}</span>
            <span class="flex-1 truncate">
              {{ languageName(t.lang) }}
              <span v-if="t.hearingImpaired" class="text-ink-2">· CC (describes sounds)</span>
              <span class="text-ink-2">· {{ t.external ? t.title || "downloaded" : "in the video" }}</span>
            </span>
          </button>
        </li>
        <li>
          <button class="flex w-full items-center gap-2 px-1 py-1.5 text-left hover:bg-layer-hover" :aria-pressed="!activeTrack" @click="command(['set', 'sid', 'no'])">
            <span class="w-3 text-ink">{{ activeTrack ? "" : "✓" }}</span>
            Off
          </button>
        </li>
      </ul>
      <template v-else>
        <ul v-if="saved.length" class="mt-1 border-y border-subtle">
          <li v-for="s in saved" :key="s.id" class="truncate px-1 py-1.5" :title="s.label">
            {{ languageName(s.lang) }}
            <span v-if="s.hearingImpaired" class="text-ink-2">· CC (describes sounds)</span>
            <span class="text-ink-2">· {{ s.source === "torrent" ? "came with the download" : s.label }}</span>
          </li>
        </ul>
        <p class="mt-1 text-small tabular-nums text-ink-2">
          {{ saved.length ? "" : "No subtitle files yet. " }}Subtitles inside the video show up here while it plays.
        </p>
      </template>

      <div v-if="playing && activeTrack" class="mt-3 flex flex-wrap items-center gap-1.5">
        <span>Timing:</span>
        <button class="btn-hud w-6 px-0" aria-label="Show subtitles earlier" @click="command(['add', 'sub-delay', -0.5])">−</button>
        <span class="min-w-24 text-center font-semibold" aria-live="polite">{{ timing(player!.subDelay) }}</span>
        <button class="btn-hud w-6 px-0" aria-label="Show subtitles later" @click="command(['add', 'sub-delay', 0.5])">+</button>
        <button v-if="Math.abs(player!.subDelay) >= 0.05" class="btn-hud" @click="command(['set', 'sub-delay', 0])">Reset</button>
        <button v-if="results.length" class="ml-auto text-small tabular-nums text-link hover:underline" @click="tryNext">Still off? Try the next match</button>
      </div>

      <div class="mt-4 flex flex-wrap items-center gap-2">
        <h3 class="text-small tabular-nums font-semibold text-ink-2">Find more</h3>
        <select v-model="language" aria-label="Language" class="popup-hud" @change="search">
          <optgroup label="Your languages">
            <option v-for="code in languages" :key="code" :value="code">{{ languageName(code) }}</option>
          </optgroup>
          <optgroup label="Other languages">
            <option v-for="code in otherLanguages" :key="code" :value="code">{{ languageName(code) }}</option>
          </optgroup>
        </select>
        <button v-if="!searching" class="btn-hud" @click="search">{{ searched ? "Search Again" : "Search" }}</button>
      </div>

      <p v-if="searching" class="flex items-center gap-2 py-3 text-ink-2" role="status">
        <Spinner light :size="12" /> Looking for {{ languageName(language) }} subtitles
      </p>
      <template v-else-if="searched">
        <p v-for="msg in errors" :key="msg" class="mt-2 text-error">{{ msg }}</p>
        <p v-if="!results.length && !errors.length" class="py-3 text-ink-2">
          No {{ languageName(language) }} subtitles found for {{ what }}.
        </p>
        <ul v-if="results.length" class="mt-2 border-y border-subtle">
          <li v-for="(r, i) in shown" :key="keyOf(r)" class="flex items-center gap-3 border-b border-subtle py-2 last:border-b-0">
            <div class="min-w-0 flex-1">
              <p class="flex items-center gap-1.5">
                <span
                  class="tag shrink-0"
                  :class="r.confidence === 'maybe' ? '' : 'tag-green'"
                >
                  {{ confidenceLabels[r.confidence] }}
                </span>
                <span class="truncate" :title="r.release">{{ r.release || "Unnamed release" }}</span>
              </p>
              <p class="mt-0.5 flex flex-wrap gap-x-2 text-small tabular-nums text-ink-2">
                <span>{{ languageName(r.lang) }}</span>
                <span v-if="r.hearingImpaired">CC (describes sounds)</span>
                <span>{{ r.provider === "opensubtitles" ? "OpenSubtitles" : "Subdl" }}</span>
                <span v-if="r.downloads">{{ r.downloads.toLocaleString() }} downloads</span>
              </p>
              <p v-if="fetching[keyOf(r)] && !['loading', 'done'].includes(fetching[keyOf(r)])" class="mt-1 text-small tabular-nums text-error">
                {{ fetching[keyOf(r)] }}
              </p>
            </div>
            <span v-if="fetching[keyOf(r)] === 'done' || alreadySaved(r)" class="shrink-0 text-ink">✓ {{ playing ? "On" : "Saved" }}</span>
            <button v-else class="btn-hud shrink-0" :disabled="fetching[keyOf(r)] === 'loading'" @click="use(r, i)">
              {{ fetching[keyOf(r)] === "loading" ? "Loading" : "Use" }}
            </button>
          </li>
        </ul>
        <button v-if="results.length > 3" class="mt-2 text-small tabular-nums text-ink-2 hover:text-ink" :aria-expanded="showAll" @click="showAll = !showAll">
          {{ showAll ? "Show Fewer" : `Show ${results.length - 3} More` }}
        </button>
        <p v-if="quota" class="mt-3 text-small tabular-nums" :class="quota.remaining <= 3 ? 'text-error' : 'text-ink-2'">
          OpenSubtitles: {{ quota.remaining }} {{ quota.remaining === 1 ? "download" : "downloads" }} left today
        </p>
      </template>
    </div>
  </section>
</template>
