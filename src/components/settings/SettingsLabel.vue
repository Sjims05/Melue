<script setup lang="ts">
/** The small grey label over a group of cards ("Sync Triggers"); `mark` adds a badge, e.g. "This device". */
import SettingsBadge from './SettingsBadge.vue'

defineProps<{ mark?: string }>()
</script>

<template>
  <div class="mt-5 mb-2 flex items-center gap-2.5 text-sm text-muted-foreground first:mt-0">
    <slot />
    <SettingsBadge v-if="mark">{{ mark }}</SettingsBadge>
  </div>
</template>
