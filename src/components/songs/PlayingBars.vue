<script setup lang="ts">
/**
 * The song playing: three bars rising and falling (the playground's: 30% → 100% → 30% of the height in 900 ms, each a
 * third of a cycle apart). Pausing hands each bar to gravity: one that's rising slows, stops and falls, and they land
 * flat with a small bounce (Simon picked it over a plain drop, 2026-10-17). Playing again makes them rise smoothly from
 * where they are into the bounce. Reduce motion: they stand still (flat while paused).
 *
 * Moved by a small animation loop (not CSS), so each bar can go on from wherever it is; the loop stops once they're
 * flat.
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps<{ paused?: boolean }>()

const HEIGHT = 11 // px, as .song-bars
const FLOOR = 2 // px: flat
const CYCLE = 900 // ms
const PHASES = [0, 300, 600] // ms: each bar a third of a cycle on
const CATCH_UP = 70 // ms: how fast a bar catches up with the bounce (playing again: no jump)
const GRAVITY = 260 // px/s²
const BOUNCE = 0.25 // of the speed it lands with

const bars = ref<HTMLElement>()
const still = window.matchMedia('(prefers-reduced-motion: reduce)')

// Each bar's height (px) and speed (px/s, up is positive).
const state = PHASES.map(() => ({ h: FLOOR, v: 0 }))
let frame = 0
let last = 0

/** Where the bounce has a bar now: 30% → 100% → 30%, eased like the playground's (a cosine). */
const bounce = (t: number, i: number) => HEIGHT * (0.3 + 0.7 * (1 - Math.cos((2 * Math.PI * (t + PHASES[i])) / CYCLE)) / 2)

function draw() {
  const items = bars.value?.children
  if (!items) return
  state.forEach((s, i) => ((items[i] as HTMLElement).style.height = `${s.h.toFixed(2)}px`))
}

function step(now: number) {
  const dt = Math.min(0.05, (now - (last || now)) / 1000)
  last = now
  let moving = !props.paused
  state.forEach((s, i) => {
    if (!props.paused) {
      // Follow the bounce, catching up smoothly (after playing again, from wherever it is).
      const target = bounce(now, i)
      const h = s.h + (target - s.h) * (1 - Math.exp((-dt * 1000) / CATCH_UP))
      s.v = dt ? (h - s.h) / dt : 0
      s.h = h
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
  frame = moving ? requestAnimationFrame(step) : 0
}

function run() {
  if (still.matches) {
    // Reduce motion: standing still (uneven while playing, flat while paused).
    state.forEach((s, i) => (s.h = props.paused ? FLOOR : HEIGHT * [0.55, 1, 0.75][i]))
    return draw()
  }
  if (!frame) {
    last = 0
    frame = requestAnimationFrame(step)
  }
}

watch(
  () => props.paused,
  () => run(),
)
onMounted(() => {
  // Shown while already playing: in the bounce at once.
  if (!props.paused) state.forEach((s, i) => (s.h = bounce(performance.now(), i)))
  draw()
  run()
})
onBeforeUnmount(() => cancelAnimationFrame(frame))
</script>

<template>
  <span ref="bars" class="song-bars" :aria-label="paused ? 'Paused' : 'Playing'"><i /><i /><i /></span>
</template>
