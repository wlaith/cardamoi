<script setup lang="ts">
import { computed, ref } from "vue";
import { formatBytes } from "../../api";
import { setTrashTimer } from "../../commands";
import type { LocalEpisode } from "../../library";
import { closeSheet } from "../../ui";
import Icon from "../ui/Icon.vue";

const props = defineProps<{ title: string; name: string; videos: LocalEpisode[] }>();

const HOUR = 3600;
const DAY = 24 * HOUR;
const choices = [
  { label: "1 day", secs: DAY },
  { label: "3 days", secs: 3 * DAY },
  { label: "1 week", secs: 7 * DAY },
  { label: "2 weeks", secs: 14 * DAY },
];
const choice = ref<number | "custom">(3 * DAY);
const days = ref(2);
const hours = ref(0);

const secs = computed(() => (choice.value === "custom" ? days.value * DAY + hours.value * HOUR : choice.value));

const size = computed(() => {
  const bytes = formatBytes(props.videos.reduce((sum, ep) => sum + ep.video.len, 0));
  return props.videos.length > 1 ? `${props.videos.length} episodes, ${bytes}` : bytes;
});

const when = computed(() => {
  const at = new Date(Date.now() + secs.value * 1000);
  const day = at.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" });
  return `${day} at ${at.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
});

async function set() {
  closeSheet();
  await setTrashTimer(props.videos, props.name, Math.round(Date.now() / 1000) + secs.value);
}
</script>

<template>
  <form @submit.prevent="set">
    <div class="px-34 pt-34">
      <h2 id="trash-timer-title" class="text-heading">{{ title }}</h2>
      <p class="mt-1 text-small tabular-nums text-ink-2">{{ size }}</p>
    </div>
    <div class="grid grid-cols-[auto_1fr] items-center gap-x-13 gap-y-2 px-34 pt-21">
      <label for="trash-in">In</label>
      <select id="trash-in" v-model="choice" class="select w-[160px] tabular-nums">
        <option v-for="c in choices" :key="c.secs" :value="c.secs">{{ c.label }}</option>
        <option value="custom">Custom…</option>
      </select>
      <div v-if="choice === 'custom'" class="col-start-2 flex gap-2 tabular-nums">
        <div class="stepper" role="group" aria-label="Days">
          <button type="button" class="btn-icon" aria-label="Fewer days" @click="days = Math.max(0, days - 1)"><Icon name="subtract" /></button>
          <span>{{ days }} {{ days === 1 ? "day" : "days" }}</span>
          <button type="button" class="btn-icon" aria-label="More days" @click="days = Math.min(365, days + 1)"><Icon name="add" /></button>
        </div>
        <div class="stepper" role="group" aria-label="Hours">
          <button type="button" class="btn-icon" aria-label="Fewer hours" @click="hours = Math.max(0, hours - 1)"><Icon name="subtract" /></button>
          <span>{{ hours }} {{ hours === 1 ? "hour" : "hours" }}</span>
          <button type="button" class="btn-icon" aria-label="More hours" @click="hours = Math.min(23, hours + 1)"><Icon name="add" /></button>
        </div>
      </div>
      <p class="col-start-2 tabular-nums text-ink-2" aria-live="polite">{{ when }}</p>
    </div>
    <div class="modal-footer">
      <button type="button" class="btn btn-secondary" @click="closeSheet()">Cancel</button>
      <button type="submit" class="btn btn-primary" :disabled="secs <= 0">Set Timer</button>
    </div>
  </form>
</template>
