<script setup lang="ts">
/**
 * A line of text that changes now and then (a count: "12 songs · 3 only here", a setting's grey line): the new text
 * fades in as the old one goes (settings.css: swap). Text that changes quickly (a download's percentage, a sync's file
 * count) just changes: a fade every moment would flicker.
 */
import { ref, watch } from 'vue'

const props = defineProps<{ text: string }>()

/** Changes closer together than this aren't faded. */
const QUICK_MS = 700
const shown = ref(props.text)
// The fade runs when the key changes: kept the same for a quick change.
const key = ref(0)
let last = 0
watch(
  () => props.text,
  (text) => {
    const now = performance.now()
    if (now - last > QUICK_MS) key.value++
    last = now
    shown.value = text
  },
)
</script>

<template>
  <Transition name="swap" mode="out-in">
    <span :key="key">{{ shown }}</span>
  </Transition>
</template>
