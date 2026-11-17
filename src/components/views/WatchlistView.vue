<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import globe from "../../../brand/assets/logo/cardamoi-globe-red.svg";
import type { Saved } from "../../api";
import { arrowNav } from "../../keys";
import { openMenu, palette } from "../../ui";
import { loadWatchlist, openSaved, saved, savedLine, unsave } from "../../watchlist";
import Icon from "../ui/Icon.vue";
import PageHeader from "../ui/PageHeader.vue";
import PosterTile from "../ui/PosterTile.vue";

const picked = ref("");
const keyOf = (t: Saved) => `${t.kind}:${t.id}`;
const subtitle = computed(() => `${saved.value.length} ${saved.value.length === 1 ? "title" : "titles"}`);

onMounted(loadWatchlist);

function onKey(event: KeyboardEvent) {
  arrowNav(event);
  const t = saved.value.find((s) => keyOf(s) === picked.value);
  if (t && event.key === "Backspace" && event.metaKey) {
    event.preventDefault();
    unsave(t);
  }
}

function menu(event: MouseEvent, t: Saved) {
  openMenu(event, [
    { label: "Open", action: () => openSaved(t) },
    { label: "Remove from Watchlist", action: () => unsave(t) },
  ]);
}
</script>

<template>
  <div v-if="!saved.length" class="space-y-55 p-34">
    <PageHeader title="Watchlist" />
    <div class="flex max-w-[754px] items-center gap-34 border border-subtle bg-layer p-34">
      <img :src="globe" alt="" class="size-[144px] shrink-0" draggable="false" />
      <div>
        <p class="text-heading">Nothing saved yet</p>
        <p class="mt-2 text-ink-2">
          Save films and shows you want to watch later, then find them here. Use Save to watchlist on a title's page, or
          right-click a search result.
        </p>
        <div class="mt-21">
          <button class="btn btn-primary btn-lg" @click="palette = true">Search for something <Icon name="search" /></button>
        </div>
      </div>
    </div>
  </div>

  <div v-else class="space-y-55 p-34" @keydown="onKey">
    <PageHeader title="Watchlist" :subtitle="subtitle" />
    <ul class="grid grid-cols-[repeat(auto-fill,minmax(144px,1fr))] gap-x-13 gap-y-21">
      <PosterTile
        v-for="t in saved"
        :key="keyOf(t)"
        :poster="t.poster"
        :title="t.title"
        :line="savedLine(t)"
        :selected="picked === keyOf(t)"
        @select="picked = keyOf(t)"
        @open="openSaved(t)"
        @menu="(e) => menu(e, t)"
      />
    </ul>
  </div>
</template>
