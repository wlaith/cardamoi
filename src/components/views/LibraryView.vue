<script setup lang="ts">
import { computed, ref } from "vue";
import { formatBytes } from "../../api";
import { moveToTrash, openFilm, openShow, play, showMenu, videoMenu } from "../../commands";
import { arrowNav } from "../../keys";
import { filmVideo, nextEpisode, showKey, showSummary, type Shelf } from "../../library";
import { selection, shelf, viewMode } from "../../store";
import { filmLine, showLine } from "../../status";
import { openMenu } from "../../ui";
import ShelfTile from "../ShelfTile.vue";
import Icon from "../ui/Icon.vue";
import PageHeader from "../ui/PageHeader.vue";

const props = defineProps<{ kind: "film" | "show" }>();

const items = computed(() => shelf.value.filter((s) => s.kind === props.kind));

type Filter = "all" | "unwatched" | "downloading";
const filter = ref<Filter>("all");
const filters: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unwatched", label: "Unwatched" },
  { id: "downloading", label: "Downloading" },
];

function unwatched(s: Shelf) {
  if (s.kind === "show") return showSummary(s.show).unwatched > 0;
  const ep = filmVideo(s.entry);
  return !!ep && !ep.video.watched;
}
function downloading(s: Shelf) {
  if (s.kind === "show") return showSummary(s.show).downloading > 0;
  return s.entry.status !== "done" && s.entry.status !== "missing";
}

type Column = "name" | "year" | "status" | "size" | "added";
const sortBy = ref<Column>("added");
const ascending = ref(false);

function name(s: Shelf) {
  return s.kind === "film" ? s.entry.title : s.show.title;
}
function year(s: Shelf) {
  return (s.kind === "film" ? s.entry.year : s.show.year) ?? 0;
}
function status(s: Shelf) {
  return s.kind === "film" ? filmLine(s.entry).text : showLine(s.show);
}
function size(s: Shelf) {
  return s.kind === "film" ? s.entry.totalBytes : s.show.entries.reduce((n, e) => n + e.totalBytes, 0);
}
function added(s: Shelf) {
  return s.kind === "film" ? s.entry.added_at : Math.max(...s.show.entries.map((e) => e.added_at));
}

const shown = computed(() => {
  const list = items.value.filter((s) => filter.value === "all" || (filter.value === "unwatched" ? unwatched(s) : downloading(s)));
  const key = { name, year, status, size, added }[sortBy.value];
  const dir = ascending.value ? 1 : -1;
  return [...list].sort((a, b) => {
    const x = key(a);
    const y = key(b);
    return (typeof x === "string" ? x.localeCompare(y as string) : x - (y as number)) * dir;
  });
});

const totalSize = computed(() => formatBytes(items.value.reduce((n, s) => n + size(s), 0)));
const subtitle = computed(() => {
  const n = items.value.length;
  const noun = props.kind === "film" ? (n === 1 ? "film" : "films") : n === 1 ? "show" : "shows";
  return `${n} ${noun} · ${totalSize.value}`;
});

const sortChoice = computed({
  get: () => `${sortBy.value}:${ascending.value ? "asc" : "desc"}`,
  set: (value: string) => {
    const [column, dir] = value.split(":");
    sortBy.value = column as Column;
    ascending.value = dir === "asc";
  },
});
const sorts = [
  { value: "added:desc", label: "Sort: Recently added" },
  { value: "name:asc", label: "Sort: Name" },
  { value: "year:desc", label: "Sort: Newest" },
  { value: "size:desc", label: "Sort: Largest" },
];

function sort(column: Column) {
  if (sortBy.value === column) ascending.value = !ascending.value;
  else {
    sortBy.value = column;
    ascending.value = column === "name";
  }
}

function ariaSort(column: Column) {
  if (sortBy.value !== column) return undefined;
  return ascending.value ? "ascending" : "descending";
}

function keyOf(s: Shelf) {
  return s.kind === "film" ? `film:${s.entry.id}` : showKey(s.show);
}

function isSelected(s: Shelf) {
  const sel = selection.value;
  if (!sel) return false;
  if (s.kind === "show") return sel.kind === "show" && showKey(sel.show) === showKey(s.show);
  return sel.kind === "video" && sel.ep.entry.id === s.entry.id;
}

function select(s: Shelf) {
  if (s.kind === "show") selection.value = { kind: "show", show: s.show };
  else {
    const ep = filmVideo(s.entry);
    if (ep) selection.value = { kind: "video", ep };
  }
}

function open(s: Shelf) {
  if (s.kind === "show") openShow(s.show);
  else {
    const ep = filmVideo(s.entry);
    if (ep) openFilm(ep);
  }
}

function menu(event: MouseEvent, s: Shelf) {
  select(s);
  if (s.kind === "show") return openMenu(event, showMenu(s.show));
  const ep = filmVideo(s.entry);
  if (ep) openMenu(event, videoMenu(ep, { open: () => open(s) }));
  else openMenu(event, [{ label: "Cancel Download…", action: () => moveToTrash(s.entry), danger: true }]);
}

function playRow(s: Shelf) {
  const ep = s.kind === "film" ? filmVideo(s.entry) : nextEpisode(s.show);
  if (ep?.video.playable) play(ep, ep.video.watched);
}

const columns: { key: Column; label: string; class?: string }[] = [
  { key: "name", label: "Name" },
  { key: "year", label: "Year", class: "w-20" },
  { key: "status", label: "Status", class: "w-[32%]" },
  { key: "size", label: "Size", class: "w-24" },
  { key: "added", label: "Added", class: "w-32" },
];
</script>

<template>
  <div v-if="!items.length" class="grid h-full place-items-center p-34">
    <div class="max-w-sm">
      <p class="empty-state">{{ kind === "film" ? "No films yet" : "No TV shows yet" }}</p>
      <p class="mt-4 body-02 text-ink-2">
        Use the search field at the top of the window to find {{ kind === "film" ? "a film" : "a show" }} and download it.
      </p>
    </div>
  </div>

  <div v-else class="space-y-55 p-34">
    <div class="space-y-21">
      <PageHeader :title="kind === 'film' ? 'Films' : 'TV Shows'" :subtitle="subtitle" />
      <div class="flex flex-wrap items-center gap-4">
        <div class="tabs flex-1" role="tablist" aria-label="Show">
          <button
            v-for="f in filters"
            :key="f.id"
            class="tab"
            role="tab"
            :aria-selected="filter === f.id"
            @click="filter = f.id"
          >
            {{ f.label }}
          </button>
        </div>
        <select v-model="sortChoice" class="select w-[240px]" aria-label="Sort by">
          <option v-for="o in sorts" :key="o.value" :value="o.value">{{ o.label }}</option>
          <option v-if="!sorts.some((o) => o.value === sortChoice)" :value="sortChoice">Sort: {{ sortBy }}</option>
        </select>
        <div class="switcher" role="group" aria-label="View">
          <button :aria-pressed="viewMode === 'grid'" aria-label="As grid" title="As Grid (⌘1)" @click="viewMode = 'grid'">
            <Icon name="grid" />
          </button>
          <button :aria-pressed="viewMode === 'list'" aria-label="As list" title="As List (⌘2)" @click="viewMode = 'list'">
            <Icon name="list" />
          </button>
        </div>
      </div>
    </div>

    <p v-if="!shown.length" class="text-ink-2">
      {{ filter === "unwatched" ? "You've watched everything here." : "Nothing is downloading." }}
    </p>

    <ul
      v-else-if="viewMode === 'grid'"
      class="grid grid-cols-[repeat(auto-fill,minmax(144px,1fr))] gap-x-13 gap-y-21"
      :aria-label="kind === 'film' ? 'Films' : 'TV Shows'"
      @keydown="arrowNav"
    >
      <ShelfTile v-for="s in shown" :key="keyOf(s)" :item="s" />
    </ul>

    <table v-else class="data-table" :aria-label="kind === 'film' ? 'Films' : 'TV Shows'" @keydown="arrowNav">
      <thead>
        <tr>
          <th v-for="c in columns" :key="c.key" :class="c.class" :aria-sort="ariaSort(c.key)">
            <button class="flex w-full items-center gap-2" @click="sort(c.key)">
              {{ c.label }}
              <Icon v-if="sortBy === c.key" name="chevron-down" :class="{ 'rotate-180': ascending }" />
            </button>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="s in shown"
          :key="keyOf(s)"
          data-nav-item
          tabindex="0"
          class="h-12"
          :class="{ 'row-selected': isSelected(s) }"
          :aria-selected="isSelected(s)"
          @click="select(s)"
          @focus="select(s)"
          @dblclick="open(s)"
          @keydown.enter.prevent="open(s)"
          @keydown.space.prevent="playRow(s)"
          @contextmenu="menu($event, s)"
        >
          <td class="cell-strong max-w-0 truncate">{{ name(s) }}</td>
          <td class="tabular-nums">{{ year(s) || "" }}</td>
          <td class="max-w-0 truncate">{{ status(s) }}</td>
          <td class="tabular-nums">{{ formatBytes(size(s)) }}</td>
          <td>{{ new Date(added(s) * 1000).toLocaleDateString() }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
