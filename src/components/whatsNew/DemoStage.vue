<script setup lang="ts">
/**
 * New in Melue's stage: a feature's live demo on a background of its own (a soft glow of the accent colour), so it
 * reads as a picture of it, not the thing itself. Looked at, not used: clicks and the keyboard don't reach it.
 * `scale`: the demo smaller (or bigger) as a whole: zoom scales its sizes, text and gaps alike and the room it takes too
 * (a transform would leave its full-size box), so the layout stays exactly the app's.
 */
defineProps<{ scale?: number }>()
</script>

<template>
  <div class="demo-stage relative grid place-items-center overflow-hidden rounded-3xl px-6 py-10" aria-hidden="true">
    <div class="pointer-events-none select-none" :style="{ zoom: scale ?? 1 }" inert>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.demo-stage {
  background:
    radial-gradient(120% 90% at 50% 0%, color-mix(in oklch, var(--primary) 32%, transparent), transparent 70%),
    radial-gradient(90% 80% at 100% 100%, color-mix(in oklch, var(--primary) 18%, transparent), transparent 70%),
    var(--well);
}
</style>
