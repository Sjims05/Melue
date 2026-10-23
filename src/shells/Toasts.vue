<script setup lang="ts">
/**
 * The note at the bottom of the window (lib/toast.ts), above the player bar: drops in, fades out. Read out by screen
 * readers as it comes (a status). With a connection to another device, its pill has this place (RemotePill.vue), and
 * notes come just above it.
 */
import RemotePill from '@/components/player/RemotePill.vue'
import { usePhone } from '@/lib/phone'
import { runToastAction, shownToast } from '@/lib/toast'

const phone = usePhone()
</script>

<template>
  <div class="pointer-events-none absolute inset-x-0 bottom-5 z-30 flex flex-col items-center gap-2 px-4">
    <div class="flex max-w-full justify-center" role="status" aria-live="polite">
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
    <RemotePill :phone="phone" />
  </div>
</template>
