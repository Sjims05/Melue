<script setup lang="ts">
/**
 * The song playing: three bars rising and falling (the playground's: 30% → 100% → 30% of the height in 900 ms, each a
 * third of a cycle apart). Pausing hands each bar to gravity: one that's rising slows, stops and falls, and they land
 * flat with a small bounce (picked over a plain drop). Playing again makes them rise smoothly from
 * where they are into the bounce. Reduce motion: they stand still (flat while paused).
 *
 * Cheap to show (a song plays for hours): each bar is a full-height bar sliding in its own rounded box (it clips it:
 * round at both ends at any height, nothing laid out or painted as it moves). The steady bounce is the browser's own
 * animation of that slide (the same curve, sampled), run without the page (and not drawn while out of view); a small
 * animation loop only moves them from wherever they are, pausing (gravity) and playing again (catching up: how far
 * each is from the bounce fades away), and hands over once they're in the bounce.
 */
import { onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from 'vue'

const props = defineProps<{ paused?: boolean }>()

const HEIGHT = 11 // px, as .song-bars
const FLOOR = 2 // px: flat
const CYCLE = 900 // ms
const PHASES = [0, 300, 600] // ms: each bar a third of a cycle on
const CATCH_UP = 70 // ms: how fast a bar catches up with the bounce (playing again: no jump)
const GRAVITY = 260 // px/s²
const BOUNCE = 0.25 // of the speed it lands with
/** Close enough to the bounce to hand over to its animation (px). */
const CAUGHT_UP = 0.05
/** Points of the bounce the animation goes through (straight between them: a smooth curve at 11 px). */
const SAMPLES = 36
/** It moves on 20 times a second, not every frame (a phone's own drawing puts each frame on screen: at 11 px, 20 a
 * second looks the same). */
const STEPS = Math.round(CYCLE / 50)

const bars = ref<HTMLElement>()
const still = window.matchMedia('(prefers-reduced-motion: reduce)')

// Each bar's height (px), speed (px/s, up is positive) and, catching up, how far it is from the bounce (px).
const state = PHASES.map(() => ({ h: FLOOR, v: 0, off: 0 }))
let frame = 0
let last = 0
let catching = false
/** The bounce's animations, one per bar, while they're in it. */
let steady: Animation[] = []

const W = (2 * Math.PI) / CYCLE
/** Where the bounce has a bar now: 30% → 100% → 30%, eased like the playground's (a cosine). */
const bounce = (t: number, i: number) => HEIGHT * (0.3 + 0.7 * (1 - Math.cos(W * (t + PHASES[i]))) / 2)
/** How fast it's moving there (px/s, up is positive). */
const bounceSpeed = (t: number, i: number) => HEIGHT * 0.35 * W * Math.sin(W * (t + PHASES[i])) * 1000

const slide = (h: number) => `translateY(${(HEIGHT - h).toFixed(2)}px)`
const inners = () => [...(bars.value?.querySelectorAll<HTMLElement>(':scope > i > b') ?? [])]

function draw() {
  inners().forEach((b, i) => (b.style.transform = slide(state[i].h)))
}

/** The bounce, one cycle from its start: where the slide goes. */
const KEYFRAMES = Array.from({ length: SAMPLES + 1 }, (_, k) => ({ transform: slide(bounce((k / SAMPLES) * CYCLE, 0)) }))

/** In the bounce now: its animations take over (each bar where the bounce has it). */
function startSteady() {
  stopSteady()
  // On the page's own clock, each a third of a cycle on: every bar on screen (the mini player's, a song row's) steps at
  // the same moments, so a phone draws 20 frames a second for all of them, not 20 for each (unaligned: ~40, measured).
  steady = inners().map((b, i) => {
    const animation = b.animate(KEYFRAMES, { duration: CYCLE, iterations: Infinity, easing: `steps(${STEPS}, jump-none)` })
    animation.startTime = -PHASES[i]
    return animation
  })
}
/** Out of the bounce: each bar where it was in it, moving as it was (for gravity to go on from). */
function stopSteady() {
  if (!steady.length) return
  const now = performance.now()
  state.forEach((s, i) => {
    s.h = bounce(now, i)
    s.v = bounceSpeed(now, i)
  })
  steady.forEach((a) => a.cancel())
  steady = []
  draw()
}

function step(now: number) {
  const dt = Math.min(0.05, (now - (last || now)) / 1000)
  last = now
  let moving = !props.paused
  let caughtUp = !props.paused
  if (!props.paused && !catching) {
    // Playing again: from wherever each bar is, how far it is from the bounce (which then fades away).
    catching = true
    state.forEach((s, i) => (s.off = s.h - bounce(now, i)))
  }
  if (props.paused) catching = false
  state.forEach((s, i) => {
    if (!props.paused) {
      // Into the bounce smoothly: on it, plus what's left of how far it was.
      s.off *= Math.exp((-dt * 1000) / CATCH_UP)
      const h = Math.max(FLOOR, bounce(now, i) + s.off)
      s.v = dt ? (h - s.h) / dt : 0
      s.h = h
      if (Math.abs(s.off) > CAUGHT_UP) caughtUp = false
    } else {
      // Gravity: it keeps its speed, slowed (going up) and sped up (coming down); it lands with a small bounce.
      s.v -= GRAVITY * dt
      s.h += s.v * dt
      if (s.h <= FLOOR) {
        s.h = FLOOR
        s.v = Math.abs(s.v) > 8 ? -s.v * BOUNCE : 0
      }
      if (s.h > FLOOR || s.v !== 0) moving = true
    }
  })
  draw()
  if (caughtUp) {
    frame = 0
    catching = false
    return startSteady()
  }
  frame = moving ? requestAnimationFrame(step) : 0
}

function run() {
  if (still.matches) {
    // Reduce motion: standing still (uneven while playing, flat while paused).
    stopSteady()
    cancelAnimationFrame(frame)
    frame = 0
    state.forEach((s, i) => (s.h = props.paused ? FLOOR : HEIGHT * [0.55, 1, 0.75][i]))
    return draw()
  }
  if (props.paused) stopSteady()
  else if (steady.length) return
  if (!frame) {
    last = 0
    frame = requestAnimationFrame(step)
  }
}

watch(
  () => props.paused,
  () => run(),
)

/** Shown (again): playing, in the bounce at once; else as it should be. */
function show() {
  if (!props.paused && !still.matches) {
    state.forEach((s, i) => (s.h = bounce(performance.now(), i)))
    draw()
    startSteady()
  } else run()
}
/** Put aside (its page behind another, kept for coming back): nothing moves. Running on, its animations would keep
 * the page's main thread busy (measured: a third of a phone's core) for bars no one sees. */
function hide() {
  cancelAnimationFrame(frame)
  frame = 0
  steady.forEach((a) => a.cancel())
  steady = []
  if (!props.paused) return
  state.forEach((s) => ((s.h = FLOOR), (s.v = 0)))
  draw()
}
// Reduce motion switched on or off meanwhile.
const motionChanged = () => run()

onMounted(() => {
  show()
  still.addEventListener('change', motionChanged)
})
onActivated(show)
onDeactivated(hide)
onBeforeUnmount(() => {
  hide()
  still.removeEventListener('change', motionChanged)
})
</script>

<template>
  <span ref="bars" class="song-bars" :aria-label="paused ? 'Paused' : 'Playing'"><i><b /></i><i><b /></i><i><b /></i></span>
</template>
