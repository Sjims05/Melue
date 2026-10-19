<script setup lang="ts">
/**
 * The phone's top bar: the Melue logo on the main pages, "‹ back" on a page opened on top (named after the page it came
 * from), and the sync status. Dev builds: a way to the developer page (the spike's tests).
 */
import { ChevronLeft, Wrench } from '@lucide/vue'
import mark from '@/assets/melue-mark.png'
import { goBack } from '@/router'
import SyncChip from '../desktop/SyncChip.vue'

defineProps<{ back?: string | null }>()
const dev = import.meta.env.DEV
</script>

<template>
  <header class="flex h-14 flex-none items-center gap-2 bg-background px-4">
    <button v-if="back" type="button" class="-ml-2 flex h-10 min-w-0 items-center gap-1.5 rounded-md px-2 text-[17px] font-semibold" @click="goBack">
      <ChevronLeft class="size-5 flex-none" /><span class="truncate">{{ back }}</span>
    </button>
    <RouterLink v-else to="/" class="flex items-center gap-2 text-[17px] font-semibold" aria-label="Melue: Home">
      <img :src="mark" alt="" class="size-6 dark:invert" draggable="false" />
      melue
    </RouterLink>
    <div class="ml-auto flex min-w-0 items-center gap-1">
      <SyncChip />
      <RouterLink v-if="dev && !back" to="/dev" class="grid size-10 place-items-center text-muted-foreground" aria-label="Developer page">
        <Wrench class="size-4" />
      </RouterLink>
    </div>
  </header>
</template>
