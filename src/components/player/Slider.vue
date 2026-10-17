<script setup lang="ts">
/**
 * A slider (Melue's design): a thin track, the part up to the value filled, a round handle under the pointer or while
 * dragging. Seeking (`change` when let go; `live` also while dragging, for volume). Arrow keys move it by `step`.
 */
import { computed, ref } from 'vue'

const props = withDefaults(defineProps<{ value: number; max: number; step?: number; label: string }>(), { step: 5 })
const emit = defineEmits<{ change: [value: number]; live: [value: number] }>()

const track = ref<HTMLElement>()
// While dragging, the handle follows the pointer, not the value (which only changes when let go).
const dragging = ref<number | null>(null)
const shown = computed(() => (dragging.value ?? props.value) / (props.max || 1))

function at(e: PointerEvent) {
  const box = track.value!.getBoundingClientRect()
  return Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) * props.max
}
function down(e: PointerEvent) {
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  dragging.value = at(e)
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
    class="group/slider relative flex h-5 cursor-pointer touch-none items-center outline-none"
    role="slider"
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
    <div ref="track" class="relative h-1 w-full overflow-hidden rounded-full bg-muted-foreground/30">
      <div class="absolute inset-y-0 left-0 rounded-full bg-foreground" :style="{ width: `${shown * 100}%` }" />
    </div>
    <span
      class="pointer-events-none absolute size-3 -translate-x-1/2 rounded-full bg-foreground shadow transition-opacity duration-150"
      :class="dragging !== null ? 'opacity-100' : 'opacity-0 group-hover/slider:opacity-100 group-focus-visible/slider:opacity-100'"
      :style="{ left: `${shown * 100}%` }"
    />
  </div>
</template>
