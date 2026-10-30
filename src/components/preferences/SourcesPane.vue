<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api, type ProwlarrStatus, type SourceRow } from "../../api";
import { go } from "../../nav";
import { announce } from "../../store";
import { alertSheet, openMenuBelow } from "../../ui";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import SectionHeader from "../ui/SectionHeader.vue";
import Spinner from "../ui/Spinner.vue";

const sources = ref<SourceRow[]>([]);
const listError = ref<unknown>(null);

const name = ref("");
const url = ref("");
const apiKey = ref("");
const adding = ref(false);
const addError = ref<unknown>(null);

const urlHint = computed(() => {
  const value = url.value.trim();
  if (!value) return "";
  return /^https?:\/\/\S+$/.test(value) ? "" : "The feed URL starts with http:// or https://";
});

async function load() {
  try {
    sources.value = await api.listSources();
    listError.value = null;
  } catch (e) {
    listError.value = e;
  }
}
onMounted(load);

const prowlarr = ref<ProwlarrStatus>({ state: "starting" });
const prowlarrError = ref<unknown>(null);

async function loadProwlarr() {
  try {
    prowlarr.value = await api.prowlarrStatus();
  } catch (e) {
    prowlarr.value = { state: "failed", message: String(e) };
  }
}

let prowlarrTimer: number | undefined;
onMounted(() => {
  loadProwlarr();
  prowlarrTimer = window.setInterval(loadProwlarr, 2000);
});
onUnmounted(() => window.clearInterval(prowlarrTimer));

async function openProwlarr() {
  prowlarrError.value = null;
  try {
    await api.openProwlarr();
  } catch (e) {
    prowlarrError.value = e;
  }
}

async function add() {
  if (urlHint.value) return;
  adding.value = true;
  addError.value = null;
  try {
    await api.addSource(name.value, url.value.trim(), apiKey.value);
    name.value = url.value = apiKey.value = "";
    await load();
  } catch (e) {
    addError.value = e;
  } finally {
    adding.value = false;
  }
}

async function toggle(s: SourceRow) {
  try {
    await api.setSourceEnabled(s.id, !s.enabled);
    await load();
  } catch (e) {
    listError.value = e;
  }
}

function clear() {
  name.value = url.value = apiKey.value = "";
  addError.value = null;
}

function prowlarrMenu(event: MouseEvent) {
  openMenuBelow(event.currentTarget as HTMLElement, [
    { label: "Manage Indexers…", action: openProwlarr, disabled: prowlarr.value.state !== "ready" },
    { label: "Help with Sources", action: () => go({ name: "help", topic: "sources" }) },
  ]);
}

function sourceMenu(event: MouseEvent, s: SourceRow) {
  openMenuBelow(event.currentTarget as HTMLElement, [
    { label: s.enabled ? "Switch Off" : "Switch On", action: () => toggle(s) },
    "separator",
    { label: "Remove…", action: () => remove(s), danger: true },
  ]);
}

async function remove(s: SourceRow) {
  const choice = await alertSheet({
    title: `Remove "${s.name}"?`,
    message: "Searches stop using it. To add it back later you'll need its feed URL and API key again.",
    buttons: [
      { label: "Cancel", value: "cancel" },
      { label: "Remove", value: "remove", kind: "danger" },
    ],
  });
  if (choice !== "remove") return;
  try {
    await api.removeSource(s.id);
    await load();
    announce(`Removed "${s.name}"`);
  } catch (e) {
    listError.value = e;
  }
}
</script>

<template>
  <div class="space-y-55">
    <section class="space-y-21" aria-labelledby="sources-heading">
      <SectionHeader id="sources-heading" title="Sources" subtitle="Every search checks the sources that are switched on.">
        <template #action>
          <button class="btn btn-ghost btn-sm" @click="go({ name: 'help', topic: 'sources' })">Help <Icon name="help" /></button>
        </template>
      </SectionHeader>
      <ErrorNote v-if="listError" :error="listError" />

      <ul>
        <li class="flex items-center gap-34 border-b border-subtle px-4 py-3">
          <label class="flex w-20 shrink-0 items-center gap-2">
            <input type="checkbox" class="toggle" checked disabled aria-label="Internet Archive is always on" /> On
          </label>
          <div class="min-w-0 flex-1">
            <p class="heading-compact-01">Internet Archive</p>
            <p class="mt-1 truncate label-01 text-ink-2">Built in · public domain and freely licensed films</p>
          </div>
          <span class="tag tag-blue w-36 justify-center">Built in</span>
          <span class="w-8" />
        </li>
        <li class="border-b border-subtle px-4 py-3">
          <div class="flex items-center gap-34">
            <label class="flex w-20 shrink-0 items-center gap-2">
              <input type="checkbox" class="toggle" checked disabled aria-label="Prowlarr is always on" /> On
            </label>
            <div class="min-w-0 flex-1">
              <p class="heading-compact-01">Prowlarr</p>
              <p class="mt-1 truncate label-01 text-ink-2">Built in · searches the indexers you add to it</p>
            </div>
            <span class="w-36">
              <span v-if="prowlarr.state === 'starting'" class="tag gap-2"><Spinner :size="12" /> Starting</span>
              <span v-else-if="prowlarr.state === 'failed'" class="tag tag-red">Not responding</span>
              <span v-else class="tag tag-green">Running</span>
            </span>
            <button class="btn-icon btn-sm" aria-label="More for Prowlarr" aria-haspopup="menu" @click="prowlarrMenu"><Icon name="overflow" /></button>
          </div>
          <ErrorNote v-if="prowlarr.state === 'failed'" :error="prowlarr.message" class="mt-3" />
          <ErrorNote v-if="prowlarrError" :error="prowlarrError" class="mt-3" />
        </li>
        <li v-for="s in sources" :key="s.id" class="flex items-center gap-34 border-b border-subtle px-4 py-3">
          <label class="flex w-20 shrink-0 items-center gap-2">
            <input type="checkbox" class="toggle" :checked="s.enabled" :aria-label="`Use ${s.name} in searches`" @change="toggle(s)" />
            {{ s.enabled ? "On" : "Off" }}
          </label>
          <div class="min-w-0 flex-1">
            <p class="heading-compact-01" :class="{ 'text-ink-2': !s.enabled }">{{ s.name }}</p>
            <p class="mt-1 truncate label-01 text-ink-2 selectable" :title="s.url">{{ s.url }}</p>
          </div>
          <span class="w-36"><span class="tag" :class="s.enabled ? 'tag-green' : ''">{{ s.enabled ? "On" : "Off" }}</span></span>
          <button class="btn-icon btn-sm" :aria-label="`More for ${s.name}`" aria-haspopup="menu" @click="sourceMenu($event, s)">
            <Icon name="overflow" />
          </button>
        </li>
      </ul>
    </section>

    <section class="space-y-21" aria-labelledby="add-heading">
      <SectionHeader
        id="add-heading"
        title="Add a Torznab source"
        subtitle="For a Prowlarr or Jackett you run yourself. The connection is tested before it's saved."
      />
      <form class="w-[560px] max-w-full space-y-21" @submit.prevent="add">
        <div>
          <label for="src-name" class="field-label">Name</label>
          <input id="src-name" v-model="name" required placeholder="Jackett" class="field" />
          <p class="field-helper">Shown next to its results.</p>
        </div>
        <div>
          <label for="src-url" class="field-label">Feed URL</label>
          <div class="relative">
            <input
              id="src-url"
              v-model="url"
              required
              placeholder="http://localhost:9117/api/v2.0/indexers/all/results/torznab/"
              spellcheck="false"
              class="field pr-12"
              :aria-invalid="!!urlHint"
              aria-describedby="src-url-help"
            />
            <Icon v-if="urlHint" name="warning-filled" class="absolute top-3 right-4 text-alarm" />
          </div>
          <p id="src-url-help" :class="urlHint ? 'field-error' : 'field-helper'">{{ urlHint || "The Torznab feed, all indexers." }}</p>
        </div>
        <div>
          <label for="src-key" class="field-label">API key</label>
          <input id="src-key" v-model="apiKey" type="password" autocomplete="off" class="field" />
          <p class="field-helper">On Jackett's dashboard, top right.</p>
        </div>
        <ErrorNote v-if="addError" :error="addError" title="Couldn't add it." />
        <div class="flex items-center gap-4">
          <button type="submit" class="btn btn-primary" :disabled="adding || !!urlHint">
            {{ adding ? "Testing connection" : "Test and add" }}
            <Spinner v-if="adding" />
          </button>
          <button type="button" class="btn btn-ghost" @click="clear">Cancel</button>
        </div>
      </form>
    </section>
  </div>
</template>
