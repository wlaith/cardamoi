<script setup lang="ts">
import type { AnimationItem } from "lottie-web";
import { onMounted, onUnmounted, ref, watch } from "vue";
import { delayed } from "../../delay";
import globeInk from "../../../brand/assets/logo/cardamoi-globe-ink.svg";
import globeRed from "../../../brand/assets/logo/cardamoi-globe-red.svg";

withDefaults(defineProps<{ size?: number; text?: string; inline?: boolean }>(), { size: 48 });

const shown = delayed(() => true);
const box = ref<HTMLElement>();
const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const paper = document.documentElement.dataset.theme === "paper";
let anim: AnimationItem | undefined;

async function start() {
  if (still || !box.value || anim) return;
  const [{ default: lottie }, data] = await Promise.all([
    import("lottie-web/build/player/lottie_light"),
    paper ? import("../../../brand/assets/motion/cardamoi-loader-ink.json") : import("../../../brand/assets/motion/cardamoi-loader-red.json"),
  ]);
  if (!box.value) return;
  anim = lottie.loadAnimation({ container: box.value, renderer: "svg", loop: true, autoplay: true, animationData: data.default });
}

onMounted(() => watch(shown, (on) => on && start(), { immediate: true, flush: "post" }));
onUnmounted(() => anim?.destroy());
</script>

<template>
  <div
    v-if="shown"
    class="flex items-center"
    :class="inline ? 'gap-13' : 'flex-col justify-center gap-13 text-center'"
    role="status"
    :aria-label="text ? undefined : 'Loading'"
  >
    <img v-if="still" :src="paper ? globeInk : globeRed" alt="" class="shrink-0" :style="{ width: `${size}px`, height: `${size}px` }" />
    <div v-else ref="box" class="shrink-0" :style="{ width: `${size}px`, height: `${size}px` }" aria-hidden="true" />
    <p v-if="text" class="text-ink-2">{{ text }}</p>
  </div>
</template>
