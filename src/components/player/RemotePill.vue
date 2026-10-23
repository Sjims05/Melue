<script setup lang="ts">
/**
 * The connection, where toasts show (shells/Toasts.vue: they come above it), for as long as it lasts: "Playing on
 * <device>" on the device controlling, "Played from <device>" on the one played on. The toasts' look, as a pill (a
 * state, not a note that goes). Tapping it opens Connect device (a menu on a computer, the drawer on a phone). While
 * it shows, pages get its room at their end (style.css: data-connection-note), so their last row scrolls clear of it.
 */
import { MonitorSpeaker } from '@lucide/vue'
import { computed } from 'vue'
import DevicesMenu from '@/components/player/DevicesMenu.vue'
import { openDevicesDrawer } from '@/player/remotePlayback'
import { connectionText } from '@/player/playingOn'

defineProps<{ phone?: boolean }>()

const text = computed(connectionText)
const PILL =
  'pointer-events-auto flex max-w-full items-center gap-2 rounded-full bg-foreground px-4 py-1.5 text-[13.5px] text-background shadow-lg transition-transform active:scale-95 [&_svg]:size-4 [&_svg]:flex-none'
</script>

<template>
  <Transition name="drop">
    <div v-if="text" data-connection-note class="flex max-w-full justify-center">
      <button v-if="phone" type="button" :class="PILL" @click="openDevicesDrawer">
        <MonitorSpeaker /><span class="truncate">{{ text }}</span>
      </button>
      <!-- Well apart: the pill and the menu are opposite colours. -->
      <DevicesMenu v-else side="top" align="center" :gap="12">
        <button type="button" :class="PILL"><MonitorSpeaker /><span class="truncate">{{ text }}</span></button>
      </DevicesMenu>
    </div>
  </Transition>
</template>
