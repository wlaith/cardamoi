<script setup lang="ts">
import globe from "../../../brand/assets/logo/cardamoi-globe-red.svg";
import { openFolder } from "../../commands";
import { entries, homeMode } from "../../store";
import { palette } from "../../ui";
import Icon from "../ui/Icon.vue";
import UpNextShelves from "./UpNextShelves.vue";
import UpNextStack from "./UpNextStack.vue";

</script>

<template>
  <div v-if="!entries.length" class="grid h-full place-items-center p-34">
    <div class="flex max-w-[754px] items-center gap-34 border border-subtle bg-layer p-34">
      <img :src="globe" alt="" class="size-[144px] shrink-0" draggable="false" />
      <div>
      <p class="text-heading">Nothing to watch yet</p>
      <p class="mt-2 text-ink-2">
        Search for a film or show and download it, or put video files you already have in the cardamoi folder in Movies. Either
        way, they show up here ready to watch.
      </p>
      <div class="mt-21 flex gap-13">
        <button class="btn btn-primary btn-lg" @click="palette = true">Search for something <Icon name="search" /></button>
        <button class="btn btn-ghost btn-lg" @click="openFolder">Open folder <Icon name="folder" /></button>
      </div>
      </div>
    </div>
  </div>
  <UpNextStack v-else-if="homeMode === 'stack'" />
  <UpNextShelves v-else />
</template>
