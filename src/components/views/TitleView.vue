<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api, episodeLabel, formatBytes, timeLeft, type CatalogTitle, type Details, type Episode } from "../../api";
import { moveToTrash, play, retry, setSeasonWatched, showMenu, subtitles, videoMenu } from "../../commands";
import { episodeKey, groupLibrary, nextEpisode, type LocalEpisode } from "../../library";
import { go } from "../../nav";
import { entries, selection } from "../../store";
import { canRetry, filmLine, resolvingText } from "../../status";
import { openMenuBelow } from "../../ui";
import EpisodeDetails from "../EpisodeDetails.vue";
import EpisodeTable, { type EpisodeRowData } from "../EpisodeTable.vue";
import ReleaseList from "../ReleaseList.vue";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import ProgressBar from "../ui/ProgressBar.vue";
import SectionHeader from "../ui/SectionHeader.vue";
import Loader from "../ui/Loader.vue";
import Spinner from "../ui/Spinner.vue";

const props = defineProps<{ title: CatalogTitle }>();

const details = ref<Details | null>(null);
const loadError = ref<unknown>(null);
const shown = computed<CatalogTitle>(() => details.value ?? props.title);

const season = ref<number | null>(null);
const episodes = ref<Episode[]>([]);
const episodesError = ref<unknown>(null);
const loadingEpisodes = ref(false);
const openRow = ref<string | null>(null);
const findingReleases = ref(false);

const mine = computed(() => entries.value.filter((e) => e.tmdb_id === props.title.id));
const show = computed(() => {
  const shelf = groupLibrary(mine.value.filter((e) => e.kind === "tv" || e.season != null));
  return shelf.find((s) => s.kind === "show")?.show ?? null;
});
const local = computed(() => new Map(show.value?.episodes.map((e) => [episodeKey(e.video.season, e.video.episode), e])));
const next = computed(() => (show.value ? nextEpisode(show.value) : null));
const filmEntry = computed(() => mine.value.find((e) => e.kind === "movie"));
const film = computed<LocalEpisode | null>(() => {
  const entry = filmEntry.value;
  return entry?.videos.length ? { entry, video: entry.videos[0] } : null;
});

function localOf(number: number) {
  return local.value.get(episodeKey(season.value, number));
}

const today = new Date().toISOString().slice(0, 10);
const rows = computed<EpisodeRowData[]>(() =>
  episodes.value.map((e) => ({
    key: String(e.number),
    number: e.number,
    name: e.name,
    airDate: e.airDate,
    still: e.still,
    overview: e.overview,
    local: localOf(e.number),
  })),
);
const openEpisode = computed(() => episodes.value.find((e) => String(e.number) === openRow.value) ?? null);
const seasonLocal = computed(() => [...local.value.values()].filter((e) => e.video.season === season.value));
const seasonAllWatched = computed(() => seasonLocal.value.length > 0 && seasonLocal.value.every((e) => e.video.watched));
const missingInSeason = computed(
  () => episodes.value.filter((e) => !localOf(e.number) && (!e.airDate || e.airDate <= today)).length,
);
const seasonWatched = computed(() => seasonLocal.value.filter((e) => e.video.watched).length);

watch(
  () => props.title.id,
  async () => {
    details.value = null;
    loadError.value = null;
    season.value = null;
    try {
      details.value = await api.titleDetails(props.title.kind, props.title.id);
      season.value = next.value?.video.season ?? details.value.seasons.find((s) => s.number > 0)?.number ?? details.value.seasons[0]?.number ?? null;
    } catch (e) {
      loadError.value = e;
    }
  },
  { immediate: true },
);

watch(season, async (number) => {
  episodes.value = [];
  episodesError.value = null;
  openRow.value = null;
  findingReleases.value = false;
  if (number == null) return;
  loadingEpisodes.value = true;
  try {
    episodes.value = await api.seasonEpisodes(props.title.id, number);
    if (next.value && next.value.video.season === number) pick(String(next.value.video.episode));
  } catch (e) {
    episodesError.value = e;
  } finally {
    loadingEpisodes.value = false;
  }
});

function pick(key: string) {
  openRow.value = key;
  findingReleases.value = false;
}

function playNext() {
  if (next.value) play(next.value);
}

function markSeason() {
  if (season.value != null) setSeasonWatched(props.title.id, season.value, seasonLocal.value, !seasonAllWatched.value);
}

const meta = computed(() => {
  const parts: string[] = [];
  if (shown.value.year) parts.push(String(shown.value.year));
  if (props.title.kind === "tv" && details.value) {
    const n = details.value.seasons.filter((s) => s.number > 0).length;
    if (n) parts.push(`${n} ${n === 1 ? "season" : "seasons"}`);
  }
  if (details.value?.runtime) {
    const h = Math.floor(details.value.runtime / 60);
    const m = details.value.runtime % 60;
    parts.push(h ? `${h} h ${m} min` : `${m} min`);
  }
  if (shown.value.rating) parts.push(`${shown.value.rating.toFixed(1)} on TMDB`);
  return parts.join(" · ");
});

const more = ref<HTMLElement>();

function moreMenu() {
  if (!more.value) return;
  if (film.value) openMenuBelow(more.value, videoMenu(film.value));
  else if (show.value) openMenuBelow(more.value, showMenu(show.value));
}

watch(
  film,
  (ep) => {
    if (ep) selection.value = { kind: "video", ep };
  },
  { immediate: true },
);
</script>

<template>
  <article class="space-y-55 p-34">
    <div class="space-y-21">
      <nav class="breadcrumb" aria-label="Breadcrumb">
        <button @click="go({ name: title.kind === 'movie' ? 'films' : 'shows' })">{{ title.kind === "movie" ? "Films" : "TV Shows" }}</button>
        <span class="text-ink-2" aria-hidden="true">/</span>
        <span class="truncate" aria-current="page">{{ shown.title }}</span>
      </nav>

      <div v-if="shown.backdrop" class="h-[280px] overflow-hidden bg-layer">
        <img :src="shown.backdrop" alt="" class="size-full object-cover" />
      </div>

      <div class="flex gap-34">
        <img v-if="shown.poster" :src="shown.poster" alt="" class="poster-frame h-[300px] w-[200px] shrink-0 object-cover" />
        <div v-else class="poster-frame grid h-[300px] w-[200px] shrink-0 place-items-center p-4 text-center heading-03 text-ink-2">
          {{ shown.title }}
        </div>

        <div class="min-w-0 flex-1 space-y-4">
          <h1 class="heading-07 selectable">{{ shown.title }}</h1>
          <div class="flex flex-wrap items-center gap-2">
            <span v-for="g in details?.genres ?? []" :key="g" class="tag tag-blue">{{ g }}</span>
            <span class="text-ink-2">{{ meta }}</span>
          </div>
          <p v-if="shown.overview" class="max-w-[80ch] body-02 text-ink-2 selectable">{{ shown.overview }}</p>
          <ErrorNote v-if="loadError" :error="loadError" class="max-w-xl" />

          <div v-if="title.kind === 'tv'" class="flex flex-wrap items-center gap-4 pt-2">
            <button v-if="next" class="btn btn-primary btn-lg" @click="playNext">
              {{ next.video.resumeAt != null ? "Continue" : "Play" }} {{ episodeLabel(next.video.season, next.video.episode) }}
              <Icon name="play" />
            </button>
            <button v-if="season != null && missingInSeason" class="btn btn-secondary btn-lg" @click="pick('pack')">
              {{ seasonLocal.length ? `Download the other ${missingInSeason}` : `Download season ${season}` }}
              <Icon name="download" />
            </button>
            <button v-if="show" ref="more" class="btn-icon size-12" aria-label="More" aria-haspopup="menu" @click="moreMenu">
              <Icon name="overflow" />
            </button>
            <p v-if="next?.video.resumeAt != null" class="w-full helper-text-01">{{ timeLeft(next.video) }}</p>
          </div>

          <template v-else-if="filmEntry">
            <div class="flex flex-wrap items-center gap-4 pt-2">
              <template v-if="film?.video.playable">
                <button class="btn btn-primary btn-lg" @click="play(film, film.video.watched)">
                  {{ film.video.resumeAt != null ? "Continue" : film.video.watched ? "Play again" : "Play" }} <Icon name="play" />
                </button>
                <button v-if="film.video.resumeAt != null" class="btn btn-ghost btn-lg" @click="play(film, true)">Play from beginning</button>
              </template>
              <button v-if="film" class="btn btn-ghost btn-lg" @click="subtitles(film)">Subtitles <Icon name="captions" /></button>
              <button v-if="film" ref="more" class="btn-icon size-12" aria-label="More" aria-haspopup="menu" @click="moreMenu">
                <Icon name="overflow" />
              </button>
              <button v-else class="btn btn-danger-ghost btn-lg" @click="moveToTrash(filmEntry)">Cancel download <Icon name="trash" /></button>
            </div>
            <p v-if="film?.video.resumeAt != null" class="helper-text-01">{{ timeLeft(film.video) }}</p>
            <div v-if="filmEntry.status !== 'done'" class="max-w-xl">
              <ProgressBar
                :value="filmEntry.progress"
                :state="filmEntry.status === 'paused' ? 'paused' : filmEntry.status === 'error' ? 'failed' : 'active'"
                label="Download progress"
              />
              <p class="mt-2 helper-text-01">{{ filmLine(filmEntry).text }}</p>
            </div>
          </template>

          <ul v-if="show?.pending.length" class="max-w-xl space-y-2">
            <li v-for="p in show.pending" :key="p.id" class="notification" :class="{ 'notification-error': p.status === 'error' }">
              <Spinner v-if="p.status !== 'error'" class="mt-0.5" />
              <Icon v-else name="error-filled" class="notification-icon" />
              <div class="min-w-0 flex-1">
                <p class="font-semibold">{{ episodeLabel(p.season, p.episode) || "Download" }}</p>
                <p class="text-ink-2" role="status">
                  {{ p.status === "resolving" ? resolvingText(p) : p.status === "error" ? "Couldn't get the file list" : "Finding peers" }}
                </p>
              </div>
              <button v-if="canRetry(p)" class="btn btn-ghost btn-sm" @click="retry(p)">Try again</button>
              <button class="btn btn-ghost btn-sm" @click="moveToTrash(p)">Cancel</button>
            </li>
          </ul>
        </div>
      </div>
    </div>

    <section v-if="title.kind === 'movie'" class="space-y-21" aria-labelledby="releases-heading">
      <SectionHeader id="releases-heading" :title="filmEntry ? 'Other releases' : 'Releases'" />
      <ReleaseList
        :title="title"
        :already-have="filmEntry ? `${formatBytes(filmEntry.totalBytes)}, added ${new Date(filmEntry.added_at * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}. Downloading another release keeps both copies.` : undefined"
      />
    </section>

    <section v-else-if="details" class="space-y-21" aria-label="Episodes">
      <div class="flex flex-wrap items-center gap-4">
        <div v-if="details.seasons.length <= 8" class="tabs flex-1" role="tablist" aria-label="Seasons">
          <button
            v-for="s in details.seasons"
            :key="s.number"
            class="tab"
            role="tab"
            :aria-selected="season === s.number"
            @click="season = s.number"
          >
            {{ s.name }}
          </button>
        </div>
        <div v-else class="flex-1">
          <select v-model="season" class="select w-[240px]" aria-label="Season">
            <option v-for="s in details.seasons" :key="s.number" :value="s.number">{{ s.name }}</option>
          </select>
        </div>
        <span v-if="seasonLocal.length" class="text-ink-2">{{ seasonWatched }} of {{ episodes.length || seasonLocal.length }} watched</span>
        <button v-if="seasonLocal.length" class="btn btn-ghost btn-sm" @click="markSeason">
          Mark season as {{ seasonAllWatched ? "unwatched" : "watched" }} <Icon name="checkmark" />
        </button>
      </div>
      <p v-if="!details.seasons.length" class="text-ink-2">TMDB doesn't list any seasons for this show yet.</p>

      <div v-if="openRow === 'pack'" class="space-y-21 bg-layer p-6">
        <SectionHeader :title="`Season ${season} packs`">
          <template #action>
            <button class="btn-icon btn-sm" aria-label="Close season packs" @click="openRow = null"><Icon name="close" /></button>
          </template>
        </SectionHeader>
        <p class="-mt-4 text-ink-2">
          Every episode in one download.
          <template v-if="seasonLocal.length">Episodes you already have stay as they are.</template>
        </p>
        <ReleaseList :title="title" :season="season" :episode="null" />
      </div>

      <ErrorNote v-if="episodesError" :error="episodesError" />
      <Loader v-else-if="loadingEpisodes" inline text="Loading episodes" class="py-21" />
      <EpisodeTable v-else-if="rows.length" :rows="rows" :selected="openRow" @select="pick">
        <template #detail="{ row }">
          <EpisodeDetails
            v-if="row.local && !findingReleases"
            :local="row.local"
            :still="openEpisode?.still"
            :overview="openEpisode?.overview"
            @find-releases="findingReleases = true"
          />
          <div v-else class="space-y-21 py-6 pr-4 pl-[72px]">
            <p v-if="openEpisode?.overview" class="max-w-[72ch] text-ink">{{ openEpisode.overview }}</p>
            <ReleaseList
              :title="title"
              :season="season"
              :episode="row.number"
              :already-have="row.local ? 'You already have this episode. Downloading another release keeps both copies.' : undefined"
              already-have-title="Already downloaded"
            />
          </div>
        </template>
      </EpisodeTable>
    </section>
  </article>
</template>
