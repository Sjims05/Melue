<script setup lang="ts">
/**
 * A place for something that's planned but not built yet (Lyrics on Now playing; a page still to come): a card with
 * a lock, its name, and what's coming. Not clickable.
 */
import { Lock } from '@lucide/vue'

defineProps<{ title: string; text?: string }>()
</script>

<template>
  <div class="flex flex-col gap-2 rounded-xl bg-card p-5 text-card-foreground" aria-disabled="true">
    <div class="flex items-center gap-2 text-[15px] font-semibold"><Lock class="size-4 text-muted-foreground" />{{ title }}</div>
    <p v-if="text" class="text-sm text-muted-foreground">{{ text }}</p>
  </div>
</template>
