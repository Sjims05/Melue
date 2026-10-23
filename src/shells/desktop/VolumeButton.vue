<script setup lang="ts">
/**
 * The player bar's volume: a button whose icon shows the level. Under the pointer (or focused), a small card rises
 * above it with the volume standing up; it stays while the slider is dragged, and goes a moment after the pointer
 * leaves both. A click mutes (and a second one brings the volume back); the mouse wheel over either turns it up or
 * down. With the keyboard: focused, the card shows, and Up / Down turn it up or down; the + and − keys (lib/hotkeys.ts)
 * show it a moment too (not while the player bar is away, on Now playing: the page shows it). The card is placed over
 * the page (the player bar clips what goes past it).
 */
import { Volume1, Volume2, VolumeX } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { volumeFlash } from '@/lib/hotkeys'
import Slider from '@/components/player/Slider.vue'
import { nowPlaying, setVolume } from '@/player/nowPlaying'

defineProps<{ iconClass: string }>()

// The volume moves with the slider at once; the player follows.
const moving = ref<number | null>(null)
const volume = computed(() => moving.value ?? nowPlaying.state?.volume ?? 1)
const Icon = computed(() => (volume.value === 0 ? VolumeX : volume.value < 0.5 ? Volume1 : Volume2))
function change(v: number) {
  moving.value = Math.min(1, Math.max(0, v))
  setVolume(moving.value).finally(() => (moving.value = null))
}
// Muting remembers the volume it had, for the next click.
let before = 1
function toggleMute() {
  if (volume.value > 0) {
    before = volume.value
    change(0)
  } else change(before || 1)
}
const wheel = (e: WheelEvent) => change(volume.value - Math.sign(e.deltaY) * 0.05)

// The card: open while the pointer is on the button or the card, while dragging, or while focused in.
const button = ref<HTMLElement>()
const over = ref(false)
const dragging = ref(false)
const focused = ref(false)
const open = computed(() => over.value || dragging.value || focused.value)
const place = ref({ left: 0, bottom: 0 })
let leaving: ReturnType<typeof setTimeout> | undefined
function enter() {
  clearTimeout(leaving)
  const box = button.value!.getBoundingClientRect()
  // Centred over the button (the card is 44 px wide: w-11).
  place.value = { left: box.left + box.width / 2 - 22, bottom: innerHeight - box.top + 8 }
  over.value = true
}
function leave() {
  leaving = setTimeout(() => (over.value = false), 200)
}
// The volume keys: the card a moment, showing the new level.
watch(
  () => volumeFlash.at,
  () => {
    // The bar slid away (Now playing): its button is below the window's edge, and the page shows the volume itself.
    if (!button.value?.isConnected || !button.value.offsetParent || button.value.getBoundingClientRect().top >= innerHeight) return
    enter()
    leaving = setTimeout(() => (over.value = false), 1200)
  },
)
</script>

<template>
  <div class="relative" @pointerenter="enter" @pointerleave="leave" @focusin="(e: FocusEvent) => (e.target as HTMLElement).matches(':focus-visible') && (enter(), (focused = true))" @focusout="focused = false" @wheel.prevent="wheel">
    <button
      ref="button"
      type="button"
      :class="iconClass"
      :aria-label="volume === 0 ? 'Unmute' : 'Mute'"
      :title="`Volume ${Math.round(volume * 100)}%`"
      @click="toggleMute"
      @keydown.up.prevent="change(volume + 0.05)"
      @keydown.down.prevent="change(volume - 0.05)"
    >
      <component :is="Icon" />
    </button>
    <Teleport to="body">
      <Transition name="volume">
        <div
          v-if="open"
          class="fixed z-50 flex h-36 w-11 flex-col items-center gap-2 rounded-xl border bg-card px-3 pt-4 pb-3 shadow-lg"
          :style="{ left: `${place.left}px`, bottom: `${place.bottom}px` }"
          @pointerenter="enter"
          @pointerleave="leave"
          @wheel.prevent="wheel"
        >
          <Slider class="min-h-0 flex-1" vertical :value="volume" :max="1" :step="0.05" label="Volume" @live="change" @change="change" @dragging="(on) => (dragging = on)" />
          <span class="text-[11px] text-muted-foreground tabular-nums">{{ Math.round(volume * 100) }}</span>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
/* Rising out of the button: it grows from its bottom and fades in; it fades away. */
.volume-enter-active {
  transition:
    opacity 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
    scale 220ms cubic-bezier(0.2, 0.8, 0.2, 1),
    translate 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.volume-leave-active {
  transition: opacity 140ms ease-in;
}
.volume-enter-from,
.volume-leave-to {
  opacity: 0;
}
.volume-enter-from {
  scale: 0.9;
  translate: 0 6px;
  transform-origin: bottom center;
}
@media (prefers-reduced-motion: reduce) {
  .volume-enter-active,
  .volume-leave-active {
    transition: none;
  }
}
</style>
