<script setup lang="ts">
import { h } from "vue";
import { icons } from "../../icons";

const props = withDefaults(defineProps<{ name: string; size?: number; label?: string }>(), { size: 16 });

const Glyph = () =>
  h(icons[props.name], {
    size: props.size,
    strokeWidth: 2,
    "stroke-linecap": "square",
    "stroke-linejoin": "miter",
    class: "shrink-0",
    role: props.label ? "img" : undefined,
    "aria-label": props.label,
    "aria-hidden": props.label ? undefined : "true",
    focusable: "false",
  });
</script>

<template>
  <Glyph />
</template>
