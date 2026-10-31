<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { checkFolder, openFilm, openFolder, openShow } from "../../commands";
import { filmVideo, showKey, type Shelf } from "../../library";
import { go } from "../../nav";
import { homeMode, shelf, theme } from "../../store";
import { tmdbReady } from "../../tmdb";
import { filmLine, showLine } from "../../status";
import { palette, sheet } from "../../ui";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";

interface Row {
  id: string;
  label: string;
  meta?: string;
  poster?: string | null;
  icon?: string;
  keys?: string;
  run: () => void;
}
interface Group {
  name: string;
  rows: Row[];
}

const query = ref("");
const active = ref(0);
const field = ref<HTMLInputElement>();
let returnFocus: HTMLElement | null = null;

const words = computed(() => query.value.trim().toLowerCase().split(/\s+/).filter(Boolean));
const matches = (text: string) => words.value.every((w) => text.toLowerCase().includes(w));

function titleOf(s: Shelf) {
  return s.kind === "film" ? s.entry.title : s.show.title;
}

const library = computed<Row[]>(() => {
  if (!words.value.length) return [];
  return shelf.value
    .filter((s) => matches(titleOf(s)))
    .sort((a, b) => Number(!titleOf(a).toLowerCase().startsWith(words.value[0])) - Number(!titleOf(b).toLowerCase().startsWith(words.value[0])))
    .slice(0, 6)
    .map((s) =>
      s.kind === "film"
        ? {
            id: `film:${s.entry.id}`,
            label: s.entry.title,
            meta: ["Film", filmLine(s.entry).text].filter(Boolean).join(" · "),
            poster: s.entry.poster,
            run: () => {
              const ep = filmVideo(s.entry);
              if (ep) openFilm(ep);
              else go({ name: "downloads" });
            },
          }
        : {
            id: showKey(s.show),
            label: s.show.title,
            meta: ["Show", showLine(s.show)].filter(Boolean).join(" · "),
            poster: s.show.poster,
            run: () => openShow(s.show),
          },
    );
});

const commands = computed<Row[]>(() => {
  const all: Row[] = [
    { id: "go-upnext", label: "Go to Up Next", icon: "arrow-right", keys: "⌥⌘1", run: () => go({ name: "upnext" }) },
    { id: "go-films", label: "Go to Films", icon: "arrow-right", keys: "⌥⌘2", run: () => go({ name: "films" }) },
    { id: "go-shows", label: "Go to TV Shows", icon: "arrow-right", keys: "⌥⌘3", run: () => go({ name: "shows" }) },
    { id: "go-downloads", label: "Go to Downloads", icon: "arrow-right", keys: "⌥⌘4", run: () => go({ name: "downloads" }) },
    { id: "magnet", label: "Add magnet link", icon: "magnet", keys: "⌘U", run: () => (sheet.value = { kind: "magnet", link: "" }) },
    { id: "folder", label: "Open the library folder", icon: "folder", run: openFolder },
    { id: "scan", label: "Check the folder for new videos", icon: "renew", run: checkFolder },
    {
      id: "home",
      label: homeMode.value === "stack" ? "Show Up Next as a grid" : "Show Up Next as a stack",
      icon: homeMode.value === "stack" ? "grid" : "stack",
      run: () => (homeMode.value = homeMode.value === "stack" ? "grid" : "stack"),
    },
    ...(["gray", "ink", "paper"] as const)
      .filter((t) => t !== theme.value)
      .map((t) => ({
        id: `theme-${t}`,
        label: `Use the ${{ gray: "Gray", ink: "Ink", paper: "Paper" }[t]} appearance`,
        icon: "settings",
        run: () => (theme.value = t),
      })),
    tmdbReady.value
      ? { id: "tmdb", label: "TMDB key", icon: "key", run: () => go({ name: "preferences", pane: "tmdb" }) }
      : { id: "tmdb", label: "Set up TMDB", icon: "key", run: () => go({ name: "setup" }) },
    { id: "prefs", label: "Preferences", icon: "settings", keys: "⌘,", run: () => go({ name: "preferences", pane: "general" }) },
    { id: "help", label: "cardamoi Help", icon: "help", keys: "⇧⌘/", run: () => go({ name: "help" }) },
  ];
  return words.value.length ? all.filter((c) => matches(c.label)).slice(0, 5) : all.slice(0, 7);
});

const online = computed<Row[]>(() =>
  words.value.length
    ? [
        {
          id: "search",
          label: `Search for releases of “${query.value.trim()}”`,
          icon: "search",
          keys: "⌘↵",
          run: () => go({ name: "search", query: query.value.trim() }),
        },
      ]
    : [],
);

const groups = computed<Group[]>(() =>
  [
    { name: "Library", rows: library.value },
    { name: "Commands", rows: commands.value },
    { name: "Online", rows: online.value },
  ].filter((g) => g.rows.length),
);
const flat = computed(() => groups.value.flatMap((g) => g.rows));

watch(query, () => (active.value = 0));

watch(palette, async (open) => {
  if (open) {
    returnFocus = document.activeElement as HTMLElement | null;
    query.value = "";
    active.value = 0;
    await nextTick();
    field.value?.focus();
  } else {
    returnFocus?.focus();
  }
});

function close() {
  palette.value = false;
}

function run(row: Row | undefined) {
  if (!row) return;
  returnFocus = null;
  close();
  row.run();
}

function jumpGroup(step: 1 | -1) {
  const starts = groups.value.map((g) => flat.value.indexOf(g.rows[0]));
  const here = starts.filter((s) => s <= active.value).length - 1;
  const to = (here + step + starts.length) % starts.length;
  active.value = starts[to];
}

function onKey(event: KeyboardEvent) {
  const n = flat.value.length;
  if (event.key === "Escape") close();
  else if (event.key === "ArrowDown" && n) active.value = (active.value + 1) % n;
  else if (event.key === "ArrowUp" && n) active.value = (active.value - 1 + n) % n;
  else if (event.key === "Tab" && groups.value.length > 1) jumpGroup(event.shiftKey ? -1 : 1);
  else if (event.key === "Enter" && event.metaKey) run(online.value[0]);
  else if (event.key === "Enter") run(flat.value[active.value]);
  else return;
  event.preventDefault();
  event.stopPropagation();
  nextTick(() => document.getElementById(`palette-${flat.value[active.value]?.id}`)?.scrollIntoView({ block: "nearest" }));
}
</script>

<template>
  <Transition name="veil">
    <div v-if="palette" class="scrim fixed inset-0 z-50" @mousedown.self="close">
      <Transition name="palette" appear>
        <div class="palette mx-auto mt-89 w-[640px] max-w-[calc(100vw-42px)]" role="dialog" aria-modal="true" aria-label="Search and commands" @keydown="onKey">
          <div class="flex h-[55px] items-center gap-13 border-b border-subtle pr-13 pl-21">
            <Icon name="search" :size="20" />
            <input
              ref="field"
              v-model="query"
              class="h-full min-w-0 flex-1 bg-transparent text-heading outline-none placeholder:text-ink-2"
              placeholder="Search, or jump to anything"
              role="combobox"
              aria-expanded="true"
              aria-controls="palette-list"
              :aria-activedescendant="flat[active] ? `palette-${flat[active].id}` : undefined"
              autocomplete="off"
              spellcheck="false"
            />
            <Kbd keys="Esc" />
          </div>

          <div id="palette-list" class="max-h-[min(480px,60vh)] overflow-y-auto py-(--space-5)" role="listbox" aria-label="Results">
            <p v-if="!flat.length" class="px-21 py-13 text-ink-2">Nothing matches “{{ query.trim() }}”.</p>
            <section v-for="g in groups" :key="g.name" role="group" :aria-label="g.name">
              <h3 class="eyebrow px-21 pt-13 pb-(--space-5)">{{ g.name }}</h3>
              <div
                v-for="row in g.rows"
                :id="`palette-${row.id}`"
                :key="row.id"
                class="palette-row"
                role="option"
                :aria-selected="flat[active] === row"
                @pointermove="active = flat.indexOf(row)"
                @click="run(row)"
              >
                <img v-if="row.poster" :src="row.poster" alt="" class="palette-thumb" draggable="false" />
                <span v-else-if="g.name === 'Library'" class="palette-thumb" />
                <Icon v-else :name="row.icon ?? 'arrow-right'" class="text-ink-2" />
                <span class="min-w-0 flex-1 truncate">{{ row.label }}</span>
                <span v-if="row.meta" class="max-w-[45%] truncate text-small tabular-nums text-ink-2">{{ row.meta }}</span>
                <Kbd v-if="row.keys" :keys="row.keys" />
                <Icon v-if="flat[active] === row" name="enter" class="text-ink-2" />
              </div>
            </section>
          </div>

          <footer class="flex h-10 items-center gap-21 border-t border-subtle px-21 text-ink-2">
            <span class="flex items-center gap-2"><Kbd keys="↑↓" /> move</span>
            <span class="flex items-center gap-2"><Kbd keys="↵" /> open</span>
            <span class="flex items-center gap-2"><Kbd keys="⌘↵" /> search releases</span>
            <span class="flex items-center gap-2"><Kbd keys="Tab" /> next group</span>
          </footer>
        </div>
      </Transition>
    </div>
  </Transition>
</template>

<style scoped>
input:focus-visible {
  outline: none;
}
</style>
