<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api, type CatalogTitle } from "../../api";
import { arrowNav } from "../../keys";
import { go } from "../../nav";
import { entries, selection, shelf } from "../../store";
import { isTmdbKeyError, noteTmdbError } from "../../tmdb";
import ShelfTile from "../ShelfTile.vue";
import ErrorNote from "../ui/ErrorNote.vue";
import PosterTile from "../ui/PosterTile.vue";
import PageHeader from "../ui/PageHeader.vue";
import SectionHeader from "../ui/SectionHeader.vue";
import Loader from "../ui/Loader.vue";
import TmdbPrompt from "../ui/TmdbPrompt.vue";

const props = defineProps<{ query: string }>();

const titles = ref<CatalogTitle[]>([]);
const searching = ref(false);
const error = ref<unknown>(null);
const picked = ref("");
let latest = 0;

watch(
  () => props.query,
  async (text) => {
    titles.value = [];
    error.value = null;
    selection.value = null;
    if (!text.trim()) return;
    const ticket = ++latest;
    searching.value = true;
    try {
      const found = await api.catalogSearch(text.trim());
      if (ticket === latest) titles.value = found;
    } catch (e) {
      noteTmdbError(e);
      if (ticket === latest) error.value = e;
    } finally {
      if (ticket === latest) searching.value = false;
    }
  },
  { immediate: true },
);

const words = computed(() => props.query.toLowerCase().split(/\s+/).filter(Boolean));
const inLibrary = computed(() =>
  shelf.value.filter((s) => {
    const name = (s.kind === "film" ? s.entry.title : s.show.title).toLowerCase();
    return words.value.length > 0 && words.value.every((w) => name.includes(w));
  }),
);
const owned = computed(() => new Set(entries.value.map((e) => `${e.kind ?? "movie"}:${e.tmdb_id}`)));
const films = computed(() => titles.value.filter((t) => t.kind === "movie"));
const shows = computed(() => titles.value.filter((t) => t.kind === "tv"));

function line(t: CatalogTitle) {
  return [t.year, t.rating ? `${t.rating.toFixed(1)} on TMDB` : ""].filter(Boolean).join(" · ");
}

const subtitle = computed(() => {
  if (searching.value) return "Searching the catalog";
  const n = titles.value.length;
  return n ? `${n} ${n === 1 ? "title" : "titles"} in the catalog` : "";
});

function open(t: CatalogTitle) {
  go({ name: "title", title: t });
}
</script>

<template>
  <div v-if="!query.trim()" class="grid h-full place-items-center p-34">
    <div class="max-w-md">
      <p class="text-heading">Search films and TV shows</p>
      <p class="mt-4 text-ink-2">Type a title in the search field at the top of the window, then press Return.</p>
    </div>
  </div>

  <div v-else class="space-y-55 p-34" @keydown="arrowNav">
    <PageHeader :title="`“${query}”`" :subtitle="subtitle" keep-case />

    <section v-if="inLibrary.length" class="space-y-21" aria-labelledby="h-owned">
      <SectionHeader id="h-owned" title="In your library" />
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(144px,1fr))] gap-x-13 gap-y-21">
        <ShelfTile v-for="s in inLibrary" :key="s.kind === 'film' ? `film:${s.entry.id}` : `show:${s.show.title}`" :item="s" />
      </ul>
    </section>

    <Loader v-if="searching" :size="55" text="Searching the catalog" class="py-55" />
    <TmdbPrompt v-else-if="isTmdbKeyError(error)" feature="search" />
    <ErrorNote v-else-if="error" :error="error" class="max-w-xl" />
    <div v-else-if="!titles.length" class="py-8">
      <p class="text-heading">No results</p>
      <p class="mt-2 text-ink-2">Try the original title, or fewer words.</p>
    </div>

    <section
      v-for="group in [{ id: 'films', heading: 'Films', list: films }, { id: 'shows', heading: 'TV Shows', list: shows }]"
      v-show="group.list.length"
      :key="group.id"
      class="space-y-21"
      :aria-labelledby="`h-${group.id}`"
    >
      <SectionHeader :id="`h-${group.id}`" :title="group.heading" />
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(144px,1fr))] gap-x-13 gap-y-21">
        <PosterTile
          v-for="t in group.list"
          :key="`${t.kind}-${t.id}`"
          :poster="t.poster"
          :title="t.title"
          :line="line(t)"
          :tag="owned.has(`${t.kind}:${t.id}`) ? 'In library' : undefined"
          tag-kind="blue"
          :selected="picked === `${t.kind}-${t.id}`"
          @select="picked = `${t.kind}-${t.id}`"
          @open="open(t)"
          @menu="(e) => e.preventDefault()"
        />
      </ul>
    </section>

    <p class="text-ink-2">
      Film and show information and images come from TMDB. This product uses the TMDB API but is not endorsed or
      certified by TMDB.
    </p>
  </div>
</template>
