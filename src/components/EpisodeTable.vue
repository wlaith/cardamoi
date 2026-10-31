<script setup lang="ts">
import { timeLeft } from "../api";
import { play, videoMenu } from "../commands";
import { arrowNav } from "../keys";
import { playedShare, type LocalEpisode } from "../library";
import { selection } from "../store";
import { percent } from "../status";
import { openMenu, openMenuBelow } from "../ui";
import Icon from "./ui/Icon.vue";
import ProgressBar from "./ui/ProgressBar.vue";

export interface EpisodeRowData {
  key: string;
  number: number;
  name: string;
  airDate?: string | null;
  still?: string | null;
  overview?: string;
  local?: LocalEpisode;
}

defineProps<{ rows: EpisodeRowData[]; selected: string | null }>();
const emit = defineEmits<{ select: [key: string] }>();

const today = new Date().toISOString().slice(0, 10);

function notAired(row: EpisodeRowData) {
  return !row.local && !!row.airDate && row.airDate > today;
}

function choose(row: EpisodeRowData) {
  emit("select", row.key);
  if (row.local) selection.value = { kind: "video", ep: row.local };
}

function activate(row: EpisodeRowData) {
  choose(row);
  const v = row.local?.video;
  if (v?.playable) play(row.local!, v.watched);
}

function menu(event: MouseEvent, row: EpisodeRowData) {
  choose(row);
  if (row.local) openMenu(event, videoMenu(row.local));
  else event.preventDefault();
}

function menuBelow(event: MouseEvent, row: EpisodeRowData) {
  choose(row);
  if (row.local) openMenuBelow(event.currentTarget as HTMLElement, videoMenu(row.local));
}

function synopsis(row: EpisodeRowData) {
  const first = row.overview?.split(". ")[0] ?? "";
  return first && !first.endsWith(".") ? `${first}.` : first;
}
</script>

<template>
  <table class="data-table table-fixed" aria-label="Episodes" @keydown="arrowNav">
    <thead>
      <tr>
        <th class="w-[72px]">No.</th>
        <th class="w-[128px]"><span class="sr-only">Still</span></th>
        <th>Episode</th>
        <th class="w-[128px]">Aired</th>
        <th class="w-[216px]">Status</th>
        <th class="w-14"><span class="sr-only">Menu</span></th>
      </tr>
    </thead>
    <tbody>
      <template v-for="row in rows" :key="row.key">
        <tr
          data-nav-item
          tabindex="0"
          :class="{ 'row-selected': selected === row.key, 'opacity-60': notAired(row) }"
          :aria-selected="selected === row.key"
          @click="choose(row)"
          @dblclick="activate(row)"
          @keydown.enter.prevent="activate(row)"
          @contextmenu="menu($event, row)"
        >
          <td class="tabular-nums">{{ String(row.number).padStart(2, "0") }}</td>
          <td class="py-2">
            <img v-if="row.still" :src="row.still" alt="" loading="lazy" class="poster-frame aspect-video w-24 object-cover" />
            <div v-else class="poster-frame aspect-video w-24" />
          </td>
          <td class="max-w-0">
            <p class="truncate font-semibold text-ink" :title="row.name">{{ row.name }}</p>
            <p v-if="synopsis(row)" class="mt-1 truncate text-small tabular-nums" :title="row.overview">{{ synopsis(row) }}</p>
          </td>
          <td class="tabular-nums">{{ row.airDate ?? "" }}</td>
          <td>
            <template v-if="!row.local">
              <span v-if="notAired(row)">Not out yet</span>
              <button v-else tabindex="-1" class="btn btn-ghost btn-sm -ml-4" @click.stop="choose(row)">
                Find release <Icon name="search" />
              </button>
            </template>
            <span v-else-if="row.local.video.watched" class="flex items-center gap-2"><Icon name="checkmark" /> Watched</span>
            <span v-else-if="row.local.video.resumeAt != null" class="flex items-center gap-3">
              <ProgressBar class="w-12" thin :value="playedShare(row.local.video)" :label="`Episode ${row.number} watched`" />
              {{ timeLeft(row.local.video) }}
            </span>
            <span v-else-if="row.local.video.playable" class="tag tag-green">Ready to watch</span>
            <span v-else-if="row.local.entry.status === 'paused'">Paused at {{ percent(row.local.entry) }} %</span>
            <span v-else class="flex items-center gap-2">
              <Icon name="download" /> Downloading · {{ percent(row.local.entry) }} %
            </span>
          </td>
          <td class="px-2 text-right">
            <button
              v-if="row.local"
              tabindex="-1"
              class="btn-icon btn-sm"
              :aria-label="`More for episode ${row.number}`"
              aria-haspopup="menu"
              @click.stop="menuBelow($event, row)"
            >
              <Icon name="overflow" />
            </button>
          </td>
        </tr>
        <tr v-if="selected === row.key && $slots.detail" class="detail-row">
          <td colspan="6" class="p-0">
            <slot name="detail" :row="row" />
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>
