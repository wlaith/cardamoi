<script setup lang="ts">
import { computed } from "vue";
import { installError, installing, installUpdate, openRelease, readyUpdate, updateDismissed } from "../../updates";
import Icon from "../ui/Icon.vue";

const shown = computed(() => readyUpdate.value && (!updateDismissed.value || installError.value));
</script>

<template>
  <div v-if="shown" class="update-banner" :class="{ 'update-banner-error': installError }" role="status">
    <Icon :name="installError ? 'warning-filled' : 'download'" class="update-banner-icon" />
    <p class="min-w-0 flex-1">
      <template v-if="installError">Couldn't install Cardamoi {{ readyUpdate?.version }}: {{ installError }}</template>
      <template v-else-if="installing">Installing Cardamoi {{ readyUpdate?.version }}…</template>
      <template v-else>Cardamoi {{ readyUpdate?.version }} is ready. Restart to finish updating.</template>
    </p>
    <button v-if="readyUpdate" type="button" class="link inline-flex items-center gap-1" @click="openRelease(readyUpdate.version)">
      What's new <Icon name="external-link" />
    </button>
    <button type="button" class="btn btn-secondary btn-sm" :disabled="installing" @click="installUpdate">
      {{ installError ? "Try again" : "Restart now" }}
    </button>
    <button
      v-if="!installing"
      type="button"
      class="btn btn-ghost btn-sm btn-icon"
      aria-label="Remind me next launch"
      title="Remind me next launch"
      @click="
        updateDismissed = true;
        installError = null;
      "
    >
      <Icon name="close" />
    </button>
  </div>
</template>

<style scoped>
.update-banner {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 13px;
  padding: 5px 13px 5px 21px;
  color: var(--c-ink);
  background: var(--c-surface-raised);
  border-bottom: 1px solid var(--c-line);
  box-shadow: inset 3px 0 0 var(--c-ultramarine);
}
.update-banner-error {
  box-shadow: inset 3px 0 0 var(--c-red-text);
}
.update-banner-error .update-banner-icon {
  color: var(--c-red-text);
}
</style>
