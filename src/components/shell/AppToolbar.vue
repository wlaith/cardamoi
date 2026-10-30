<script setup lang="ts">
import { ref, watch } from "vue";
import lockupInk from "../../../brand/assets/logo/cardamoi-lockup-horizontal-red-on-ink.svg";
import lockupPaper from "../../../brand/assets/logo/cardamoi-lockup-horizontal-ink.svg";
import { go, route } from "../../nav";
import Icon from "../ui/Icon.vue";
import StatusDisplay from "./StatusDisplay.vue";

const query = ref("");
watch(route, (r) => {
  if (r.name === "search") query.value = r.query;
});

function search() {
  const text = query.value.trim();
  if (text) go({ name: "search", query: text });
}
</script>

<template>
  <header class="header flex shrink-0 items-stretch">
    <button class="header-brand" aria-label="cardamoi, Up Next" @click="go({ name: 'upnext' })">
      <img :src="lockupInk" alt="" class="logo-ink" draggable="false" />
      <img :src="lockupPaper" alt="" class="logo-paper" draggable="false" />
    </button>

    <form role="search" class="header-search relative ml-34 flex w-[377px] min-w-0 shrink items-center" @submit.prevent="search">
      <Icon name="search" class="pointer-events-none absolute left-13 text-ink-2" />
      <label for="toolbar-search" class="sr-only">Search films and shows</label>
      <input
        id="toolbar-search"
        v-model="query"
        type="search"
        placeholder="Search films and shows"
        autocomplete="off"
        spellcheck="false"
        class="h-full w-full bg-transparent pr-13 pl-[37px]"
        @keydown.esc="query = ''"
      />
    </form>

    <div class="min-w-4 flex-1" />
    <StatusDisplay />
    <button class="header-action" aria-label="Help" title="cardamoi Help" @click="go({ name: 'help' })">
      <Icon name="help" />
    </button>
    <button class="header-action mr-13" aria-label="Preferences" title="Preferences (⌘,)" @click="go({ name: 'preferences', pane: 'general' })">
      <Icon name="settings" />
    </button>
  </header>
</template>
