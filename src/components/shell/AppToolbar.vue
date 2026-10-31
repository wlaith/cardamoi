<script setup lang="ts">
import { computed } from "vue";
import lockupInk from "../../../brand/assets/logo/cardamoi-lockup-horizontal-red-on-ink.svg";
import lockupPaper from "../../../brand/assets/logo/cardamoi-lockup-horizontal-ink.svg";
import { back, canGoBack, canGoForward, forward, go, section } from "../../nav";
import { palette } from "../../ui";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
import TabList from "../ui/TabList.vue";
import StatusDisplay from "./StatusDisplay.vue";

type Place = "upnext" | "films" | "shows";
const places: { id: Place; label: string; title: string }[] = [
  { id: "upnext", label: "Up next", title: "Up Next (⌥⌘1)" },
  { id: "films", label: "Films", title: "Films (⌥⌘2)" },
  { id: "shows", label: "Shows", title: "TV Shows (⌥⌘3)" },
];

const place = computed<Place | null>({
  get: () => places.find((p) => p.id === section.value)?.id ?? null,
  set: (id) => id && go({ name: id }),
});
</script>

<template>
  <header class="header flex shrink-0 items-center gap-2 pr-13">
    <button class="header-brand" aria-label="cardamoi, Up Next" @click="go({ name: 'upnext' })">
      <img :src="lockupInk" alt="" class="logo-ink" draggable="false" />
      <img :src="lockupPaper" alt="" class="logo-paper" draggable="false" />
    </button>

    <div class="flex shrink-0">
      <button class="header-action" :disabled="!canGoBack" aria-label="Back" title="Back (⌘[)" @click="back">
        <Icon name="chevron-left" />
      </button>
      <button class="header-action" :disabled="!canGoForward" aria-label="Forward" title="Forward (⌘])" @click="forward">
        <Icon name="chevron-right" />
      </button>
    </div>

    <TabList v-model="place" :items="places" label="Library" nav class="self-stretch" />

    <div class="min-w-4 flex-1" />

    <button id="toolbar-search" class="header-search" aria-haspopup="dialog" :aria-expanded="palette" @click="palette = true">
      <Icon name="search" class="text-ink-2" />
      <span class="min-w-0 flex-1 truncate text-left text-ink-2">Search, or jump to anything</span>
      <Kbd keys="⌘F" />
    </button>

    <StatusDisplay />
    <button class="header-action" aria-label="Help" title="cardamoi Help (⇧⌘/)" @click="go({ name: 'help' })">
      <Icon name="help" />
    </button>
    <button class="header-action" aria-label="Preferences" title="Preferences (⌘,)" @click="go({ name: 'preferences', pane: 'general' })">
      <Icon name="settings" />
    </button>
  </header>
</template>
