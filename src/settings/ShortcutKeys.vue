<script setup lang="ts">
/**
 * A shortcut's keys, as key caps (KeyboardShortcuts.vue's look); clicked, the next keys pressed are the new ones (with
 * Ctrl, Alt or the Windows key: a key alone would be taken from every app). Esc: as it was; Backspace: none.
 * `keys`: as the shortcut plugin reads them ("Control+Alt+ArrowUp"; the keys by their place, as KeyboardEvent.code).
 */
import { computed, onBeforeUnmount, ref } from 'vue'

const props = defineProps<{ keys: string; label: string }>()
const emit = defineEmits<{ change: [keys: string] }>()

const NAMES: Record<string, string> = {
  Control: 'Ctrl',
  Super: 'Win',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Space: 'Space',
  PageUp: 'Page Up',
  PageDown: 'Page Down',
}
/** A key's cap: "KeyA" A, "Digit1" 1, "ArrowUp" ↑. */
const cap = (part: string) => NAMES[part] ?? part.replace(/^Key|^Digit/, '').replace(/^Numpad/, 'Num ')
const caps = computed(() => (props.keys ? props.keys.split('+').map(cap) : []))

const listening = ref(false)
function onKey(e: KeyboardEvent) {
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') return stop()
  if (e.key === 'Backspace' || e.key === 'Delete') {
    emit('change', '')
    return stop()
  }
  // Only the modifiers so far: waiting for the key.
  if (['Control', 'Alt', 'Shift', 'Meta', 'OS'].includes(e.key)) return
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return
  const parts = [e.ctrlKey && 'Control', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super', e.code].filter(Boolean)
  emit('change', parts.join('+'))
  stop()
}
function start() {
  listening.value = true
  window.addEventListener('keydown', onKey, true)
}
function stop() {
  listening.value = false
  window.removeEventListener('keydown', onKey, true)
}
onBeforeUnmount(stop)
</script>

<template>
  <button
    type="button"
    class="flex h-8 items-center gap-1 rounded-md px-2 transition-colors hover:bg-well"
    :class="listening && 'bg-well ring-2 ring-ring/50'"
    :aria-label="listening ? `${label}: press the new keys (Esc: as it was, Backspace: none)` : `${label}: ${caps.join(' ') || 'none'}, change`"
    @click="listening ? stop() : start()"
    @blur="stop"
  >
    <span v-if="listening" class="px-1 text-sm text-muted-foreground">Press the keys…</span>
    <template v-else-if="caps.length">
      <kbd v-for="(k, i) in caps" :key="i" class="min-w-6 rounded border border-b-2 bg-well px-1.5 py-0.5 text-center font-sans text-xs">{{ k }}</kbd>
    </template>
    <span v-else class="px-1 text-sm text-muted-foreground">None</span>
  </button>
</template>
