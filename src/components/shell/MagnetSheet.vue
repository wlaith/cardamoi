<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { api, episodeLabel, magnetName, type CatalogTitle, type NameLookup } from "../../api";
import { addMagnet } from "../../commands";
import { arrowNav } from "../../keys";
import { go } from "../../nav";
import { tmdbBlocked } from "../../tmdb";
import { closeSheet } from "../../ui";

const props = defineProps<{ link: string }>();
const link = ref(props.link);
const adding = ref(false);
const error = ref("");

const valid = computed(() => /^magnet:\?.*xt=urn:bt[im]h:/i.test(link.value.trim()));
const name = computed(() => (valid.value ? magnetName(link.value.trim()) : ""));
const hint = computed(() => {
  if (!link.value.trim()) return "";
  if (!link.value.trim().startsWith("magnet:")) return "A magnet link starts with magnet:?";
  if (!valid.value) return "This magnet link is missing its torrent hash (xt=urn:btih:…).";
  return name.value ? `"${name.value}"` : "";
});

// What the name is on TMDB. `chosen` starts as the automatic match and changes when the user picks another.
const lookup = ref<NameLookup | null>(null);
const chosen = ref<CatalogTitle | null>(null);
const looking = ref(false);
const picking = ref(false);
let lookupTicket = 0;
let lookupTimer: number | undefined;

watch(
  name,
  (text) => {
    clearTimeout(lookupTimer);
    const ticket = ++lookupTicket;
    lookup.value = null;
    chosen.value = null;
    picking.value = false;
    looking.value = !!text && !tmdbBlocked.value;
    if (!text || tmdbBlocked.value) return;
    lookupTimer = window.setTimeout(async () => {
      // A failed lookup only means no cover yet; the download still gets matched once it starts.
      const found = await api.matchName(text).catch(() => null);
      if (ticket !== lookupTicket) return;
      lookup.value = found;
      chosen.value = found?.found ?? null;
      looking.value = false;
    }, 300);
  },
  { immediate: true },
);

const query = ref("");
const results = ref<CatalogTitle[]>([]);
const searching = ref(false);
const searchField = ref<HTMLInputElement | null>(null);
let searchTicket = 0;
let searchTimer: number | undefined;

async function startPicking() {
  query.value = lookup.value?.query || name.value;
  picking.value = true;
  await nextTick();
  searchField.value?.select();
}

watch(query, (text) => {
  clearTimeout(searchTimer);
  const ticket = ++searchTicket;
  if (!picking.value || !text.trim()) {
    results.value = [];
    searching.value = false;
    return;
  }
  searching.value = true;
  searchTimer = window.setTimeout(async () => {
    const found = await api.catalogSearch(text.trim()).catch(() => []);
    if (ticket !== searchTicket) return;
    results.value = found;
    searching.value = false;
  }, 300);
});

function pick(title: CatalogTitle | null) {
  chosen.value = title;
  picking.value = false;
}

const kindLabel = (t: CatalogTitle) => (t.kind === "tv" ? "Show" : "Film");
const details = computed(() => {
  const t = chosen.value;
  if (!t) return "";
  const at = t.kind === "tv" ? episodeLabel(lookup.value?.season ?? null, lookup.value?.episode ?? null) : "";
  return [t.year, kindLabel(t), at].filter(Boolean).join(" · ");
});

async function add() {
  if (!valid.value) return;
  adding.value = true;
  error.value = (await addMagnet(link.value.trim(), chosen.value, lookup.value)) ?? "";
  adding.value = false;
  if (!error.value) closeSheet();
}
</script>

<template>
  <form @submit.prevent="add">
    <div class="px-4 pt-4 pr-16">
      <p class="text-small tabular-nums text-ink-2">Add to library</p>
      <h2 class="mt-1 text-heading">Add a magnet link</h2>
    </div>
    <div class="px-4 pt-6">
      <p>It downloads into your library, with the cover shown below.</p>
      <label for="magnet-link" class="field-label mt-6">Magnet link</label>
      <input
        id="magnet-link"
        v-model="link"
        class="field"
        placeholder="magnet:?xt=urn:btih:…"
        autocomplete="off"
        spellcheck="false"
        :aria-invalid="!!link.trim() && !valid"
      />
      <p class="min-h-4 truncate" :class="valid && !error ? 'field-helper' : 'field-error'">{{ error || hint }}</p>

      <div v-if="name && picking" class="mt-4">
        <label for="magnet-match" class="field-label">Find it on TMDB</label>
        <input
          id="magnet-match"
          ref="searchField"
          v-model="query"
          class="field"
          autocomplete="off"
          spellcheck="false"
          @keydown.enter.prevent
          @keydown.esc.stop.prevent="picking = false"
        />
        <p class="field-helper min-h-4">
          {{ searching ? "Searching" : query.trim() && !results.length ? "Nothing found. Try fewer words." : "" }}
        </p>
        <ul v-if="results.length" class="match-list mt-2" aria-label="TMDB results" @keydown="arrowNav">
          <li v-for="t in results" :key="`${t.kind}:${t.id}`">
            <button type="button" data-nav-item class="match-row" @click="pick(t)">
              <span class="match-thumb poster-frame">
                <img v-if="t.poster" :src="t.poster" alt="" loading="lazy" draggable="false" />
              </span>
              <span class="min-w-0">
                <span class="block truncate font-semibold">{{ t.title }}</span>
                <span class="block text-ink-2">{{ [t.year, kindLabel(t)].filter(Boolean).join(" · ") }}</span>
              </span>
            </button>
          </li>
        </ul>
        <div class="mt-2 flex gap-2">
          <button type="button" class="btn btn-ghost" @click="picking = false">Back</button>
          <button v-if="chosen" type="button" class="btn btn-ghost" @click="pick(null)">Don't match it</button>
        </div>
      </div>

      <div v-else-if="name" class="mt-4 flex min-h-[134px] gap-13" aria-live="polite">
        <span class="match-poster poster-frame">
          <img v-if="chosen?.poster" :src="chosen.poster" alt="" draggable="false" />
        </span>
        <div v-if="looking" class="text-ink-2">Looking it up on TMDB</div>
        <div v-else-if="chosen" class="min-w-0">
          <p class="truncate font-semibold">{{ chosen.title }}</p>
          <p class="text-ink-2">{{ details }}</p>
          <p v-if="chosen.overview" class="match-overview mt-2">{{ chosen.overview }}</p>
          <button type="button" class="btn btn-ghost mt-1 -ml-2" @click="startPicking">Not this?</button>
        </div>
        <div v-else-if="tmdbBlocked" class="min-w-0">
          <p>Matching is off without a TMDB key.</p>
          <p class="text-ink-2">It still downloads, and gets matched once a key is added.</p>
          <button type="button" class="btn btn-ghost mt-1 -ml-2" @click="closeSheet(); go({ name: 'setup' })">Add a TMDB key</button>
        </div>
        <div v-else class="min-w-0">
          <p>No match on TMDB.</p>
          <p class="text-ink-2">It'll be matched again once the download starts.</p>
          <button type="button" class="btn btn-ghost mt-1 -ml-2" @click="startPicking">Find it</button>
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button type="button" class="btn btn-secondary" @click="closeSheet()">Cancel</button>
      <button type="submit" class="btn btn-primary" :disabled="!valid || adding">{{ adding ? "Adding" : "Add" }}</button>
    </div>
  </form>
</template>

<style scoped>
.match-poster {
  position: relative;
  flex-shrink: 0;
  width: 89px;
  aspect-ratio: 2 / 3;
  overflow: hidden;
}
.match-poster:hover {
  outline-color: transparent;
}
.match-poster img,
.match-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.match-overview {
  display: -webkit-box;
  overflow: hidden;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
}
.match-list {
  max-height: 300px;
  overflow-y: auto;
  border-top: 1px solid var(--c-line);
}
.match-row {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 13px;
  padding: 8px;
  text-align: left;
  border-bottom: 1px solid var(--c-line);
}
.match-row:hover,
.match-row:focus-visible {
  background: var(--c-surface-raised);
}
.match-thumb {
  flex-shrink: 0;
  width: 34px;
  aspect-ratio: 2 / 3;
  overflow: hidden;
}
</style>
