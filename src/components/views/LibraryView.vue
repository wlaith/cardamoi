<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { formatBytes } from "../../api";
import { moveToTrash, openFilm, openShow, play, reveal, setManyWatched, showMenu, videoMenu } from "../../commands";
import { arrowNav, isTyping } from "../../keys";
import { filmVideo, nextEpisode, showKey, showSummary, type LocalEpisode, type Shelf } from "../../library";
import { COLUMNS, columns, selection, shelf, viewMode } from "../../store";
import { filmLine, showLine } from "../../status";
import { contextMenu, openMenu, palette, sheet } from "../../ui";
import ShelfTile from "../ShelfTile.vue";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
import PageHeader from "../ui/PageHeader.vue";
import TabList from "../ui/TabList.vue";

const props = defineProps<{ kind: "film" | "show" }>();

const items = computed(() => shelf.value.filter((s) => s.kind === props.kind));

type Filter = "all" | "unwatched" | "downloading";
const filter = ref<Filter>("all");

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

const test: Record<Filter, (s: Shelf) => boolean> = { all: () => true, unwatched, downloading };
const filters = computed(() =>
  (
    [
      { id: "all", label: "All" },
      { id: "unwatched", label: "Unwatched" },
      { id: "downloading", label: "Downloading" },
    ] as const
  ).map((f) => ({ ...f, count: items.value.filter(test[f.id]).length })),
);

const shown = computed(() => {
  const list = items.value.filter(test[filter.value]);
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

const picked = ref(new Set<string>());
let anchor: string | null = null;

function pick(s: Shelf, event: MouseEvent) {
  const key = keyOf(s);
  const next = new Set(event.metaKey || event.shiftKey ? picked.value : []);
  if (event.shiftKey && anchor) {
    const keys = shown.value.map(keyOf);
    const [a, b] = [keys.indexOf(anchor), keys.indexOf(key)].sort((x, y) => x - y);
    if (a >= 0) keys.slice(a, b + 1).forEach((k) => next.add(k));
  } else if (event.metaKey) {
    if (!next.size && anchor && anchor !== key) next.add(anchor);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    anchor = key;
  } else {
    anchor = key;
  }
  picked.value = next;
}

const pickedItems = computed(() => shown.value.filter((s) => picked.value.has(keyOf(s))));
const several = computed(() => pickedItems.value.length > 1);
const pickedFilms = computed(() =>
  pickedItems.value.flatMap((s) => (s.kind === "film" ? [filmVideo(s.entry)].filter((ep): ep is LocalEpisode => !!ep) : [])),
);
const allWatched = computed(() => pickedFilms.value.every((ep) => ep.video.watched));

function release() {
  picked.value = new Set();
}

function markPicked() {
  setManyWatched(pickedFilms.value, !allWatched.value);
}

function revealPicked() {
  pickedItems.value.slice(0, 5).forEach((s) => reveal(s.kind === "film" ? s.entry : s.show.entries[0]));
}

function trashPicked() {
  moveToTrash(pickedItems.value.flatMap((s) => (s.kind === "film" ? [s.entry] : s.show.entries)));
  release();
}

watch([filter, () => props.kind], release);

function setColumns(n: number) {
  columns.value = Math.min(COLUMNS.max, Math.max(COLUMNS.min, n));
}

function onWindowKey(event: KeyboardEvent) {
  if (event.defaultPrevented || sheet.value || contextMenu.value || palette.value || isTyping(event.target)) return;
  if (event.key === "Escape" && picked.value.size) release();
  else if (event.metaKey && (event.key === "=" || event.key === "+") && viewMode.value === "grid") setColumns(columns.value - 1);
  else if (event.metaKey && event.key === "-" && viewMode.value === "grid") setColumns(columns.value + 1);
  else return;
  event.preventDefault();
}
onMounted(() => window.addEventListener("keydown", onWindowKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onWindowKey));

function holdInPlace(el: Element) {
  const tile = el as HTMLElement;
  Object.assign(tile.style, {
    left: `${tile.offsetLeft}px`,
    top: `${tile.offsetTop}px`,
    width: `${tile.offsetWidth}px`,
  });
}

function menu(event: MouseEvent, s: Shelf) {
  select(s);
  if (s.kind === "show") return openMenu(event, showMenu(s.show));
  const ep = filmVideo(s.entry);
  if (ep) openMenu(event, videoMenu(ep, { open: () => open(s) }));
  else openMenu(event, [{ label: "Cancel Download", action: () => moveToTrash(s.entry), danger: true }]);
}

function playRow(s: Shelf) {
  const ep = s.kind === "film" ? filmVideo(s.entry) : nextEpisode(s.show);
  if (ep?.video.playable) play(ep, ep.video.watched);
}

const tableColumns: { key: Column; label: string; class?: string }[] = [
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
      <p class="text-heading">{{ kind === "film" ? "No films yet" : "No TV shows yet" }}</p>
      <p class="mt-4 text-ink-2">
        Use the search field at the top of the window to find {{ kind === "film" ? "a film" : "a show" }} and download it.
      </p>
    </div>
  </div>

  <div v-else class="space-y-55 p-34">
    <div class="space-y-21">
      <PageHeader :title="kind === 'film' ? 'Films' : 'TV Shows'" :subtitle="subtitle" />
      <div class="flex flex-wrap items-center gap-13">
        <TabList v-model="filter" :items="filters" label="Show" class="flex-1" />
        <div v-if="viewMode === 'grid'" class="density" role="group" aria-label="Posters to a row">
          <button class="btn-icon btn-sm" :disabled="columns >= COLUMNS.max" aria-label="Smaller posters" title="Smaller (⌘−)" @click="setColumns(columns + 1)">
            <Icon name="subtract" />
          </button>
          <span class="density-value" aria-live="polite">{{ columns }} across</span>
          <button class="btn-icon btn-sm" :disabled="columns <= COLUMNS.min" aria-label="Bigger posters" title="Bigger (⌘+)" @click="setColumns(columns - 1)">
            <Icon name="add" />
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

    <TransitionGroup
      v-else-if="viewMode === 'grid'"
      tag="ul"
      name="tile"
      class="tile-grid relative grid gap-x-13 gap-y-21"
      :style="{ '--columns': columns }"
      :aria-label="kind === 'film' ? 'Films' : 'TV Shows'"
      :aria-multiselectable="true"
      @keydown="arrowNav"
      @before-leave="holdInPlace"
    >
      <ShelfTile
        v-for="(s, i) in shown"
        :key="keyOf(s)"
        :item="s"
        :checked="several && picked.has(keyOf(s))"
        :style="{ '--i': Math.min(i, 8) }"
        @pick="pick(s, $event)"
      />
    </TransitionGroup>

    <table v-else class="data-table" :aria-label="kind === 'film' ? 'Films' : 'TV Shows'" @keydown="arrowNav">
      <thead>
        <tr>
          <th v-for="c in tableColumns" :key="c.key" :class="c.class" :aria-sort="ariaSort(c.key)">
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

    <Transition name="rise">
      <div v-if="several" class="pointer-events-none sticky bottom-21 z-20 flex justify-center">
        <div class="selection-bar pointer-events-auto" role="toolbar" aria-label="Selected items">
          <span class="text-small tabular-nums" aria-live="polite">{{ pickedItems.length }} selected</span>
          <span class="w-13" />
          <button class="btn btn-ghost" :disabled="!pickedFilms.length" @click="markPicked">
            <Icon name="checkmark" /> {{ allWatched && pickedFilms.length ? "Mark unwatched" : "Mark watched" }}
          </button>
          <button class="btn btn-ghost" @click="revealPicked"><Icon name="folder" /> Show in Finder</button>
          <button class="btn btn-danger-ghost" @click="trashPicked"><Icon name="trash" /> Move to Trash</button>
          <span class="mx-(--space-5) h-21 w-px bg-line" />
          <Kbd keys="Esc" />
          <button class="btn-icon" aria-label="Clear selection" @click="release"><Icon name="close" /></button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.tile-grid {
  grid-template-columns: repeat(auto-fill, minmax(max(120px, calc((100% - (var(--columns) - 1) * 13px) / var(--columns))), 1fr));
}

.density {
  display: inline-flex;
  height: 40px;
  align-items: center;
  padding: 0 3px;
  border: 1px solid var(--c-line);
}
.density-value {
  min-width: 72px;
  text-align: center;
  font-size: var(--text-small);
  font-variant-numeric: tabular-nums;
}

.tile-move {
  transition: transform var(--dur-3) var(--ease-spring);
}
.tile-enter-active {
  transition:
    opacity var(--dur-3) var(--ease-out),
    translate var(--dur-3) var(--ease-out);
  transition-delay: calc(var(--i) * 21ms);
}
.tile-enter-from {
  opacity: 0;
  translate: 0 13px;
}
.tile-leave-active {
  position: absolute;
  transition: opacity var(--dur-2) var(--ease-out);
}
.tile-leave-to {
  opacity: 0;
}
</style>
