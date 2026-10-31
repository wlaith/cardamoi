<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { episodeLabel, plainError, type LibraryEntry } from "../../api";
import { pause, resume, retry } from "../../commands";
import { go } from "../../nav";
import { librarySummary, unfinished } from "../../store";
import { canRetry, downloadStatus, percent } from "../../status";
import { downloadsOpen, sheet } from "../../ui";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
import ProgressBar from "../ui/ProgressBar.vue";

const props = defineProps<{ trigger: HTMLElement | undefined }>();

const SHOWN = 4;
const rows = computed(() => [...unfinished.value].sort((a, b) => b.added_at - a.added_at));
const moving = computed(() => rows.value.filter((e) => e.status === "downloading" || e.status === "starting"));
const paused = computed(() => rows.value.filter((e) => e.status === "paused"));
const speed = computed(() => moving.value.reduce((n, e) => n + e.speedMibps, 0));

const panel = ref<HTMLElement>();

function name(e: LibraryEntry) {
  const label = episodeLabel(e.season, e.episode);
  return label ? `${e.title} ${label}` : e.title;
}

function state(e: LibraryEntry) {
  if (e.status === "paused") return "paused";
  if (e.status === "error" || e.status === "missing") return "failed";
  return "active";
}

function close(refocus = true) {
  downloadsOpen.value = false;
  if (refocus) props.trigger?.focus();
}

function visit(to: () => void) {
  close(false);
  to();
}

function onOutside(event: PointerEvent) {
  const target = event.target as Node;
  if (panel.value?.contains(target) || props.trigger?.contains(target)) return;
  close(false);
}

watch(downloadsOpen, async (open) => {
  if (open) {
    document.addEventListener("pointerdown", onOutside, true);
    await nextTick();
    panel.value?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  } else {
    document.removeEventListener("pointerdown", onOutside, true);
  }
});
onBeforeUnmount(() => document.removeEventListener("pointerdown", onOutside, true));
</script>

<template>
  <Transition name="pop">
    <section
      v-if="downloadsOpen"
      ref="panel"
      class="popover absolute top-full right-0 z-40 mt-(--space-5) w-[420px]"
      role="dialog"
      aria-label="Downloads"
      @keydown.esc.stop.prevent="close()"
    >
      <header class="flex items-center gap-13 border-b border-subtle py-2 pr-2 pl-21">
        <h2 class="font-semibold">Downloads</h2>
        <p class="min-w-0 flex-1 truncate text-small tabular-nums text-ink-2">
          <template v-if="moving.length">{{ moving.length }} active · {{ speed.toFixed(1) }} MB/s</template>
          <template v-else-if="paused.length">{{ paused.length }} paused</template>
          <template v-else>Nothing downloading</template>
        </p>
        <button v-if="moving.length" class="btn btn-ghost" @click="moving.forEach(pause)"><Icon name="pause" /> Pause all</button>
        <button v-else-if="paused.length" class="btn btn-ghost" @click="paused.forEach(resume)"><Icon name="play" /> Resume all</button>
      </header>

      <p v-if="!rows.length" class="px-21 py-21 text-ink-2">Finished downloads are in your library.</p>
      <TransitionGroup v-else tag="ul" name="row" class="relative">
        <li v-for="e in rows.slice(0, SHOWN)" :key="e.id" class="flex items-center gap-13 border-b border-subtle py-13 pr-2 pl-21">
          <img v-if="e.poster" :src="e.poster" alt="" class="popover-poster" draggable="false" />
          <span v-else class="popover-poster" />
          <div class="min-w-0 flex-1">
            <p class="flex items-baseline gap-2">
              <span class="min-w-0 flex-1 truncate" :title="name(e)">{{ name(e) }}</span>
              <span class="text-small tabular-nums text-ink-2">{{ percent(e) }} %</span>
            </p>
            <ProgressBar class="mt-(--space-5)" :value="e.progress" :state="state(e)" :label="`${name(e)} progress`" />
            <p class="mt-(--space-5) truncate text-small tabular-nums text-ink-2" :class="{ '!text-error': state(e) === 'failed' }">
              {{ e.status === "error" && e.error ? plainError(e.error).message : downloadStatus(e) }}
            </p>
          </div>
          <button v-if="e.status === 'downloading' || e.status === 'starting'" class="btn-icon" :aria-label="`Pause ${name(e)}`" title="Pause" @click="pause(e)">
            <Icon name="pause" />
          </button>
          <button v-else-if="e.status === 'paused'" class="btn-icon" :aria-label="`Resume ${name(e)}`" title="Resume" @click="resume(e)">
            <Icon name="play" />
          </button>
          <button v-else-if="canRetry(e)" class="btn-icon" :aria-label="`Try ${name(e)} again`" title="Try again" @click="retry(e)">
            <Icon name="renew" />
          </button>
        </li>
      </TransitionGroup>

      <footer>
        <div class="flex items-center gap-2 py-(--space-5) pr-13 pl-13">
          <button class="btn btn-ghost" @click="visit(() => go({ name: 'downloads' }))">
            <Icon name="arrow-right" />
            {{ rows.length > SHOWN ? `All ${rows.length} downloads` : "Open Downloads" }}
          </button>
          <span class="flex-1" />
          <Kbd keys="⌥⌘4" />
        </div>
        <div class="flex items-center gap-2 border-t border-subtle py-(--space-5) pr-21 pl-13">
          <button class="btn btn-ghost" @click="visit(() => (sheet = { kind: 'magnet', link: '' }))">
            <Icon name="magnet" /> Add magnet link
          </button>
          <span class="min-w-0 flex-1 truncate text-right text-small tabular-nums text-ink-2">{{ librarySummary }}</span>
        </div>
      </footer>
    </section>
  </Transition>
</template>
