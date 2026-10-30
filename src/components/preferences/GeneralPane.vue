<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "../../api";
import { checkFolder, openFolder } from "../../commands";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import SectionHeader from "../ui/SectionHeader.vue";

const autoplayNext = ref<boolean | null>(null);
const error = ref<unknown>(null);

onMounted(async () => {
  try {
    autoplayNext.value = (await api.playbackSettings()).autoplayNext;
  } catch (e) {
    error.value = e;
  }
});

async function save() {
  if (autoplayNext.value == null) return;
  try {
    await api.savePlaybackSettings({ autoplayNext: autoplayNext.value });
    error.value = null;
  } catch (e) {
    error.value = e;
    autoplayNext.value = !autoplayNext.value;
  }
}
</script>

<template>
  <div class="max-w-[640px] space-y-55">
    <section class="space-y-21" aria-labelledby="folder-heading">
      <SectionHeader
        id="folder-heading"
        title="Library folder"
        subtitle="Videos you put in the cardamoi folder in Movies show up in your library within a minute."
      />
      <div class="flex gap-4">
        <button class="btn btn-secondary" @click="openFolder">Open folder <Icon name="folder" /></button>
        <button class="btn btn-ghost" @click="checkFolder">Check for new videos <Icon name="renew" /></button>
      </div>
    </section>

    <section class="space-y-21" aria-labelledby="playback-heading">
      <SectionHeader id="playback-heading" title="Playback" />
      <label v-if="autoplayNext != null" class="flex items-start gap-4">
        <input v-model="autoplayNext" type="checkbox" class="toggle" @change="save" />
        <span>
          Play the next episode automatically
          <span class="mt-1 block helper-text-01">
            When an episode ends and the next one is downloaded, it starts after 10 seconds. Press Esc to stay, or Return
            to start it straight away.
          </span>
        </span>
      </label>
      <ErrorNote v-if="error" :error="error" />
    </section>
  </div>
</template>
