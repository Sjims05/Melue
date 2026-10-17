<script setup lang="ts">
/**
 * A top bar (Melue's design). The main pages share one, which stays put: the Melue logo at the content's left edge.
 * A page opened on top brings its own, sliding in with it over the one under it: "‹ back" there instead, named after
 * the page it came from. At the window's right edge: search and the sync status (Search has its own big field, so its
 * bar has only the sync status), and the window buttons: each bar has its own, so a page on top covers all of it.
 * Its empty parts drag the window (double-click: maximize), as the window has no system frame.
 */
import { ChevronLeft } from '@lucide/vue'
import mark from '@/assets/melue-mark.png'
import { goBack } from '@/router'
import SyncChip from './SyncChip.vue'
import TopSearch from './TopSearch.vue'
import WindowButtons from './WindowButtons.vue'

withDefaults(defineProps<{ back?: string | null; search?: boolean }>(), { back: null, search: true })
</script>

<template>
  <header class="flex h-[88px] flex-none items-center gap-4 bg-background pt-2 pr-5 pl-[var(--col-left)]" data-tauri-drag-region>
    <button
      v-if="back"
      type="button"
      class="-ml-2 flex h-9 items-center gap-2 rounded-md px-2 text-[19px] font-semibold transition-colors hover:bg-card"
      @click="goBack"
    >
      <ChevronLeft class="size-5" />{{ back }}
    </button>
    <RouterLink v-else to="/" class="flex items-center gap-2.5 text-[19px] font-semibold" aria-label="Melue: Home">
      <!-- The app icon itself (app-icon.png, scaled down), inverted in dark mode (a light tile on the dark page). -->
      <img :src="mark" alt="" class="size-6 dark:invert" draggable="false" />
      melue
    </RouterLink>
    <div class="ml-auto flex items-center gap-3" data-tauri-drag-region>
      <TopSearch v-if="search" />
      <SyncChip />
      <WindowButtons class="ml-2" />
    </div>
  </header>
</template>
