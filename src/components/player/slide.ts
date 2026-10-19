/**
 * A progress bar's jumps: the position moves on a little each frame while playing, but sometimes it jumps (the player's
 * word after the app was away, another device's position, a seek). Then the bar slides there for a moment instead of
 * snapping. `sliding` is true for that moment; small moves (a frame's worth) never start it.
 */
import { onBeforeUnmount, ref, watch, type WatchSource } from 'vue'

const SLIDE_MS = 350

export function useSlide(value: WatchSource<number>, max: () => number, paused: () => boolean = () => false) {
  const sliding = ref(false)
  let done: ReturnType<typeof setTimeout> | undefined
  watch(value, (now, before) => {
    if (paused() || before === undefined) return
    if (Math.abs(now - before) <= (max() || 1) * 0.01) return
    sliding.value = true
    clearTimeout(done)
    done = setTimeout(() => (sliding.value = false), SLIDE_MS)
  })
  onBeforeUnmount(() => clearTimeout(done))
  return sliding
}
