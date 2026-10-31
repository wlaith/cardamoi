<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api } from "../../api";
import { go } from "../../nav";
import { keyFormat, loadTmdbStatus, openTmdb, TMDB_API_SETTINGS, tmdbStatus } from "../../tmdb";
import { alertSheet } from "../../ui";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import SectionHeader from "../ui/SectionHeader.vue";

const key = ref("");
const saving = ref(false);
const saveError = ref<unknown>(null);
const offline = ref(false);
const saved = ref(false);

loadTmdbStatus();

const format = computed(() => keyFormat(key.value));
const usable = computed(() => format.value === "token" || format.value === "key");
watch(key, () => {
  saveError.value = null;
  offline.value = false;
  saved.value = false;
});

function ago(seconds: number) {
  const minutes = Math.round((Date.now() / 1000 - seconds) / 60);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} ${hours === 1 ? "hour" : "hours"} ago`;
  return new Date(seconds * 1000).toLocaleDateString();
}

const statusLine = computed(() => {
  const s = tmdbStatus.value;
  if (!s) return "";
  if (s.state === "missing") return "Not set up";
  if (s.state === "rejected") return "Key turned down by TMDB";
  const kind = s.kind === "token" ? "read access token" : "API key";
  return ["Connected", kind, s.checkedAt ? `checked ${ago(s.checkedAt)}` : "not checked yet"].join(" · ");
});

async function save(skipCheck = false) {
  if (!usable.value) return;
  saving.value = true;
  saveError.value = null;
  offline.value = false;
  try {
    await api.saveTmdbKey(key.value, skipCheck);
    key.value = "";
    saved.value = true;
    await loadTmdbStatus();
  } catch (e) {
    offline.value = String(e).includes("tmdb:unreachable");
    saveError.value = e;
  } finally {
    saving.value = false;
  }
}

async function remove() {
  const choice = await alertSheet({
    title: "Remove the TMDB key?",
    message: "Search, artwork and matching stop until you add a key again. Your library and downloads stay as they are.",
    buttons: [
      { label: "Cancel", value: "cancel" },
      { label: "Remove Key", value: "remove", kind: "danger" },
    ],
  });
  if (choice !== "remove") return;
  await api.removeTmdbKey().catch((e) => (saveError.value = e));
  await loadTmdbStatus();
}
</script>

<template>
  <form class="max-w-xl space-y-34" @submit.prevent="save()">
    <section class="space-y-21" aria-labelledby="tmdb-heading">
      <SectionHeader id="tmdb-heading" title="TMDB">
        <template #action>
          <button type="button" class="btn btn-ghost btn-sm" @click="go({ name: 'help', topic: 'tmdb' })">Help <Icon name="help" /></button>
        </template>
      </SectionHeader>
      <p class="-mt-4 text-ink-2">Film and show information, artwork and matching come from TMDB, using your own free key.</p>

      <p class="flex items-center gap-2" :class="{ 'text-red-text': tmdbStatus?.state === 'rejected' }" role="status">
        <Icon
          :name="tmdbStatus?.state === 'ok' ? 'checkmark-filled' : tmdbStatus?.state === 'rejected' ? 'warning-filled' : 'key'"
        />
        <span class="tabular-nums">{{ statusLine }}</span>
      </p>

      <div>
        <label for="tmdb-pref-key" class="field-label">{{ tmdbStatus?.state === "missing" ? "Key" : "Replace key" }}</label>
        <input
          id="tmdb-pref-key"
          v-model="key"
          type="password"
          autocomplete="off"
          spellcheck="false"
          :placeholder="tmdbStatus?.state === 'missing' ? '' : 'Saved'"
          class="field"
          :aria-invalid="format === 'unknown'"
        />
        <p :class="format === 'unknown' ? 'field-error' : 'field-helper'">
          <template v-if="format === 'unknown'">This doesn't look like a TMDB key.</template>
          <template v-else-if="format === 'token'">Read access token</template>
          <template v-else-if="format === 'key'">API key</template>
          <template v-else>
            The API Read Access Token from
            <button type="button" class="link" @click="openTmdb(TMDB_API_SETTINGS)">your TMDB API settings</button>.
          </template>
        </p>
      </div>
    </section>

    <ErrorNote v-if="saveError" :error="saveError" />
    <div class="flex flex-wrap items-center gap-4">
      <button type="submit" class="btn btn-primary" :disabled="saving || !usable">{{ saving ? "Checking" : "Save" }}</button>
      <button v-if="offline" type="button" class="btn btn-secondary" @click="save(true)">Save anyway</button>
      <button v-if="tmdbStatus && tmdbStatus.state !== 'missing'" type="button" class="btn btn-danger" @click="remove">Remove Key</button>
      <button type="button" class="btn btn-ghost" @click="go({ name: 'setup', step: 'get' })">Walk me through it</button>
      <p v-if="saved && !saveError" class="flex items-center gap-2 text-ok" role="status"><Icon name="checkmark" /> Saved</p>
    </div>
  </form>
</template>
