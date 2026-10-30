<script setup lang="ts">
import { computed } from "vue";
import { formatBytes } from "../../api";
import { go, lastQuery, section, type Route, type Section } from "../../nav";
import { entries, shelf, unfinished } from "../../store";
import { sheet } from "../../ui";
import Icon from "../ui/Icon.vue";

interface Item {
  section: Section;
  label: string;
  icon: string;
  count?: number;
  route: () => Route;
}

const films = computed(() => shelf.value.filter((s) => s.kind === "film").length);
const shows = computed(() => shelf.value.filter((s) => s.kind === "show").length);

const groups = computed<{ heading: string; items: Item[] }[]>(() => [
  {
    heading: "Library",
    items: [
      { section: "upnext", label: "Up Next", icon: "recently-viewed", route: () => ({ name: "upnext" }) },
      { section: "films", label: "Films", icon: "video", count: films.value, route: () => ({ name: "films" }) },
      { section: "shows", label: "TV Shows", icon: "screen", count: shows.value, route: () => ({ name: "shows" }) },
      { section: "downloads", label: "Downloads", icon: "download", count: unfinished.value.length, route: () => ({ name: "downloads" }) },
    ],
  },
  {
    heading: "Find",
    items: [
      {
        section: "search",
        label: lastQuery.value ? `“${lastQuery.value}”` : "Search",
        icon: "search",
        route: () => ({ name: "search", query: lastQuery.value }),
      },
    ],
  },
]);

const summary = computed(() => {
  if (!entries.value.length) return [];
  const size = formatBytes(entries.value.reduce((sum, e) => sum + e.totalBytes, 0));
  return [`${shows.value} ${shows.value === 1 ? "show" : "shows"} · ${films.value} ${films.value === 1 ? "film" : "films"}`, `${size} on disk`];
});
</script>

<template>
  <nav class="sidenav flex shrink-0 flex-col" aria-label="Library">
    <div class="flex-1 overflow-y-auto px-13 pt-21">
      <section v-for="g in groups" :key="g.heading" class="mb-34">
        <h2 class="sidenav-heading">{{ g.heading }}</h2>
        <ul>
          <li v-for="item in g.items" :key="item.section">
            <button class="sidenav-item" :aria-current="section === item.section ? 'page' : undefined" @click="go(item.route())">
              <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
              <span v-if="item.count" class="count tabular-nums">{{ item.count }}</span>
            </button>
          </li>
        </ul>
      </section>
    </div>
    <div class="mx-13 border-t border-subtle pt-13 pb-21">
      <button class="btn btn-ghost w-full justify-start" title="Add Magnet Link (⌘U)" @click="sheet = { kind: 'magnet', link: '' }">
        <Icon name="magnet" /> Add magnet link
      </button>
      <p v-for="line in summary" :key="line" class="truncate px-2 label-01 text-ink-2 first:mt-2">{{ line }}</p>
    </div>
  </nav>
</template>
