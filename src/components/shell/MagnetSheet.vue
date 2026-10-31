<script setup lang="ts">
import { computed, ref } from "vue";
import { magnetName } from "../../api";
import { addMagnet } from "../../commands";
import { closeSheet } from "../../ui";

const props = defineProps<{ link: string }>();
const link = ref(props.link);
const adding = ref(false);
const error = ref("");

const valid = computed(() => /^magnet:\?.*xt=urn:bt[im]h:/i.test(link.value.trim()));
const hint = computed(() => {
  if (!link.value.trim()) return "";
  if (!link.value.trim().startsWith("magnet:")) return "A magnet link starts with magnet:?";
  if (!valid.value) return "This magnet link is missing its torrent hash (xt=urn:btih:…).";
  return magnetName(link.value.trim()) ? `"${magnetName(link.value.trim())}"` : "";
});

async function add() {
  if (!valid.value) return;
  adding.value = true;
  error.value = (await addMagnet(link.value.trim())) ?? "";
  adding.value = false;
  if (!error.value) closeSheet();
}
</script>

<template>
  <form @submit.prevent="add">
    <div class="px-4 pt-4 pr-16">
      <p class="text-small tabular-nums text-ink-2">Add to library</p>
      <h2 class="mt-1 text-heading">Add a magnet link</h2>
    </div>
    <div class="px-4 pt-6">
      <p>It downloads into your library. The poster is matched from its name.</p>
      <label for="magnet-link" class="field-label mt-6">Magnet link</label>
      <input
        id="magnet-link"
        v-model="link"
        class="field"
        placeholder="magnet:?xt=urn:btih:…"
        autocomplete="off"
        spellcheck="false"
        :aria-invalid="!!link.trim() && !valid"
      />
      <p class="min-h-4 truncate" :class="valid && !error ? 'field-helper' : 'field-error'">{{ error || hint }}</p>
    </div>
    <div class="modal-footer">
      <button type="button" class="btn btn-secondary" @click="closeSheet()">Cancel</button>
      <button type="submit" class="btn btn-primary" :disabled="!valid || adding">{{ adding ? "Adding" : "Add" }}</button>
    </div>
  </form>
</template>
