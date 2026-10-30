<script setup lang="ts">
import globe from "../../../brand/assets/logo/cardamoi-globe-red.svg";
import { computed, ref } from "vue";
import { episodeLabel, timeLeft } from "../../api";
import { backdropOf } from "../../backdrops";
import { openFilm, openFolder, openShow, play, videoMenu } from "../../commands";
import { arrowNav } from "../../keys";
import { playedShare, upNext, type UpNextItem } from "../../library";
import { go } from "../../nav";
import { entries, selection, shelf } from "../../store";
import { openMenu, openMenuBelow } from "../../ui";
import ShelfTile from "../ShelfTile.vue";
import Icon from "../ui/Icon.vue";
import PageHeader from "../ui/PageHeader.vue";
import PosterTile from "../ui/PosterTile.vue";
import ProgressBar from "../ui/ProgressBar.vue";
import SectionHeader from "../ui/SectionHeader.vue";

const rows = computed(() => upNext(shelf.value));

const hero = computed(() => rows.value.continueWatching[0] ?? rows.value.nextEpisodes[0] ?? null);
const nextUp = computed(() =>
  [...rows.value.continueWatching, ...rows.value.nextEpisodes].filter((i) => i !== hero.value),
);

const heroArt = computed(() => {
  const item = hero.value;
  if (!item) return null;
  const entry = item.ep.entry;
  return backdropOf(item.show ? "tv" : entry.kind, entry.tmdb_id) ?? item.show?.poster ?? entry.poster;
});

function heroLine(item: UpNextItem) {
  const v = item.ep.video;
  if (item.show && v.season != null && v.episode != null) return `Season ${v.season}, Episode ${v.episode}`;
  return item.ep.entry.year ? String(item.ep.entry.year) : "";
}

function line(item: UpNextItem) {
  const v = item.ep.video;
  const label = item.show ? episodeLabel(v.season, v.episode) : "";
  const status = v.resumeAt != null ? timeLeft(v) : "";
  return [label, status].filter(Boolean).join(" · ") || (item.ep.entry.year ? String(item.ep.entry.year) : "");
}

function readyTag(item: UpNextItem) {
  const e = item.ep.entry;
  return e.status === "done" ? { text: undefined, kind: "gray" as const } : { text: `${Math.floor(e.progress * 100)} %`, kind: "blue" as const };
}

function isSelected(item: UpNextItem) {
  const s = selection.value;
  return s?.kind === "video" && s.ep.entry.id === item.ep.entry.id && s.ep.video.fileIndex === item.ep.video.fileIndex;
}

function open(item: UpNextItem) {
  if (item.show) openShow(item.show);
  else openFilm(item.ep);
}

const more = ref<HTMLElement>();

function heroMenu() {
  const item = hero.value;
  if (item && more.value) openMenuBelow(more.value, videoMenu(item.ep, { open: () => open(item) }));
}

function focusSearch() {
  document.getElementById("toolbar-search")?.focus();
}
</script>

<template>
  <div v-if="!entries.length" class="grid h-full place-items-center p-34">
    <div class="flex max-w-[754px] items-center gap-34 border border-subtle bg-layer p-34">
      <img :src="globe" alt="" class="size-[144px] shrink-0" draggable="false" />
      <div>
      <p class="empty-state">Nothing to watch yet</p>
      <p class="mt-2 body-02 text-ink-2">
        Search for a film or show and download it, or put video files you already have in the cardamoi folder in Movies. Either
        way, they show up here ready to watch.
      </p>
      <div class="mt-21 flex gap-13">
        <button class="btn btn-primary btn-lg" @click="focusSearch">Search for something <Icon name="search" /></button>
        <button class="btn btn-ghost btn-lg" @click="openFolder">Open folder <Icon name="folder" /></button>
      </div>
      </div>
    </div>
  </div>

  <div v-else class="space-y-55 p-34" @keydown="arrowNav">
    <PageHeader title="Up Next" subtitle="Pick up where you left off, or start something new." />

    <section v-if="hero" class="flex min-h-[360px] border border-subtle bg-layer" aria-labelledby="hero-title">
      <button class="relative w-[57%] shrink-0 overflow-hidden bg-layer-2" tabindex="-1" @click="open(hero)">
        <img v-if="heroArt" :src="heroArt" alt="" class="absolute inset-0 size-full object-cover" />
      </button>
      <div class="flex min-w-0 flex-1 flex-col p-34">
        <p class="eyebrow">{{ hero.ep.video.resumeAt != null ? "Continue watching" : "Next episode" }}</p>
        <h2 id="hero-title" class="mt-2 heading-05 truncate">
          <button class="max-w-full truncate text-left hover:underline" @click="open(hero)">
            {{ hero.show?.title ?? hero.ep.entry.title }}
          </button>
        </h2>
        <p class="mt-2 label-01 text-[13px] leading-5 text-ink-2">{{ heroLine(hero) }}</p>
        <div class="flex-1" />
        <template v-if="hero.ep.video.resumeAt != null">
          <ProgressBar :value="playedShare(hero.ep.video)" label="Watched so far" />
          <p class="mt-2 label-01 text-[13px] leading-5 text-ink-2">{{ timeLeft(hero.ep.video) }}</p>
        </template>
        <div class="mt-21 flex flex-wrap items-center gap-13">
          <button class="btn btn-primary btn-lg" @click="play(hero.ep)">
            {{ hero.ep.video.resumeAt != null ? "Continue" : "Play" }} <Icon name="play" />
          </button>
          <button v-if="hero.ep.video.resumeAt != null" class="btn btn-ghost btn-lg" @click="play(hero.ep, true)">Play from beginning</button>
          <button ref="more" class="btn-icon ml-auto" aria-label="More" aria-haspopup="menu" @click="heroMenu">
            <Icon name="overflow" />
          </button>
        </div>
      </div>
    </section>

    <section v-if="nextUp.length" class="space-y-21" aria-labelledby="h-next">
      <SectionHeader id="h-next" title="Next up" />
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-x-13 gap-y-21">
        <PosterTile
          v-for="item in nextUp"
          :key="`${item.ep.entry.id}:${item.ep.video.fileIndex}`"
          :poster="item.show?.poster ?? item.ep.entry.poster"
          :title="item.show?.title ?? item.ep.entry.title"
          :line="line(item)"
          :played="item.ep.video.resumeAt != null ? playedShare(item.ep.video) : 0"
          :selected="isSelected(item)"
          playable
          @select="selection = { kind: 'video', ep: item.ep }"
          @open="open(item)"
          @play="play(item.ep)"
          @menu="(e) => openMenu(e, videoMenu(item.ep, { open: () => open(item) }))"
        />
      </ul>
    </section>

    <section v-if="rows.ready.length" class="space-y-21" aria-labelledby="h-ready">
      <SectionHeader id="h-ready" title="Ready to watch">
        <template #action>
          <button class="btn btn-ghost btn-sm" @click="go({ name: 'films' })">See all <Icon name="chevron-right" /></button>
        </template>
      </SectionHeader>
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-x-13 gap-y-21">
        <PosterTile
          v-for="item in rows.ready"
          :key="`${item.ep.entry.id}:${item.ep.video.fileIndex}`"
          :poster="item.show?.poster ?? item.ep.entry.poster"
          :title="item.show?.title ?? item.ep.entry.title"
          :line="line(item)"
          :tag="readyTag(item).text"
          :tag-kind="readyTag(item).kind"
          :fill="item.ep.entry.status === 'done' ? null : item.ep.entry.progress"
          :selected="isSelected(item)"
          playable
          @select="selection = { kind: 'video', ep: item.ep }"
          @open="open(item)"
          @play="play(item.ep)"
          @menu="(e) => openMenu(e, videoMenu(item.ep, { open: () => open(item) }))"
        />
      </ul>
    </section>

    <section class="space-y-21" aria-labelledby="h-recent">
      <SectionHeader id="h-recent" title="Recently added" />
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(144px,1fr))] gap-x-13 gap-y-21">
        <ShelfTile
          v-for="item in rows.recentlyAdded"
          :key="item.kind === 'film' ? `film:${item.entry.id}` : `show:${item.show.title}`"
          :item="item"
        />
      </ul>
    </section>
  </div>
</template>
