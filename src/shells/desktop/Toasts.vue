<script setup lang="ts">
/**
 * The note at the bottom of the window (lib/toast.ts), above the player bar: drops in, fades out. Read out by screen
 * readers as it comes (a status).
 */
import { runToastAction, shownToast } from '@/lib/toast'
</script>

<template>
  <div class="pointer-events-none absolute inset-x-0 bottom-5 z-30 flex justify-center px-4" role="status" aria-live="polite">
    <Transition name="drop" mode="out-in">
      <div
        v-if="shownToast"
        :key="shownToast.id"
        class="flex max-w-full items-center gap-2 rounded-lg bg-foreground px-3.5 py-2 text-[13.5px] text-background shadow-lg"
      >
        <component :is="shownToast.icon" v-if="shownToast.icon" class="size-4 flex-none" />
        <span class="truncate">{{ shownToast.text }}</span>
        <button
          v-if="shownToast.action"
          type="button"
          class="pointer-events-auto -my-1 -mr-1.5 ml-1 flex-none rounded-md px-2 py-1 font-semibold transition-colors hover:bg-background/15"
          @click="runToastAction"
        >
          {{ shownToast.action.label }}
        </button>
      </div>
    </Transition>
  </div>
</template>
