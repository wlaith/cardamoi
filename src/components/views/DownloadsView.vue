<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api, episodeLabel, plainError, type LibraryEntry } from "../../api";
import { moveToTrash, openFolder, pause, resume, retry, reveal } from "../../commands";
import { arrowNav } from "../../keys";
import { go } from "../../nav";
import { unfinished } from "../../store";
import { canRetry, downloadStatus } from "../../status";
import Icon from "../ui/Icon.vue";
import PageHeader from "../ui/PageHeader.vue";
import ProgressBar from "../ui/ProgressBar.vue";

const rows = computed(() => [...unfinished.value].sort((a, b) => b.added_at - a.added_at));
const moving = computed(() => rows.value.filter((e) => e.status === "downloading" || e.status === "starting"));
const paused = computed(() => rows.value.filter((e) => e.status === "paused"));
const failed = computed(() => rows.value.filter((e) => e.status === "error" || e.status === "missing"));

const subtitle = computed(() => {
  const parts: string[] = [];
  if (moving.value.length) {
    const speed = moving.value.reduce((n, e) => n + e.speedMibps, 0);
    parts.push(`${moving.value.length} downloading`, `${speed.toFixed(1)} MB/s`);
  }
  if (paused.value.length) parts.push(`${paused.value.length} paused`);
  if (failed.value.length) parts.push(`${failed.value.length} failed`);
  return parts.join(" · ");
});

const prowlarrDown = ref(false);
onMounted(async () => {
  try {
    prowlarrDown.value = (await api.prowlarrStatus()).state === "failed";
  } catch {
    prowlarrDown.value = true;
  }
});

function name(e: LibraryEntry) {
  const label = episodeLabel(e.season, e.episode);
  return label ? `${e.title} ${label}` : e.title;
}

function state(e: LibraryEntry) {
  if (e.status === "paused") return "paused";
  if (e.status === "error" || e.status === "missing") return "failed";
  return "active";
}

function status(e: LibraryEntry) {
  return e.status === "error" && e.error ? plainError(e.error).message : downloadStatus(e);
}
</script>

<template>
  <div class="space-y-21 p-34">
    <PageHeader title="Downloads" :subtitle="subtitle || 'Finished downloads are in your library.'">
      <template #actions>
        <button v-if="moving.length || !paused.length" class="btn btn-secondary" :disabled="!moving.length" @click="moving.forEach(pause)">
          Pause all <Icon name="pause" />
        </button>
        <button v-else class="btn btn-secondary" @click="paused.forEach(resume)">Resume all <Icon name="play" /></button>
        <button class="btn btn-ghost" @click="openFolder">Open folder <Icon name="folder" /></button>
      </template>
    </PageHeader>

    <div v-if="prowlarrDown" class="notification notification-error" role="alert">
      <Icon name="error-filled" class="notification-icon" />
      <div class="min-w-0 flex-1">
        <p class="font-semibold">Couldn't reach Prowlarr</p>
        <p>
          Searches skip it until it's running. Check it in
          <button class="link" @click="go({ name: 'preferences', pane: 'sources' })">Preferences › Sources</button>.
        </p>
      </div>
      <button class="btn-icon btn-sm" aria-label="Dismiss" @click="prowlarrDown = false"><Icon name="close" /></button>
    </div>

    <div v-if="!rows.length" class="py-16">
      <p class="text-heading">No downloads</p>
      <p class="mt-4 text-ink-2">Find something to watch with the search field at the top of the window.</p>
    </div>

    <ul v-else class="pt-6" aria-label="Downloads" @keydown="arrowNav">
      <li v-for="e in rows" :key="e.id" class="flex items-center gap-4 border-b border-subtle p-4">
        <img v-if="e.poster" :src="e.poster" alt="" class="poster-frame h-[72px] w-12 shrink-0 object-cover" />
        <div v-else class="poster-frame h-[72px] w-12 shrink-0" />
        <div class="min-w-0 flex-1">
          <p class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate" :title="name(e)">{{ name(e) }}</span>
            <Icon v-if="state(e) === 'failed'" name="error-filled" class="text-alarm" label="Failed" />
          </p>
          <ProgressBar class="mt-2" :value="e.progress" :state="state(e)" :label="`${name(e)} progress`" />
          <p class="mt-2 truncate text-ink-2" :class="{ '!text-error': state(e) === 'failed' }" :title="e.error ?? ''">
            {{ status(e) }}
          </p>
        </div>
        <div class="flex shrink-0">
          <button
            v-if="e.status === 'downloading' || e.status === 'starting'"
            data-nav-item
            class="btn-icon"
            :aria-label="`Pause ${name(e)}`"
            title="Pause"
            @click="pause(e)"
          >
            <Icon name="pause" />
          </button>
          <button v-else-if="e.status === 'paused'" data-nav-item class="btn-icon" :aria-label="`Resume ${name(e)}`" title="Resume" @click="resume(e)">
            <Icon name="play" />
          </button>
          <button v-else-if="canRetry(e)" data-nav-item class="btn-icon" :aria-label="`Try ${name(e)} again`" title="Try again" @click="retry(e)">
            <Icon name="renew" />
          </button>
          <span v-else class="w-10" />
          <button
            class="btn-icon"
            :disabled="e.status === 'missing' || e.status === 'resolving'"
            :aria-label="`Show ${name(e)} in Finder`"
            title="Show in Finder"
            @click="reveal(e)"
          >
            <Icon name="folder" />
          </button>
          <button class="btn-icon" :aria-label="`Remove ${name(e)}`" title="Cancel and move to Trash" @click="moveToTrash(e)">
            <Icon name="close" />
          </button>
        </div>
      </li>
    </ul>
  </div>
</template>
