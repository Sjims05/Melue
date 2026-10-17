<script setup lang="ts">
/**
 * A card of setting rows. It also sizes the trailing icons' column (Simon's rule): as wide as the card's shortest row
 * with a trailing icon is tall, at most 64 px. CSS can't size a width from a stretched height, so it's measured, and
 * again whenever the card's rows or size change.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'

const card = ref<HTMLElement>()
let resizes: ResizeObserver | undefined
let changes: MutationObserver | undefined

function sizeEnds() {
  const el = card.value
  if (!el) return
  const heights = [...el.querySelectorAll<HTMLElement>('[data-end]')].map((end) => end.parentElement?.offsetHeight ?? 0)
  el.style.setProperty('--end-size', heights.length ? `${Math.min(64, ...heights)}px` : '')
}

onMounted(() => {
  sizeEnds()
  resizes = new ResizeObserver(sizeEnds)
  resizes.observe(card.value!)
  changes = new MutationObserver(sizeEnds)
  changes.observe(card.value!, { childList: true, subtree: true })
})
onBeforeUnmount(() => {
  resizes?.disconnect()
  changes?.disconnect()
})
</script>

<template>
  <div ref="card" class="relative rounded-xl bg-card text-card-foreground">
    <slot />
  </div>
</template>
