<script setup lang="ts">
/**
 * The first-start guide's welcome picture: what Melue is at a glance. A computer and a phone, kept in step through the
 * NAS between them: the lines to it and the NAS itself pulse slowly, nearly still.
 */
import { HardDrive, Laptop, Smartphone } from '@lucide/vue'
</script>

<template>
  <div class="relative mx-auto flex h-32 w-full max-w-[360px] items-center justify-between" aria-hidden="true">
    <div class="welcome-line absolute inset-x-12 top-1/2 h-0.5 -translate-y-1/2 rounded-full bg-muted-foreground" />
    <span class="relative grid size-20 place-items-center rounded-2xl border bg-popover text-popover-foreground shadow-sm"><Laptop class="size-9" :stroke-width="1.5" /></span>
    <span class="welcome-nas relative grid size-16 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-sm"><HardDrive class="size-7" :stroke-width="1.5" /></span>
    <span class="relative grid size-20 place-items-center rounded-2xl border bg-popover text-popover-foreground shadow-sm"><Smartphone class="size-9" :stroke-width="1.5" /></span>
  </div>
</template>

<style scoped>
/* A slow breath: the lines brighten and dim, and a soft ring swells from the NAS. */
.welcome-line {
  animation: welcome-line 4s ease-in-out infinite;
}
.welcome-nas::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
  box-shadow: 0 0 0 0 var(--primary);
  animation: welcome-ring 4s ease-in-out infinite;
}
@keyframes welcome-line {
  0%,
  100% {
    opacity: 0.25;
  }
  50% {
    opacity: 0.6;
  }
}
@keyframes welcome-ring {
  0%,
  100% {
    box-shadow: 0 0 0 0 color-mix(in oklch, var(--primary) 0%, transparent);
  }
  50% {
    box-shadow: 0 0 0 8px color-mix(in oklch, var(--primary) 18%, transparent);
  }
}
@media (prefers-reduced-motion: reduce) {
  .welcome-line,
  .welcome-nas::after {
    animation: none;
  }
}
</style>
