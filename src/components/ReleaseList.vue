<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api, episodeLabel, formatBytes, plainError, type CatalogTitle, type Found } from "../api";
import { go } from "../nav";
import { availability, bestRelease, qualityRank, rankReleases } from "../releases";
import { announce, refresh } from "../store";
import ErrorNote from "./ui/ErrorNote.vue";
import Icon from "./ui/Icon.vue";
import Notification from "./ui/Notification.vue";
import Loader from "./ui/Loader.vue";

const props = defineProps<{
  title: CatalogTitle;
  season?: number | null;
  episode?: number | null;
  alreadyHave?: string;
  alreadyHaveTitle?: string;
}>();

const results = ref<Found[]>([]);
const errors = ref<string[]>([]);
const loading = ref(false);
const showOthers = ref(false);
const adding = ref<Record<string, string>>({});

type Column = "quality" | "name" | "size" | "availability" | "source";
const sortBy = ref<Column | null>(null);
const ascending = ref(false);

const what = computed(() => episodeLabel(props.season ?? null, props.episode ?? null) || "this film");

function order(list: Found[]) {
  if (!sortBy.value) return rankReleases(list);
  const dir = ascending.value ? 1 : -1;
  const key: Record<Column, (r: Found) => number | string> = {
    quality: (r) => qualityRank(r.release.resolution),
    name: (r) => r.title.toLowerCase(),
    size: (r) => r.size_bytes ?? 0,
    availability: (r) => r.seeders ?? -1,
    source: (r) => r.source.toLowerCase(),
  };
  const get = key[sortBy.value];
  return [...list].sort((a, b) => {
    const x = get(a);
    const y = get(b);
    return (typeof x === "string" ? x.localeCompare(y as string) : x - (y as number)) * dir;
  });
}

const matched = computed(() => order(results.value.filter((r) => r.matches)));
const others = computed(() => order(results.value.filter((r) => !r.matches)));
const best = computed(() => bestRelease(rankReleases(results.value.filter((r) => r.matches))));

function sort(column: Column) {
  if (sortBy.value === column) ascending.value = !ascending.value;
  else {
    sortBy.value = column;
    ascending.value = column === "name" || column === "source";
  }
}

async function load() {
  loading.value = true;
  showOthers.value = false;
  try {
    const response = await api.search({
      text: props.title.title,
      year: props.title.kind === "movie" ? props.title.year : null,
      season: props.season ?? null,
      episode: props.episode ?? null,
      match_title: props.title.title,
    });
    results.value = response.results;
    errors.value = response.errors;
  } catch (e) {
    results.value = [];
    errors.value = [String(e)];
  } finally {
    loading.value = false;
  }
}
watch(() => [props.title.id, props.season, props.episode], load, { immediate: true });

async function download(r: Found) {
  adding.value[r.torrent_url] = "adding";
  try {
    await api.addDownload({
      title: props.title.title,
      year: props.title.year,
      source: r.source,
      torrentUrl: r.torrent_url,
      poster: props.title.poster,
      singleVideo: r.single_video,
      tmdbId: props.title.id,
      kind: props.title.kind,
      season: props.season ?? r.release.season,
      episode: props.episode ?? r.release.episode,
    });
    adding.value[r.torrent_url] = "added";
    announce(`Downloading ${props.title.title} ${episodeLabel(props.season ?? null, props.episode ?? null)}`.trim());
    refresh();
  } catch (e) {
    adding.value[r.torrent_url] = plainError(e).message;
  }
}

const columns: { key: Column; label: string; class: string }[] = [
  { key: "quality", label: "Quality", class: "w-[104px]" },
  { key: "name", label: "Release", class: "" },
  { key: "size", label: "Size", class: "w-[104px]" },
  { key: "availability", label: "Availability", class: "w-[128px]" },
  { key: "source", label: "Source", class: "w-[160px]" },
];

const sources = computed(() => new Set(results.value.map((r) => r.source)).size);

function availabilityTag(r: Found) {
  const a = availability(r.seeders);
  return a === "Good" ? "tag-green" : a === "None" ? "tag-red" : "tag-blue";
}
</script>

<template>
  <div class="space-y-21">
    <p v-if="!loading && results.length" class="-mt-5 text-ink-2">
      {{ results.length }} {{ results.length === 1 ? "result" : "results" }} from {{ sources }}
      {{ sources === 1 ? "source" : "sources" }}, best first.
    </p>

    <Notification v-if="alreadyHave" :title="alreadyHaveTitle ?? 'Already in your library'">{{ alreadyHave }}</Notification>

    <Loader v-if="loading" :size="55" :text="`Looking for ${what} in your sources`" class="py-55" />
    <template v-else>
      <ErrorNote v-for="msg in errors" :key="msg" :error="msg">
        <button class="btn btn-ghost btn-sm shrink-0" @click="go({ name: 'preferences', pane: 'sources' })">Sources</button>
      </ErrorNote>

      <div v-if="!matched.length" class="text-ink-2">
        <p>No releases of {{ what }} found{{ others.length ? ", but there are other results below." : "." }}</p>
        <p v-if="!others.length" class="mt-2">
          Searches use the sources in
          <button class="link" @click="go({ name: 'preferences', pane: 'sources' })">Preferences › Sources</button>. If
          none of them has indexers set up, nothing can be found.
          <button class="link" @click="go({ name: 'help', topic: 'sources' })">How do I set up sources?</button>
        </p>
      </div>

      <table v-for="(group, g) in [matched, showOthers ? others : []]" v-show="group.length" :key="g" class="data-table table-fixed">
        <thead>
          <tr>
            <th v-for="c in columns" :key="c.key" :class="c.class" :aria-sort="sortBy === c.key ? (ascending ? 'ascending' : 'descending') : undefined">
              <button class="flex w-full items-center gap-2" @click="sort(c.key)">
                {{ c.label }}
                <Icon v-if="sortBy === c.key" name="chevron-down" :class="{ 'rotate-180': ascending }" />
              </button>
            </th>
            <th class="w-[160px]"><span class="sr-only">Download</span></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in group" :key="r.torrent_url" class="h-14" :class="{ 'opacity-60': r.seeders === 0 }">
            <td><span class="tag">{{ r.release.resolution ?? "SD" }}</span></td>
            <td class="max-w-0">
              <p class="flex items-center gap-2 text-ink">
                <span v-if="r === best" class="tag tag-blue">Best match</span>
                <span class="truncate selectable" :title="r.title">{{ r.title }}</span>
              </p>
              <p
                v-if="adding[r.torrent_url] && !['adding', 'added'].includes(adding[r.torrent_url])"
                class="truncate label-01 text-error"
                :title="adding[r.torrent_url]"
              >
                {{ adding[r.torrent_url] }}
              </p>
              <p v-if="r.release.season != null && r.release.episode == null" class="label-01">Whole season</p>
            </td>
            <td class="tabular-nums">{{ formatBytes(r.size_bytes) }}</td>
            <td :title="r.seeders != null ? `${r.seeders} ${r.seeders === 1 ? 'person is' : 'people are'} sharing this` : 'Not known'">
              <span class="tag" :class="availabilityTag(r)">{{ availability(r.seeders) }}</span>
            </td>
            <td class="max-w-0 truncate" :title="r.source">{{ r.source }}</td>
            <td class="p-0 text-right">
              <span v-if="adding[r.torrent_url] === 'added'" class="flex items-center justify-end gap-2 pr-4">
                <span class="flex items-center gap-1 text-ok"><Icon name="checkmark" /> Added</span>
                <button class="link" @click="go({ name: 'downloads' })">Show</button>
              </span>
              <button
                v-else
                class="btn btn-sm w-full min-w-0"
                :class="r === best ? 'btn-primary' : 'btn-ghost'"
                :disabled="adding[r.torrent_url] === 'adding'"
                @click="download(r)"
              >
                {{ adding[r.torrent_url] === "adding" ? "Adding" : "Download" }} <Icon name="download" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>

      <button v-if="others.length" class="btn btn-ghost btn-sm" :aria-expanded="showOthers" @click="showOthers = !showOthers">
        {{ showOthers ? "Hide" : "Show" }} {{ others.length }} {{ others.length === 1 ? "result that doesn't" : "results that don't" }}
        look like {{ what }}
        <Icon name="chevron-down" :class="{ 'rotate-180': showOthers }" />
      </button>
    </template>
  </div>
</template>
