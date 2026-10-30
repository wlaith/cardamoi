<script setup lang="ts">
import { notice, undoLast } from "../../store";
import Icon from "../ui/Icon.vue";

</script>

<template>
  <div class="pointer-events-none fixed right-21 bottom-21 z-30" role="status" aria-live="polite">
    <div v-if="notice" class="toast pointer-events-auto w-[377px] max-w-[calc(100vw-42px)]">
      <Icon name="checkmark-filled" />
      <p class="min-w-0 flex-1 py-13">{{ notice.text }}</p>
      <button v-if="notice.undo" class="btn btn-ghost" @click="undoLast">Undo</button>
      <button class="btn-icon" aria-label="Close" @click="notice = null">
        <Icon name="close" />
      </button>
    </div>
  </div>
</template>
