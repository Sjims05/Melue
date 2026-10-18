<script setup lang="ts">
/**
 * A slider (Melue's design): a thin track, the part up to the value filled, a round handle under the pointer or while
 * dragging. Seeking (`change` when let go; `live` also while dragging, for volume). Arrow keys move it by `step`.
 * `vertical`: standing up, filled from the bottom (the player bar's volume); it takes the height it's given.
 */
import { computed, ref } from 'vue'

const props = withDefaults(defineProps<{ value: number; max: number; step?: number; label: string; vertical?: boolean }>(), { step: 5 })
const emit = defineEmits<{ change: [value: number]; live: [value: number]; dragging: [on: boolean] }>()

const track = ref<HTMLElement>()
// While dragging, the handle follows the pointer, not the value (which only changes when let go).
const dragging = ref<number | null>(null)
const shown = computed(() => (dragging.value ?? props.value) / (props.max || 1))

function at(e: PointerEvent) {
  const box = track.value!.getBoundingClientRect()
  const part = props.vertical ? (box.bottom - e.clientY) / box.height : (e.clientX - box.left) / box.width
  return Math.min(1, Math.max(0, part)) * props.max
}
function down(e: PointerEvent) {
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  dragging.value = at(e)
  emit('dragging', true)
  emit('live', dragging.value)
}
function move(e: PointerEvent) {
  if (dragging.value === null) return
  dragging.value = at(e)
  emit('live', dragging.value)
}
function up() {
  if (dragging.value === null) return
  emit('change', dragging.value)
  dragging.value = null
  emit('dragging', false)
}
function key(e: KeyboardEvent) {
  const by = e.key === 'ArrowRight' || e.key === 'ArrowUp' ? props.step : e.key === 'ArrowLeft' || e.key === 'ArrowDown' ? -props.step : 0
  if (!by) return
  e.preventDefault()
  e.stopPropagation()
  const value = Math.min(props.max, Math.max(0, props.value + by))
  emit('live', value)
  emit('change', value)
}
</script>

<template>
  <div
    class="group/slider relative flex cursor-pointer touch-none items-center outline-none"
    :class="vertical ? 'h-full w-5 justify-center' : 'h-5'"
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
    @pointercancel="up"
    @keydown="key"
  >
    <div ref="track" class="relative overflow-hidden rounded-full bg-muted-foreground/30" :class="vertical ? 'h-full w-1' : 'h-1 w-full'">
      <div
        class="absolute rounded-full bg-foreground"
        :class="vertical ? 'inset-x-0 bottom-0' : 'inset-y-0 left-0'"
        :style="vertical ? { height: `${shown * 100}%` } : { width: `${shown * 100}%` }"
      />
    </div>
    <span
      class="pointer-events-none absolute size-3 rounded-full bg-foreground shadow transition-opacity duration-150"
      :class="[
        vertical ? 'translate-y-1/2' : '-translate-x-1/2',
        dragging !== null ? 'opacity-100' : 'opacity-0 group-hover/slider:opacity-100 group-focus-visible/slider:opacity-100',
      ]"
      :style="vertical ? { bottom: `${shown * 100}%` } : { left: `${shown * 100}%` }"
    />
  </div>
</template>
