<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api, languageName, SUBTITLE_LANGUAGES, systemLanguage, type SubtitleSettings } from "../../api";
import { go } from "../../nav";
import ErrorNote from "../ui/ErrorNote.vue";
import Icon from "../ui/Icon.vue";
import SectionHeader from "../ui/SectionHeader.vue";

const settings = ref<SubtitleSettings | null>(null);
const password = ref("");
const loadError = ref<unknown>(null);
const saving = ref(false);
const saveError = ref<unknown>(null);
const saved = ref(false);
const adding = ref("");

onMounted(async () => {
  try {
    const s = await api.subtitleSettings();
    if (s.languages.length === 0) s.languages = [systemLanguage()];
    settings.value = s;
  } catch (e) {
    loadError.value = e;
  }
});

const addable = computed(() => SUBTITLE_LANGUAGES.filter((l) => !settings.value?.languages.includes(l)));

function addLanguage() {
  if (settings.value && adding.value) settings.value.languages.push(adding.value);
  adding.value = "";
}

function removeLanguage(code: string) {
  if (!settings.value) return;
  settings.value.languages = settings.value.languages.filter((l) => l !== code);
}

async function save() {
  if (!settings.value) return;
  saving.value = true;
  saveError.value = null;
  saved.value = false;
  try {
    await api.saveSubtitleSettings({ ...settings.value, opensubtitlesPassword: password.value });
    if (password.value) settings.value.hasOpensubtitlesPassword = true;
    password.value = "";
    saved.value = true;
  } catch (e) {
    saveError.value = e;
  } finally {
    saving.value = false;
  }
}

const signingIn = computed(
  () => !!settings.value?.opensubtitlesUser && (!!password.value || settings.value.hasOpensubtitlesPassword),
);
</script>

<template>
  <ErrorNote v-if="loadError" :error="loadError" />
  <form v-if="settings" class="max-w-[640px] space-y-55" @submit.prevent="save">
    <section class="space-y-21" aria-labelledby="lang-heading">
      <SectionHeader id="lang-heading" title="Languages" subtitle="Most wanted first." />
      <ul class="flex flex-wrap items-center gap-2" aria-label="Subtitle languages, most wanted first">
        <li v-for="code in settings.languages" :key="code" class="tag tag-blue gap-1 pr-1">
          {{ languageName(code) }}
          <button
            type="button"
            class="grid size-5 place-items-center rounded-full hover:bg-white/15"
            :aria-label="`Remove ${languageName(code)}`"
            @click="removeLanguage(code)"
          >
            <Icon name="close" :size="12" />
          </button>
        </li>
        <li>
          <select v-model="adding" aria-label="Add a language" class="select h-8 w-48" @change="addLanguage">
            <option value="">Add language</option>
            <option v-for="code in addable" :key="code" :value="code">{{ languageName(code) }}</option>
          </select>
        </li>
      </ul>
      <label class="flex items-start gap-4">
        <input v-model="settings.autoFetch" type="checkbox" class="toggle" />
        <span>
          Download subtitles automatically while a film downloads
          <span class="mt-1 block helper-text-01">
            Gets the best match in your first language. They only show on screen when the film isn't in one of your
            languages.
          </span>
        </span>
      </label>
    </section>

    <section class="space-y-21" aria-labelledby="os-heading">
      <SectionHeader id="os-heading" title="OpenSubtitles">
        <template #action>
          <button type="button" class="btn btn-ghost btn-sm" @click="go({ name: 'help', topic: 'subtitles' })">Help <Icon name="help" /></button>
        </template>
      </SectionHeader>
      <div>
        <label for="os-key" class="field-label">API key</label>
        <input id="os-key" v-model="settings.opensubtitlesKey" type="password" autocomplete="off" spellcheck="false" class="field" />
        <p class="field-helper">From opensubtitles.com, under Consumers.</p>
      </div>
      <div class="grid grid-cols-2 gap-4">
        <div>
          <label for="os-user" class="field-label">Username</label>
          <input id="os-user" v-model="settings.opensubtitlesUser" autocomplete="username" spellcheck="false" class="field" />
        </div>
        <div>
          <label for="os-pass" class="field-label">Password</label>
          <input
            id="os-pass"
            v-model="password"
            type="password"
            autocomplete="current-password"
            :placeholder="settings.hasOpensubtitlesPassword ? 'Saved' : ''"
            class="field"
          />
        </div>
      </div>
      <p class="-mt-4 field-helper">Signing in with a free account allows 20 downloads a day.</p>
    </section>

    <section class="space-y-21" aria-labelledby="subdl-heading">
      <SectionHeader id="subdl-heading" title="Subdl" />
      <div>
        <label for="subdl-key" class="field-label">API key</label>
        <input id="subdl-key" v-model="settings.subdlKey" type="password" autocomplete="off" spellcheck="false" class="field" />
        <p class="field-helper">From subdl.com, on your account's API page.</p>
      </div>
    </section>

    <ErrorNote v-if="saveError" :error="saveError" />
    <div class="flex items-center gap-4">
      <button type="submit" class="btn btn-primary" :disabled="saving">
        {{ saving ? (signingIn ? "Signing in to OpenSubtitles" : "Saving") : "Save" }}
      </button>
      <p v-if="saved && !saveError" class="flex items-center gap-2 text-ok" role="status"><Icon name="checkmark" /> Saved</p>
    </div>
  </form>
</template>
