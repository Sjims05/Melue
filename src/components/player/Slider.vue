<script setup lang="ts">
/**
 * A slider (Melue's design): a thin track, the part up to the value filled, a round handle under the pointer or while
 * dragging. Seeking (`change` when let go; `live` also while dragging, for volume). Arrow keys move it by `step`.
 * `vertical`: standing up, filled from the bottom (the player bar's volume); it takes the height it's given. `ticks`: a
 * value in that many steps (a phone's volume), each marked on the track when there's room for the marks.
 *
 * Let go, it stays where you put it until the value catches up (the player's answer), not back at the old value in
 * between. On a touch screen a sideways drag moves it and a tap puts it there; an up or down drag is the page
 * scrolling, not the slider (the browser takes it, and nothing is changed).
 */
import { useElementSize } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useSlide } from './slide'

const props = withDefaults(defineProps<{ value: number; max: number; step?: number; label: string; vertical?: boolean; ticks?: number }>(), { step: 5 })
const emit = defineEmits<{ change: [value: number]; live: [value: number]; dragging: [on: boolean] }>()

const track = ref<HTMLElement>()
// While dragging, the handle follows the pointer, not the value (which only changes when let go).
const dragging = ref<number | null>(null)
// Let go: shown there until the value comes to it (or a moment passes).
const held = ref<number | null>(null)
let letGo: ReturnType<typeof setTimeout> | undefined
function hold(value: number) {
  held.value = value
  clearTimeout(letGo)
  letGo = setTimeout(() => (held.value = null), 1200)
}
watch(
  () => props.value,
  (value) => {
    if (held.value !== null && Math.abs(value - held.value) <= (props.max || 1) * 0.01) held.value = null
  },
)
onBeforeUnmount(() => clearTimeout(letGo))
// The steps' marks (between the ends), when they're at least 4 px apart.
const size = useElementSize(track)
const marks = computed(() => {
  const n = props.ticks ?? 0
  const length = props.vertical ? size.height.value : size.width.value
  return n > 1 && length / n >= 4 ? Array.from({ length: n - 1 }, (_, i) => (i + 1) / n) : []
})
const shown = computed(() => (dragging.value ?? held.value ?? props.value) / (props.max || 1))
// A jump (not a frame's move, not a drag): it slides there. Not just after you moved it (the drag, a click, the arrow
// keys): it's already where you put it, and the player's answer coming in would make it slide back and forth.
let movedAt = 0
const OWN_MOVE_MS = 1500
const sliding = useSlide(
  () => props.value,
  () => props.max,
  () => dragging.value !== null || performance.now() - movedAt < OWN_MOVE_MS,
)
const SLIDES = 'transition-[width,height,left,bottom] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]'

function at(e: PointerEvent) {
  const box = track.value!.getBoundingClientRect()
  const part = props.vertical ? (box.bottom - e.clientY) / box.height : (e.clientX - box.left) / box.width
  const clamped = Math.min(1, Math.max(0, part))
  // In steps: the nearest one (the handle jumps from step to step).
  return (props.ticks ? Math.round(clamped * props.ticks) / props.ticks : clamped) * props.max
}
// A finger down, not yet moved: a tap, a sideways drag (the slider's) or the page scrolling (not the slider's).
let touch: { x: number; y: number } | null = null
function down(e: PointerEvent) {
  if (e.pointerType === 'touch' && !props.vertical) touch = { x: e.clientX, y: e.clientY }
  else grab(e)
}
function grab(e: PointerEvent) {
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  dragging.value = at(e)
  emit('dragging', true)
  emit('live', dragging.value)
}
function move(e: PointerEvent) {
  if (touch) {
    const dx = e.clientX - touch.x
    const dy = e.clientY - touch.y
    if (Math.abs(dx) > 6 && Math.abs(dx) > Math.abs(dy)) {
      touch = null
      grab(e)
    } else if (Math.abs(dy) > 6) touch = null
    return
  }
  if (dragging.value === null) return
  dragging.value = at(e)
  emit('live', dragging.value)
}
function up(e: PointerEvent) {
  // A tap: there.
  if (touch) {
    touch = null
    const value = at(e)
    hold(value)
    movedAt = performance.now()
    emit('live', value)
    emit('change', value)
    return
  }
  if (dragging.value === null) return
  emit('change', dragging.value)
  hold(dragging.value)
  dragging.value = null
  movedAt = performance.now()
  emit('dragging', false)
}
// The browser took the touch (the page scrolling): nothing changes.
function cancel() {
  touch = null
  if (dragging.value === null) return
  dragging.value = null
  emit('dragging', false)
}
function key(e: KeyboardEvent) {
  const by = e.key === 'ArrowRight' || e.key === 'ArrowUp' ? props.step : e.key === 'ArrowLeft' || e.key === 'ArrowDown' ? -props.step : 0
  if (!by) return
  e.preventDefault()
  e.stopPropagation()
  const value = Math.min(props.max, Math.max(0, props.value + by))
  movedAt = performance.now()
  emit('live', value)
  emit('change', value)
}
</script>

<template>
  <div
    class="group/slider relative flex cursor-pointer items-center outline-none"
    :class="vertical ? 'h-full w-5 touch-none justify-center' : 'h-5 touch-pan-y'"
    role="slider"
    :aria-orientation="vertical ? 'vertical' : undefined"
    tabindex="0"
    :aria-label="label"
    :aria-valuemin="0"
    :aria-valuemax="max"
    :aria-valuenow="Math.round(value)"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="cancel"
    @keydown="key"
  >
    <div ref="track" class="relative overflow-hidden rounded-full bg-muted-foreground/30" :class="vertical ? 'h-full w-1' : 'h-1 w-full'">
      <div
        class="absolute rounded-full bg-foreground"
        :class="[vertical ? 'inset-x-0 bottom-0' : 'inset-y-0 left-0', sliding && SLIDES]"
        :style="vertical ? { height: `${shown * 100}%` } : { width: `${shown * 100}%` }"
      />
      <!-- Notches in the track, one per step. -->
      <span
        v-for="mark in marks"
        :key="mark"
        class="absolute bg-background"
        :class="vertical ? 'inset-x-0 h-px' : 'inset-y-0 w-px'"
        :style="vertical ? { bottom: `${mark * 100}%` } : { left: `${mark * 100}%` }"
      />
    </div>
    <span
      class="pointer-events-none absolute size-3 rounded-full bg-foreground shadow transition-opacity duration-150"
      :class="[
        sliding && SLIDES,
        vertical ? 'translate-y-1/2' : '-translate-x-1/2',
        dragging !== null ? 'opacity-100' : 'opacity-0 group-hover/slider:opacity-100 group-focus-visible/slider:opacity-100',
      ]"
      :style="vertical ? { bottom: `${shown * 100}%` } : { left: `${shown * 100}%` }"
    />
  </div>
</template>
