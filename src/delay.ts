import { onUnmounted, ref, watch, type Ref, type WatchSource } from "vue";

export function delayed(source: WatchSource<unknown>, ms = 500): Ref<boolean> {
  const shown = ref(false);
  let timer: number | undefined;
  watch(
    source,
    (on) => {
      window.clearTimeout(timer);
      if (!on) shown.value = false;
      else if (!shown.value) timer = window.setTimeout(() => (shown.value = true), ms);
    },
    { immediate: true },
  );
  onUnmounted(() => window.clearTimeout(timer));
  return shown;
}
