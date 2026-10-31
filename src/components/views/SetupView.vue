<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { api, plainError } from "../../api";
import { back, canGoBack, replace, type SetupStep } from "../../nav";
import { keyFormat, loadTmdbStatus, matching, openTmdb, TMDB_API_SETTINGS, TMDB_SIGNUP } from "../../tmdb";
import Icon from "../ui/Icon.vue";
import Kbd from "../ui/Kbd.vue";
import ProgressBar from "../ui/ProgressBar.vue";

const props = defineProps<{ step?: SetupStep }>();
const step = computed<SetupStep>(() => props.step ?? "welcome");
const numbered: SetupStep[] = ["welcome", "get", "paste"];

const heading = ref<HTMLElement>();
const field = ref<HTMLInputElement>();

watch(
  step,
  async (s) => {
    await nextTick();
    if (s === "paste") field.value?.focus();
    else heading.value?.focus();
  },
  { immediate: true },
);

function to(s: SetupStep) {
  replace({ name: "setup", step: s });
}

function leave() {
  if (canGoBack.value) back();
  else replace({ name: "upnext" });
}

async function later() {
  await api.markOnboardingSeen().catch(() => {});
  await loadTmdbStatus();
  leave();
}

const key = ref("");
const reveal = ref(false);
const checking = ref(false);
const failure = ref<"rejected" | "unreachable" | "other" | null>(null);
const failureText = ref("");
const matched = ref(0);

const format = computed(() => keyFormat(key.value));
const formatLine = computed(() => {
  switch (format.value) {
    case "token":
      return "Read access token";
    case "key":
      return "API key";
    case "unknown":
      return "This doesn't look like a TMDB key. The token starts with eyJ; the API key is 32 letters and numbers.";
    default:
      return "";
  }
});

watch(key, () => {
  failure.value = null;
  failureText.value = "";
});

async function connect(skipCheck = false) {
  if (format.value !== "token" && format.value !== "key") return;
  checking.value = true;
  failure.value = null;
  try {
    matched.value = await api.saveTmdbKey(key.value, skipCheck);
    key.value = "";
    await loadTmdbStatus();
    to("done");
  } catch (e) {
    const text = String(e);
    failure.value = text.includes("tmdb:rejected") ? "rejected" : text.includes("tmdb:unreachable") ? "unreachable" : "other";
    failureText.value = plainError(e).message;
  } finally {
    checking.value = false;
  }
}

const status = computed(() => {
  if (checking.value) return "Checking with TMDB";
  if (failure.value === "rejected") return "TMDB didn't accept this key. Copy it again from your API settings.";
  if (failure.value === "unreachable") return "Couldn't reach TMDB to check it. You can save it now and it'll be checked when you're online.";
  return failureText.value;
});
</script>

<template>
  <div class="grid min-h-full place-items-center p-34">
    <div class="w-full max-w-[560px]">
      <p v-if="numbered.includes(step)" class="text-small tabular-nums text-ink-2" aria-label="Step">
        {{ numbered.indexOf(step) + 1 }} / {{ numbered.length }}
      </p>

      <template v-if="step === 'welcome'">
        <h1 ref="heading" tabindex="-1" class="mt-13 text-title lowercase outline-none">connect to tmdb</h1>
        <p class="mt-13">
          cardamoi uses TMDB, a free film and TV database, to find what you want to watch and to recognise what you
          download.
        </p>
        <ul class="mt-21 space-y-13">
          <li class="flex items-center gap-2"><Icon name="search" /> Search films and shows</li>
          <li class="flex items-center gap-2"><Icon name="video" /> Posters, artwork and episode lists</li>
          <li class="flex items-center gap-2"><Icon name="magnet" /> Downloads and magnet links matched to the right title</li>
        </ul>
        <p class="mt-21 text-ink-2">You need your own key. It's free and takes about two minutes.</p>
        <div class="mt-34 flex flex-wrap items-center gap-2">
          <button type="button" class="btn btn-primary" @click="to('get')">Get started <Icon name="arrow-right" /></button>
          <button type="button" class="btn btn-ghost" @click="to('paste')">I already have a key</button>
          <div class="flex-1" />
          <button type="button" class="btn btn-ghost" @click="later">Set up later</button>
        </div>
      </template>

      <template v-else-if="step === 'get'">
        <h1 ref="heading" tabindex="-1" class="mt-13 text-title lowercase outline-none">get a free key</h1>
        <p class="mt-13 text-ink-2">These pages open in your browser. Come back here when you've copied the key.</p>
        <ol class="mt-21 border-t border-line">
          <li class="flex gap-13 border-b border-line py-13">
            <span class="tabular-nums text-ink-2">01</span>
            <div class="min-w-0 flex-1">
              <p class="font-semibold">Create a free TMDB account</p>
              <p class="mt-2 text-ink-2">Confirm your email address before the next step.</p>
            </div>
            <button type="button" class="btn btn-secondary btn-sm shrink-0" @click="openTmdb(TMDB_SIGNUP)">
              Sign up <Icon name="external-link" />
            </button>
          </li>
          <li class="flex gap-13 border-b border-line py-13">
            <span class="tabular-nums text-ink-2">02</span>
            <div class="min-w-0 flex-1">
              <p class="font-semibold">Request an API key</p>
              <p class="mt-2 text-ink-2">Choose Developer and fill in the short form. Say it's for personal, non-commercial use.</p>
            </div>
            <button type="button" class="btn btn-secondary btn-sm shrink-0" @click="openTmdb(TMDB_API_SETTINGS)">
              API settings <Icon name="external-link" />
            </button>
          </li>
          <li class="flex gap-13 border-b border-line py-13">
            <span class="tabular-nums text-ink-2">03</span>
            <div class="min-w-0 flex-1">
              <p class="font-semibold">Copy the API Read Access Token</p>
              <p class="mt-2 text-ink-2">It's the long one that starts with eyJ. The shorter API Key works too.</p>
            </div>
          </li>
        </ol>
        <div class="mt-34 flex flex-wrap items-center gap-2">
          <button type="button" class="btn btn-secondary" @click="to('welcome')"><Icon name="arrow-left" /> Back</button>
          <button type="button" class="btn btn-primary" @click="to('paste')">I have my key <Icon name="arrow-right" /></button>
          <div class="flex-1" />
          <button type="button" class="btn btn-ghost" @click="later">Set up later</button>
        </div>
      </template>

      <form v-else-if="step === 'paste'" @submit.prevent="connect()">
        <h1 ref="heading" tabindex="-1" class="mt-13 text-title lowercase outline-none">paste your key</h1>
        <p class="mt-13 text-ink-2">It's saved on this Mac and only sent to TMDB.</p>
        <label for="tmdb-key" class="field-label mt-21">TMDB key</label>
        <div class="flex gap-2">
          <input
            id="tmdb-key"
            ref="field"
            v-model="key"
            :type="reveal ? 'text' : 'password'"
            autocomplete="off"
            spellcheck="false"
            class="field min-w-0 flex-1"
            :aria-invalid="format === 'unknown' || failure === 'rejected'"
            aria-describedby="tmdb-key-help tmdb-key-status"
          />
          <button
            type="button"
            class="btn btn-secondary btn-icon"
            :aria-label="reveal ? 'Hide key' : 'Show key'"
            :aria-pressed="reveal"
            @click="reveal = !reveal"
          >
            <Icon :name="reveal ? 'eye-off' : 'eye'" />
          </button>
        </div>
        <p id="tmdb-key-help" class="min-h-4" :class="format === 'unknown' ? 'field-error' : 'field-helper'">
          <template v-if="format === 'token' || format === 'key'"><Icon name="checkmark" class="inline align-[-3px]" /> {{ formatLine }}</template>
          <template v-else-if="formatLine">{{ formatLine }}</template>
          <template v-else>Paste it with <Kbd keys="⌘V" /></template>
        </p>

        <div id="tmdb-key-status" aria-live="polite" class="mt-13 min-h-5">
          <p v-if="status" :class="failure ? 'text-red-text' : 'text-ink-2'">{{ status }}</p>
          <button v-if="failure === 'rejected'" type="button" class="link mt-2" @click="openTmdb(TMDB_API_SETTINGS)">
            Open your TMDB API settings
          </button>
        </div>

        <div class="mt-34 flex flex-wrap items-center gap-2">
          <button type="button" class="btn btn-secondary" @click="to('get')"><Icon name="arrow-left" /> Back</button>
          <button v-if="failure === 'unreachable'" type="button" class="btn btn-secondary" @click="connect(true)">Save anyway</button>
          <button type="submit" class="btn btn-primary" :disabled="checking || (format !== 'token' && format !== 'key')">
            {{ checking ? "Checking" : "Connect" }}
          </button>
          <div class="flex-1" />
          <button type="button" class="btn btn-ghost" @click="later">Set up later</button>
        </div>
      </form>

      <template v-else>
        <h1 ref="heading" tabindex="-1" class="text-title lowercase outline-none">you're set</h1>
        <p class="mt-13">cardamoi is connected to TMDB. Search, artwork and matching are on.</p>
        <div v-if="matched" class="mt-21 space-y-2" aria-live="polite">
          <p v-if="matching" class="tabular-nums text-ink-2">Matching downloads · {{ matching.done }} / {{ matching.total }}</p>
          <p v-else class="text-ink-2">Your downloads have been matched.</p>
          <ProgressBar
            :value="matching ? matching.done / matching.total : 1"
            :state="matching ? 'active' : 'idle'"
            label="Matching downloads"
            thin
          />
        </div>
        <div class="mt-34">
          <button type="button" class="btn btn-primary" @click="leave">Start watching <Icon name="arrow-right" /></button>
        </div>
      </template>
    </div>
  </div>
</template>
