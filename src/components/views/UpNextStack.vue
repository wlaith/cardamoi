<script lang="ts">
import { ref } from "vue";

type FilterId = "all" | "continue" | "next" | "new" | "downloading" | "watched" | "films" | "shows";

const filter = ref<FilterId>("all");
const focusKey = ref<string | null>(null);
const zoomTarget = ref(1);
</script>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, onUpdated, ref as vueRef, watch } from "vue";
import { episodeLabel, timeLeft } from "../../api";
import { openFilm, openFolder, openShow, play, showMenu, videoMenu } from "../../commands";
import { isTyping } from "../../keys";
import {
  filmVideo,
  nextEpisode,
  playedShare,
  showKey,
  showSummary,
  watchedShare,
  type LocalEpisode,
  type Shelf,
} from "../../library";
import { go } from "../../nav";
import { homeMode, selection, shelf } from "../../store";
import { filmLine, showLine } from "../../status";
import { openMenu, openMenuBelow } from "../../ui";
import Icon from "../ui/Icon.vue";

type State = "continue" | "next" | "new" | "downloading" | "watched" | "other";

interface Card {
  key: string;
  item: Shelf;
  title: string;
  poster: string | null;
  ep: LocalEpisode | null;
  state: State;
  downloading: boolean;
  line: string;
  fill: number;
  playedAt: number;
  addedAt: number;
}

const W = 178;
const H = 267;
const PITCH = 8;
const GAP_FRONT = 233;
const GAP_BACK = 189;
const BEHIND = 34;
const ZOOM_MIN = 0.3;
const ZOOM_MAX = 1.9;
const OVERVIEW = 0.36;

const ACTIVE = ["resolving", "starting", "downloading", "paused"];

function cardOf(item: Shelf): Card {
  if (item.kind === "film") {
    const e = item.entry;
    const ep = filmVideo(e);
    const v = ep?.video;
    const downloading = ACTIVE.includes(e.status);
    const state: State =
      v?.resumeAt != null ? "continue" : v?.watched ? "watched" : v?.playable ? "new" : downloading ? "downloading" : "other";
    return {
      key: `film:${e.id}`,
      item,
      title: e.title,
      poster: e.poster,
      ep,
      state,
      downloading,
      line: filmLine(e).text,
      fill: e.status !== "done" && downloading ? e.progress : v?.resumeAt != null ? playedShare(v) : 0,
      playedAt: v?.lastPlayedAt ?? 0,
      addedAt: e.added_at,
    };
  }
  const show = item.show;
  const next = nextEpisode(show);
  const s = showSummary(show);
  const started = show.episodes.some((e) => e.video.watched || e.video.lastPlayedAt != null);
  const state: State =
    next?.video.resumeAt != null
      ? "continue"
      : next?.video.playable && started
        ? "next"
        : next?.video.playable
          ? "new"
          : s.total && s.watched === s.total
            ? "watched"
            : s.downloading || show.pending.length
              ? "downloading"
              : "other";
  const line =
    next && (state === "continue" || state === "next")
      ? [episodeLabel(next.video.season, next.video.episode), next.video.resumeAt != null ? timeLeft(next.video) : ""]
          .filter(Boolean)
          .join(" · ")
      : showLine(show);
  return {
    key: `show:${showKey(show)}`,
    item,
    title: show.title,
    poster: show.poster,
    ep: next,
    state,
    downloading: s.downloading > 0 || show.pending.length > 0,
    line,
    fill: state === "continue" && next ? playedShare(next.video) : watchedShare(show),
    playedAt: Math.max(0, ...show.episodes.map((e) => e.video.lastPlayedAt ?? 0)),
    addedAt: Math.max(...show.entries.map((e) => e.added_at)),
  };
}

const filters: { id: FilterId; label: string; test: (c: Card) => boolean }[] = [
  { id: "all", label: "Everything", test: () => true },
  { id: "continue", label: "Continue watching", test: (c) => c.state === "continue" },
  { id: "next", label: "Next episodes", test: (c) => c.state === "next" },
  { id: "new", label: "Not started", test: (c) => c.state === "new" },
  { id: "downloading", label: "Downloading", test: (c) => c.downloading },
  { id: "watched", label: "Watched", test: (c) => c.state === "watched" },
  { id: "films", label: "Films", test: (c) => c.item.kind === "film" },
  { id: "shows", label: "Shows", test: (c) => c.item.kind === "show" },
];

const everything = computed(() => shelf.value.map(cardOf));

const filterRows = computed(() =>
  filters
    .map((f) => ({ ...f, count: everything.value.filter(f.test).length }))
    .filter((f) => f.id === "all" || f.count > 0 || f.id === filter.value),
);

const time = (c: Card) => Math.max(c.addedAt, c.playedAt);

const cards = computed(() => {
  const test = filters.find((f) => f.id === filter.value)!.test;
  return everything.value.filter(test).sort((a, b) => time(b) - time(a));
});
const indexOf = computed(() => new Map(cards.value.map((c, i) => [c.key, i])));

watch(
  () => filterRows.value.find((f) => f.id === filter.value)?.count,
  (count) => {
    if (!count && filter.value !== "all") filter.value = "all";
  },
);

const index = computed(() => indexOf.value.get(focusKey.value ?? "") ?? 0);
const focused = computed<Card | null>(() => cards.value[index.value] ?? null);

watch(
  focused,
  (c) => {
    if (!c) selection.value = null;
    else if (c.item.kind === "show") selection.value = { kind: "show", show: c.item.show };
    else selection.value = c.ep ? { kind: "video", ep: c.ep } : null;
  },
  { immediate: true },
);

const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

let target = index.value;
const cam = { pos: index.value + 21, zoom: 0.62, rx: -32, ry: -50, px: 0, py: 0 };

const center = vueRef(Math.round(cam.pos));
const reach = vueRef(144);

interface Live {
  el: HTMLElement;
  rank: number;
  f: number;
  fv: number;
  hover: number;
  hoverTo: number;
  born: number;
  delay: number;
}
const live = new Map<string, Live>();
let introAt = -Infinity;

const stage = vueRef<HTMLElement>();
const world = vueRef<HTMLElement>();

const visible = computed(() => {
  const lo = Math.max(0, Math.min(center.value, cards.value.length - 1) - BEHIND);
  return cards.value.slice(lo, center.value + reach.value + 1).map((card, n) => ({ card, i: lo + n }));
});

function setTarget(t: number, snap = false) {
  const n = cards.value.length;
  if (!n) return;
  target = Math.min(n - 1, Math.max(0, snap ? Math.round(t) : t));
  const key = cards.value[Math.round(target)].key;
  if (key !== focusKey.value) focusKey.value = key;
  kick();
}

function focusAt(i: number) {
  setTarget(i, true);
}

function move(by: number) {
  setTarget(Math.round(target) + by, true);
}

function zoomTo(z: number) {
  zoomTarget.value = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));
  kick();
}

const approach = (v: number, to: number, dt: number, k: number) => (reduced ? to : to + (v - to) * Math.exp(-k * dt));
const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);
const clamp01 = (t: number) => Math.min(1, Math.max(0, t));

let spread = 1;

function rowZ(d: number, open: number) {
  const pitch = PITCH * (1 + (spread - 1) * (1 - open));
  return d * pitch + (d < 0 ? GAP_FRONT : GAP_BACK) * open * Math.tanh(d * 1.35);
}

let frame = 0;
let last = 0;

function kick() {
  if (!frame) {
    last = performance.now();
    frame = requestAnimationFrame(tick);
  }
}

function tick(now: number) {
  frame = 0;
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  if (!world.value) return;
  let busy = false;
  const track = (a: number, b: number) => {
    if (Math.abs(a - b) > 0.0008) busy = true;
    return b;
  };

  track(cam.pos, (cam.pos = approach(cam.pos, target, dt, 7.5)));
  track(cam.zoom, (cam.zoom = approach(cam.zoom, zoomTarget.value, dt, 7)));
  track(cam.ry, (cam.ry = approach(cam.ry, -50 + cam.px * 6, dt, 4)));
  track(cam.rx, (cam.rx = approach(cam.rx, -32 - cam.py * 5, dt, 4)));

  const open = easeOut(clamp01((cam.zoom - OVERVIEW) / (0.85 - OVERVIEW)));
  const z = cam.zoom * fit.value;
  spread = Math.min(5, Math.max(1, 900 / (Math.max(1, cards.value.length) * PITCH * OVERVIEW * 0.9)));

  const c = Math.round(cam.pos);
  if (c !== center.value) center.value = c;
  const r = Math.min(400, Math.round(144 / Math.min(1, cam.zoom) ** 1.2));
  if (Math.abs(r - reach.value) > 21) reach.value = r;

  let minD = Infinity;
  let maxD = -Infinity;
  for (const [key, s] of live) {
    const idx = indexOf.value.get(key);
    if (idx == null) continue;
    track(s.rank, (s.rank = approach(s.rank, idx, dt, 9)));
    track(s.hover, (s.hover = approach(s.hover, s.hoverTo, dt, 16)));

    const d = s.rank - cam.pos;
    const w = Math.exp(-d * d * 2.4) * open;

    if (reduced) s.f = w;
    else {
      s.fv += (170 * (w - s.f) - 17 * s.fv) * dt;
      s.f += s.fv * dt;
    }
    if (Math.abs(w - s.f) > 0.001 || Math.abs(s.fv) > 0.001) busy = true;

    let y = -(21 * s.f + 13 * s.hover);
    let alpha = 1;
    const intro = reduced ? 1 : clamp01((now - s.born - s.delay) / 700);
    if (intro < 1) {
      busy = true;
      const e = easeOut(intro);
      y += (1 - e) * 89;
      alpha = e;
    }
    alpha *= clamp01((d + BEHIND - 2) / 8) * clamp01((reach.value - d) / 13);

    const rz = rowZ(d, open);
    const turn = 26 * s.f;
    const scale = 1 + 0.06 * s.f;
    s.el.style.transform = `translate3d(0, ${y.toFixed(2)}px, ${(-rz).toFixed(2)}px) rotateY(${turn.toFixed(2)}deg) scale(${scale.toFixed(4)})`;
    s.el.style.opacity = alpha.toFixed(3);
    s.el.style.setProperty("--focus", clamp01(s.f).toFixed(3));
    s.el.style.setProperty("--dim", ((1 - clamp01(s.f)) * 0.42 * open - 0.3 * s.hover).toFixed(3));
    if (alpha > 0.05) {
      minD = Math.min(minD, d);
      maxD = Math.max(maxD, d);
    }
  }

  const mid = live.size && maxD > minD ? (minD + maxD) / 2 : 0;
  const shift = rowZ(mid, open) * (1 - open);
  world.value.style.transform = `scale3d(${z}, ${z}, ${z}) rotateX(${cam.rx.toFixed(3)}deg) rotateY(${cam.ry.toFixed(3)}deg) translate3d(0, 0, ${shift.toFixed(2)}px)`;

  if (busy || dragging) frame = requestAnimationFrame(tick);
}

function sync() {
  if (!world.value) return;
  const now = performance.now();
  const seen = new Set<string>();
  for (const el of world.value.querySelectorAll<HTMLElement>(".card[data-key]")) {
    if (el.dataset.leaving) continue;
    const key = el.dataset.key!;
    seen.add(key);
    const s = live.get(key);
    if (s) {
      s.el = el;
      continue;
    }
    const idx = indexOf.value.get(key) ?? 0;
    const entering = now - introAt < 900;
    live.set(key, {
      el,
      rank: idx,
      f: 0,
      fv: 0,
      hover: 0,
      hoverTo: 0,
      born: entering ? introAt : -Infinity,
      delay: entering ? Math.min(Math.abs(idx - Math.round(target)), 34) * 17 : 0,
    });
  }
  for (const key of live.keys()) if (!seen.has(key)) live.delete(key);
  kick();
}

onUpdated(sync);

function onLeave(el: Element, done: () => void) {
  const node = el as HTMLElement;
  node.dataset.leaving = "1";
  if (reduced || Number(node.style.opacity || 1) < 0.05) return done();
  const from = node.style.transform;
  node
    .animate(
      [
        { transform: from, opacity: node.style.opacity || "1" },
        { transform: `${from} translateY(110px)`, opacity: 0 },
      ],
      { duration: 380, delay: Math.random() * 140, easing: "cubic-bezier(.55,0,.8,.2)", fill: "forwards" },
    )
    .finished.then(done, done);
}

watch(filter, () => {
  introAt = performance.now();
  nextTick(() => setTarget(indexOf.value.get(focusKey.value ?? "") ?? 0, true));
});
watch(indexOf, () => {
  const i = indexOf.value.get(focusKey.value ?? "");
  if (i != null && Math.round(target) !== i) target = i;
  kick();
});

const fit = vueRef(1);
const hovered = vueRef<Card | null>(null);
const tip = vueRef({ x: 0, y: 0 });
const leaving = vueRef(false);

function onKey(event: KeyboardEvent) {
  if (event.metaKey || event.ctrlKey || event.altKey || isTyping(event.target)) return;
  const steps: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1, PageDown: 10, PageUp: -10 };
  if (event.key in steps) move(steps[event.key]);
  else if (event.key === "Home") focusAt(0);
  else if (event.key === "End") focusAt(cards.value.length - 1);
  else if (event.key === "Enter" && focused.value) flyInto(focused.value);
  else if (event.key === "-" || event.key === "_") zoomTo(zoomTarget.value / 1.35);
  else if (event.key === "=" || event.key === "+") zoomTo(zoomTarget.value * 1.35);
  else if (event.key === "0") zoomTo(1);
  else if (event.key === "o") toggleOverview();
  else return;
  event.preventDefault();
}

let snapTimer: number | undefined;
function onWheel(event: WheelEvent) {
  if (event.ctrlKey) {
    zoomTo(zoomTarget.value * Math.exp(-event.deltaY * 0.012));
    return;
  }
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  setTarget(target + (delta / 42) * Math.max(1, 1 / zoomTarget.value));
  window.clearTimeout(snapTimer);
  snapTimer = window.setTimeout(() => setTarget(target, true), 140);
}

let dragging = false;
let dragged = false;
let dragX = 0;
let dragStartX = 0;
let dragV = 0;
let dragT = 0;

const pxPerPoster = () => Math.max(3, 24 * zoomTarget.value ** 1.4);

function onPointerDown(event: PointerEvent) {
  if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
  dragging = true;
  dragged = false;
  dragX = dragStartX = event.clientX;
  dragV = 0;
  dragT = performance.now();
}

function onPointerMove(event: PointerEvent) {
  const box = stage.value!.getBoundingClientRect();
  cam.px = ((event.clientX - box.left) / box.width) * 2 - 1;
  cam.py = ((event.clientY - box.top) / box.height) * 2 - 1;
  const home = stage.value!.parentElement!.getBoundingClientRect();
  tip.value = { x: event.clientX - home.left, y: event.clientY - home.top };
  kick();
  if (!dragging) return;
  if (!dragged && Math.abs(event.clientX - dragStartX) > 5) {
    dragged = true;
    stage.value!.setPointerCapture(event.pointerId);
    hovered.value = null;
  }
  if (!dragged) return;
  const now = performance.now();
  const dx = event.clientX - dragX;
  dragX = event.clientX;
  const step = -dx / pxPerPoster();
  dragV = 0.8 * (step / Math.max(1, now - dragT)) + 0.2 * dragV;
  dragT = now;
  setTarget(target + step);
}

function onPointerUp() {
  if (!dragging) return;
  dragging = false;
  if (dragged) setTarget(target + dragV * 260, true);
  window.setTimeout(() => (dragged = false));
}

function onPointerLeave() {
  cam.px = 0;
  cam.py = 0;
  kick();
}

function onCardEnter(card: Card) {
  const s = live.get(card.key);
  if (s) s.hoverTo = 1;
  if (!dragged) hovered.value = card;
  kick();
}

function onCardLeave(card: Card) {
  const s = live.get(card.key);
  if (s) s.hoverTo = 0;
  if (hovered.value?.key === card.key) hovered.value = null;
  kick();
}

function onCardClick(i: number) {
  if (dragged) return;
  const card = cards.value[i];
  if (i === index.value && zoomTarget.value >= 0.7) flyInto(card);
  else {
    focusAt(i);
    if (zoomTarget.value < 0.7) zoomTo(1);
  }
  stage.value?.focus({ preventScroll: true });
}

function onStageDblClick(event: MouseEvent) {
  if (!(event.target as HTMLElement).closest(".card")) toggleOverview();
}

function toggleOverview() {
  zoomTo(zoomTarget.value < 0.7 ? 1 : OVERVIEW);
}

function onCardMenu(event: MouseEvent, i: number) {
  focusAt(i);
  openMenu(event, menuFor(cards.value[i]));
}

const rail = vueRef<HTMLElement>();
function scrub(event: PointerEvent) {
  const box = rail.value!.getBoundingClientRect();
  const share = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
  setTarget(share * (cards.value.length - 1));
}
function onRailDown(event: PointerEvent) {
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  scrub(event);
}
function onRailMove(event: PointerEvent) {
  if ((event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) scrub(event);
}
function onRailUp() {
  setTarget(target, true);
}

const position = computed(() => (cards.value.length > 1 ? index.value / (cards.value.length - 1) : 0));

function playCard(c: Card) {
  if (c.ep?.video.playable) play(c.ep, c.ep.video.watched);
}

function open(c: Card) {
  if (c.item.kind === "show") openShow(c.item.show);
  else if (c.ep) openFilm(c.ep);
  else go({ name: "downloads" });
}

function flyInto(c: Card) {
  if (leaving.value) return;
  if (reduced) return open(c);
  leaving.value = true;
  focusAt(indexOf.value.get(c.key) ?? 0);
  zoomTo(ZOOM_MAX);
  window.setTimeout(() => {
    open(c);
    leaving.value = false;
  }, 340);
}

function menuFor(c: Card) {
  if (c.item.kind === "show") return showMenu(c.item.show);
  return c.ep ? videoMenu(c.ep, { open: () => open(c) }) : [{ label: "Show in Downloads", action: () => open(c) }];
}

const more = vueRef<HTMLElement>();
function focusedMenu() {
  if (focused.value && more.value) openMenuBelow(more.value, menuFor(focused.value));
}

function setFilter(id: FilterId) {
  filter.value = id;
  stage.value?.focus({ preventScroll: true });
}

const dateFormat = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" });
const clockFormat = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });

const stamp = computed(() => {
  const c = focused.value;
  if (!c) return null;
  const at = time(c);
  const watchedLast = c.playedAt >= c.addedAt;
  return {
    label: watchedLast ? "Last watched" : "Added",
    date: dateFormat.format(at * 1000),
    clock: clockFormat.format(at * 1000),
  };
});

const eyebrow: Record<State, string> = {
  continue: "Continue watching",
  next: "Next episode",
  new: "Not started",
  downloading: "Downloading",
  watched: "Watched",
  other: "In your library",
};

const playable = computed(() => !!focused.value?.ep?.video.playable);
const playLabel = computed(() => {
  const v = focused.value?.ep?.video;
  return v?.resumeAt != null ? "Continue" : v?.watched ? "Play again" : "Play";
});

const zoomLabel = computed(() => `${Math.round(zoomTarget.value * 100)} %`);
const showTip = computed(() => !!hovered.value && (hovered.value.key !== focused.value?.key || zoomTarget.value < 0.7));

const resize = new ResizeObserver(([entry]) => {
  const { width, height } = entry.contentRect;
  fit.value = Math.max(0.55, Math.min(1, (width - 144) / 1063, height / 820));
  kick();
});

onMounted(() => {
  target = index.value;
  if (zoomTarget.value > 1.2) zoomTarget.value = 1;
  introAt = performance.now();
  resize.observe(stage.value!);
  stage.value?.focus({ preventScroll: true });
  window.addEventListener("pointerup", onPointerUp);
  sync();
});

onUnmounted(() => {
  resize.disconnect();
  cancelAnimationFrame(frame);
  window.clearTimeout(snapTimer);
  window.removeEventListener("pointerup", onPointerUp);
});
</script>

<template>
  <div class="home relative h-full min-h-[560px] overflow-hidden" :class="{ leaving }">
    <div
      ref="stage"
      class="stage absolute inset-0"
      tabindex="0"
      role="listbox"
      aria-label="Your library, most recent first"
      :aria-activedescendant="focused ? `stack-${index}` : undefined"
      @keydown="onKey"
      @wheel.prevent="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerleave="onPointerLeave"
      @dblclick="onStageDblClick"
    >
      <div ref="world" class="world">
        <TransitionGroup tag="div" class="layer" :css="false" @leave="onLeave">
          <div
            v-for="{ card, i } in visible"
            :id="`stack-${i}`"
            :key="card.key"
            :data-key="card.key"
            class="card"
            :class="{ 'is-focus': i === index }"
            role="option"
            :aria-selected="i === index"
            :aria-label="[card.title, eyebrow[card.state], card.line].filter(Boolean).join('. ')"
            @click="onCardClick(i)"
            @contextmenu.prevent="onCardMenu($event, i)"
            @pointerenter="onCardEnter(card)"
            @pointerleave="onCardLeave(card)"
          >
            <div class="poster">
              <img v-if="card.poster" :src="card.poster" alt="" draggable="false" />
              <span v-else class="poster-title">{{ card.title }}</span>
              <span v-if="card.fill > 0" class="slot" aria-hidden="true">
                <span :style="{ width: `${card.fill * 100}%` }" />
              </span>
            </div>
            <button
              v-if="i === index && card.ep?.video.playable"
              class="poster-play"
              tabindex="-1"
              :aria-label="`Play ${card.title}`"
              @click.stop="playCard(card)"
            >
              <Icon name="play" :size="24" />
            </button>
          </div>
        </TransitionGroup>
      </div>
    </div>

    <Transition name="tip">
      <div v-if="showTip && hovered" class="tip" :style="{ transform: `translate3d(${tip.x + 16}px, ${tip.y + 16}px, 0)` }">
        <p class="font-semibold">{{ hovered.title }}</p>
        <p v-if="hovered.line" class="text-small tabular-nums text-ink-muted">{{ hovered.line }}</p>
      </div>
    </Transition>

    <section v-if="focused && stamp" class="overlay chrome top-0 left-0 w-[377px] p-34" style="--d: 120">
      <div class="swap-stack" aria-live="polite" aria-atomic="true">
        <Transition name="swap">
          <div :key="focused.key" class="swap-item">
            <p class="text-small text-ink-muted">{{ stamp.label }}</p>
            <p class="mt-(--space-5) text-small tabular-nums">
              {{ stamp.date }}<br />
              <span class="text-ink-muted">{{ stamp.clock }}</span>
            </p>
            <p class="mt-34 text-small text-ink-muted">{{ eyebrow[focused.state] }}</p>
            <h1 class="mt-2 text-heading">
              <button class="text-left underline-offset-4 hover:underline" @click="flyInto(focused)">{{ focused.title }}</button>
            </h1>
            <p v-if="focused.line" class="mt-(--space-5) text-small tabular-nums text-ink-muted">{{ focused.line }}</p>
          </div>
        </Transition>
      </div>
      <div class="mt-21 flex items-center gap-13">
        <button v-if="playable" class="btn btn-primary press" @click="playCard(focused)"><Icon name="play" /> {{ playLabel }}</button>
        <button v-else-if="focused.downloading" class="btn btn-secondary press" @click="go({ name: 'downloads' })">Show in Downloads</button>
        <button class="btn btn-ghost press" @click="flyInto(focused)">Open</button>
        <button ref="more" class="btn-icon press" aria-label="More" aria-haspopup="menu" @click="focusedMenu"><Icon name="overflow" /></button>
      </div>
    </section>

    <nav class="overlay chrome right-34 bottom-[144px] flex flex-col items-end" style="--d: 220" aria-label="Show">
      <button
        v-for="(f, n) in filterRows"
        :key="f.id"
        class="filter"
        :class="{ 'mb-2': f.id === 'all' || f.id === 'watched' }"
        :style="{ '--n': n }"
        :aria-pressed="filter === f.id"
        @click="setFilter(f.id)"
      >
        {{ f.label }}<sup class="text-small tabular-nums">{{ f.count }}</sup>
      </button>
    </nav>

    <div class="overlay chrome right-34 bottom-34 left-34 flex items-center justify-end gap-21" style="--d: 300">
      <p class="hint text-small hidden text-ink-muted 2xl:block">
        <kbd>←</kbd> <kbd>→</kbd> browse <span class="mx-2">·</span> <kbd>−</kbd> <kbd>+</kbd> zoom <span class="mx-2">·</span>
        <kbd>↵</kbd> open <span class="mx-2">·</span> <kbd>space</kbd> play
      </p>
      <span class="text-small tabular-nums shrink-0 text-ink-muted">
        <span class="text-ink">{{ cards.length ? index + 1 : 0 }}</span> / {{ cards.length }}
      </span>
      <div
        ref="rail"
        class="rail"
        role="slider"
        tabindex="0"
        aria-label="Position in your library"
        :aria-valuemin="1"
        :aria-valuemax="cards.length"
        :aria-valuenow="index + 1"
        :aria-valuetext="focused?.title"
        @pointerdown="onRailDown"
        @pointermove="onRailMove"
        @pointerup="onRailUp"
        @keydown="onKey"
      >
        <span class="rail-track" />
        <span class="rail-fill" :style="{ transform: `scaleX(${position})` }" />
        <span class="rail-thumb" :style="{ left: `${position * 100}%` }" />
      </div>
      <div class="zoom flex shrink-0 items-center" role="group" aria-label="Zoom">
        <button class="btn-icon btn-sm press" aria-label="Zoom out" title="Zoom out (−)" @click="zoomTo(zoomTarget / 1.35)">
          <Icon name="subtract" />
        </button>
        <button class="zoom-value text-small tabular-nums press" title="Overview (O)" @click="toggleOverview">{{ zoomLabel }}</button>
        <button class="btn-icon btn-sm press" aria-label="Zoom in" title="Zoom in (+)" @click="zoomTo(zoomTarget * 1.35)">
          <Icon name="add" />
        </button>
      </div>
      <div class="flex shrink-0">
        <button class="btn-icon press" aria-label="Open folder" title="Open folder" @click="openFolder"><Icon name="folder" /></button>
        <button class="btn-icon press" aria-label="Show as grid" title="Show as grid" @click="homeMode = 'grid'"><Icon name="grid" /></button>
        <button class="btn-icon press" aria-label="Help" title="Help" @click="go({ name: 'help' })"><Icon name="help" /></button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.home {
  --spring: var(--ease-spring);
  background: var(--c-surface);
}

.stage {
  perspective: 4000px;
  perspective-origin: 50% 50%;
  outline: none;
  touch-action: none;
}

.world {
  position: absolute;
  left: 50%;
  top: 62%;
  transform-style: preserve-3d;
  will-change: transform;
}
.layer {
  position: absolute;
  transform-style: preserve-3d;
}

.card {
  position: absolute;
  left: calc(v-bind(W) * -0.5px);
  top: calc(v-bind(H) * -1px);
  width: calc(v-bind(W) * 1px);
  height: calc(v-bind(H) * 1px);
  transform-style: preserve-3d;
  transform-origin: 50% 100%;
  will-change: transform, opacity;
  opacity: 0;
}

.poster {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: var(--c-surface-raised);
  outline: 1px solid var(--c-line);
  outline-offset: -1px;
}
.poster img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.poster::before {
  content: "";
  position: absolute;
  inset: 0;
  z-index: 1;
  background: var(--c-dim);
  opacity: max(0, var(--dim, 0));
  pointer-events: none;
}
.stage:focus-visible .is-focus .poster {
  outline: 2px solid var(--c-focus-ring);
  outline-offset: 4px;
}

.poster-play {
  position: absolute;
  top: 50%;
  left: 50%;
  z-index: 3;
  display: grid;
  width: 55px;
  height: 55px;
  place-items: center;
  color: var(--c-on-red);
  background: var(--c-red);
  opacity: 0;
  transform: translate3d(-50%, -50%, 1px) scale(0.6);
  transition:
    opacity 0.2s,
    transform 0.35s var(--spring),
    background 0.12s;
}
.card:hover .poster-play {
  opacity: 1;
  transform: translate3d(-50%, -50%, 1px) scale(1);
}
.poster-play:hover {
  background: var(--c-red-deep);
  color: var(--brand-paper);
}
.poster-play:active {
  transform: translate3d(-50%, -50%, 1px) scale(0.92);
}

.poster-title {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: var(--space-21);
  text-align: center;
  font-size: var(--text-heading);
  line-height: var(--text-heading--line-height);
  font-weight: 600;
  color: var(--c-ink-muted);
}

.slot {
  position: absolute;
  inset: auto 0 0 0;
  z-index: 2;
  height: 4px;
  background: color-mix(in srgb, var(--c-ink) 20%, transparent);
}
.slot > span {
  display: block;
  height: 100%;
  background: var(--c-mark);
}

.overlay {
  position: absolute;
  z-index: 1;
}
.chrome {
  animation: chrome-in 0.8s var(--spring) backwards;
  animation-delay: calc(var(--d) * 1ms + 150ms);
  transition: opacity var(--dur-2) var(--ease-out);
}
.leaving .chrome {
  opacity: 0;
}
@keyframes chrome-in {
  from {
    opacity: 0;
    translate: 0 13px;
  }
}

.swap-stack {
  display: grid;
}
.swap-item {
  grid-area: 1 / 1;
  min-width: 0;
}
.swap-enter-active {
  transition:
    opacity var(--dur-3) var(--ease-out),
    translate var(--dur-3) var(--spring);
}
.swap-leave-active {
  transition:
    opacity var(--dur-2) var(--ease-out),
    translate var(--dur-2) var(--ease-out);
}
.swap-enter-from {
  opacity: 0;
  translate: 0 8px;
}
.swap-leave-to {
  opacity: 0;
  translate: 0 -5px;
}

.tip {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 2;
  max-width: 260px;
  padding: var(--space-8) var(--space-13);
  pointer-events: none;
  background: var(--c-surface-raised);
  border: 1px solid var(--c-line);
  transition: transform 0.08s linear;
}
.tip-enter-active,
.tip-leave-active {
  transition:
    opacity 0.16s ease,
    scale 0.2s var(--spring);
}
.tip-enter-from,
.tip-leave-to {
  opacity: 0;
  scale: 0.94;
}

.press {
  transition:
    scale 0.25s var(--spring),
    background 0.12s,
    color 0.12s;
}
.press:active {
  scale: 0.94;
}

.filter {
  display: flex;
  height: 21px;
  align-items: flex-start;
  font-size: var(--text-body);
  line-height: var(--text-body--line-height);
  color: var(--c-ink-muted);
  transition:
    color 0.15s ease,
    translate 0.35s var(--spring);
  animation: chrome-in 0.7s var(--spring) backwards;
  animation-delay: calc(var(--n) * 40ms + 400ms);
}
.filter:hover {
  color: var(--c-ink);
  translate: -5px 0;
}
.filter[aria-pressed="true"] {
  font-weight: 600;
  color: var(--c-ink);
}
.filter sup {
  top: 0;
  min-width: 21px;
  margin-left: 3px;
  font-size: var(--text-small);
  line-height: 12px;
  text-align: left;
}

kbd {
  color: var(--c-ink);
}

.rail {
  position: relative;
  width: 233px;
  height: 34px;
  flex-shrink: 0;
  touch-action: none;
}
.rail-track,
.rail-fill {
  position: absolute;
  inset: 15px 0 auto;
  height: 4px;
  background: var(--c-line);
  transition: height 0.2s var(--spring), inset 0.2s var(--spring);
}
.rail-fill {
  background: var(--c-ink-muted);
  transform-origin: 0 50%;
  transition:
    transform 0.4s var(--spring),
    height 0.2s var(--spring),
    inset 0.2s var(--spring);
}
.rail:hover .rail-track,
.rail:hover .rail-fill {
  inset: 13px 0 auto;
  height: 8px;
}
.rail-thumb {
  position: absolute;
  top: 7px;
  width: 4px;
  height: 20px;
  margin-left: -2px;
  background: var(--c-ink);
  transition:
    left 0.4s var(--spring),
    scale 0.25s var(--spring);
}
.rail:hover .rail-thumb {
  scale: 1 1.25;
}

.zoom {
  border: 1px solid var(--c-line);
  padding: 0 3px;
}
.zoom-value {
  min-width: 55px;
  height: 34px;
  text-align: center;
  font-size: var(--text-small);
}
.zoom-value:hover {
  color: var(--c-ink);
}
</style>
