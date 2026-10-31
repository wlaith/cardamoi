<script setup lang="ts">
import { nextTick, onMounted, watch } from "vue";
import { go } from "../../nav";
import { openTmdb, TMDB_API_SETTINGS, TMDB_SIGNUP } from "../../tmdb";
import PageHeader from "../ui/PageHeader.vue";

const props = defineProps<{ topic?: string }>();

const topics = [
  { id: "start", title: "Getting started" },
  { id: "tmdb", title: "Your TMDB key" },
  { id: "sources", title: "Sources and indexers" },
  { id: "files", title: "Where your videos are kept" },
  { id: "subtitles", title: "Subtitles" },
  { id: "playback", title: "Playing videos" },
  { id: "keys", title: "Keyboard shortcuts" },
];

const shortcuts: [string, string][] = [
  ["Space", "Play the selected film or episode, or pause"],
  ["⇧ Space", "Play from the beginning"],
  ["⌘F", "Search"],
  ["⌘[  ⌘]", "Back, forward"],
  ["⌥⌘1 to ⌥⌘4", "Up Next, Films, TV Shows, Downloads"],
  ["⌘1  ⌘2", "View as grid, as list"],
  ["⌘I", "Get Info"],
  ["⌘T", "Subtitles"],
  ["⌘K", "Mark as watched or unwatched"],
  ["⌘⌫", "Move to Trash"],
  ["⌘Z", "Undo the last change shown in the status bar"],
  ["⌘U", "Add a magnet link (or paste one anywhere in the window)"],
  ["⌘.", "Stop playback"],
  ["⌘,", "Preferences"],
];

async function scrollToTopic() {
  await nextTick();
  if (props.topic) document.getElementById(`help-${props.topic}`)?.scrollIntoView({ block: "start" });
}
onMounted(scrollToTopic);
watch(() => props.topic, scrollToTopic);
</script>

<template>
  <div class="space-y-55 p-34">
    <PageHeader title="Help" subtitle="How cardamoi finds, downloads and plays films and shows." />
    <div class="flex gap-12">
    <nav class="sticky top-8 w-56 shrink-0 self-start" aria-label="Help topics">
      <button
        v-for="t in topics"
        :key="t.id"
        class="sidenav-item"
        :aria-current="topic === t.id ? 'page' : undefined"
        @click="go({ name: 'help', topic: t.id })"
      >
        {{ t.title }}
      </button>
    </nav>
    <div class="max-w-[640px] flex-1 space-y-55 text-ink-2 selectable">
      <section id="help-start">
        <h2 class="mb-4 text-heading text-ink">Getting started</h2>
        <p class="mt-4 first-of-type:mt-0">
          Type a film or show into the search field at the top right and press Return. Open a result, pick a release (the
          one marked <strong class="font-semibold text-ink">Best Match</strong> is a safe choice) and press Download. It appears under
          <strong class="font-semibold text-ink">Downloads</strong>, and on <strong class="font-semibold text-ink">Up Next</strong> as soon as enough of it is on disk to watch.
        </p>
      </section>
      <section id="help-tmdb">
        <h2 class="mb-4 text-heading text-ink">Your TMDB key</h2>
        <p class="mt-4 first-of-type:mt-0">
          Search, posters, episode lists and matching downloads to the right title all come from TMDB, a free film and TV
          database. cardamoi uses your own key, which is free for personal use. Without one, the library, downloads and
          playback still work; titles just have no artwork and aren't matched.
        </p>
        <p class="mt-4 first-of-type:mt-0">
          To get one,
          <button class="link" @click="openTmdb(TMDB_SIGNUP)">create a free TMDB account</button>, then open
          <button class="link" @click="openTmdb(TMDB_API_SETTINGS)">your API settings</button> and request a key: choose
          Developer and say it's for personal, non-commercial use. Copy the
          <strong class="font-semibold text-ink">API Read Access Token</strong> (the long one that starts with eyJ; the shorter API Key works too) and paste it in
          <button class="link" @click="go({ name: 'preferences', pane: 'tmdb' })">Preferences › TMDB</button>, or
          <button class="link" @click="go({ name: 'setup' })">let cardamoi walk you through it</button>.
        </p>
        <p class="mt-4 first-of-type:mt-0">
          If TMDB stops accepting the key, a bar under the toolbar says so. Copy the token again from your API settings and
          replace it. Downloads that couldn't be matched are matched again as soon as a working key is saved.
        </p>
      </section>
      <section id="help-sources">
        <h2 class="mb-4 text-heading text-ink">Sources and indexers</h2>
        <p class="mt-4 first-of-type:mt-0">
          cardamoi looks for releases in its sources. The Internet Archive is built in and only has public domain and
          freely licensed films. For everything else, add indexers to the built-in Prowlarr: open
          <button class="link" @click="go({ name: 'preferences', pane: 'sources' })">Preferences › Sources</button>, press
          <strong class="font-semibold text-ink">Manage Indexers…</strong>, and add the indexers you use. If searches find nothing at all, this is
          usually why.
        </p>
        <p class="mt-4 first-of-type:mt-0">
          If you already run Prowlarr or Jackett, add its Torznab feed URL and API key under Add a Torznab Source instead.
        </p>
        <p class="mt-4 first-of-type:mt-0">
          <strong class="font-semibold text-ink">Availability</strong> says how many people are sharing a release: Good is 20 or more, Fair is 5 or more,
          Few is under 5. Releases nobody is sharing can't finish downloading.
        </p>
      </section>
      <section id="help-files">
        <h2 class="mb-4 text-heading text-ink">Where your videos are kept</h2>
        <p class="mt-4 first-of-type:mt-0">
          Everything is in the cardamoi folder inside your Movies folder. Video files you put there yourself show up
          in the library within a minute. Use <strong class="font-semibold text-ink">Show in Finder</strong> on any film or episode to go to its file.
          <strong class="font-semibold text-ink">Move to Trash</strong> moves files to the Trash, so you can put them back from there.
        </p>
      </section>
      <section id="help-subtitles">
        <h2 class="mb-4 text-heading text-ink">Subtitles</h2>
        <p class="mt-4 first-of-type:mt-0">
          Subtitles come from OpenSubtitles and Subdl, which each need a free API key. Create an account on
          opensubtitles.com, then add a consumer under your profile to get its key; on subdl.com the key is on your
          account's API page. Paste them into
          <button class="link" @click="go({ name: 'preferences', pane: 'subtitles' })">Preferences › Subtitles</button>.
        </p>
      </section>
      <section id="help-playback">
        <h2 class="mb-4 text-heading text-ink">Playing videos</h2>
        <p class="mt-4 first-of-type:mt-0">
          cardamoi plays videos with mpv. If it isn't installed, install it with Homebrew (<code>brew install mpv</code>),
          or choose <strong class="font-semibold text-ink">Open in System Player</strong> from a video's menu. While a video plays, the display at the
          top of the window shows its progress; click it for playback controls.
        </p>
      </section>
      <section id="help-keys">
        <h2 class="mb-4 text-heading text-ink">Keyboard shortcuts</h2>
        <table class="data-table">
          <tbody>
            <tr v-for="[keys, what] in shortcuts" :key="keys">
              <td class="cell-strong w-44 whitespace-nowrap">{{ keys }}</td>
              <td>{{ what }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
    </div>
  </div>
</template>
