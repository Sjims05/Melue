<script setup lang="ts">
/** A section's heading ("Syncing") with its description under it; `mark` adds a badge (e.g. "Phones"). */
import SettingsBadge from './SettingsBadge.vue'

defineProps<{ title: string; description?: string; mark?: string }>()
</script>

<template>
  <div class="mt-7 mb-2.5 first:mt-0">
    <h2 class="flex items-center gap-3 text-[22px] font-bold tracking-tight">
      {{ title }}
      <SettingsBadge v-if="mark">{{ mark }}</SettingsBadge>
    </h2>
    <p v-if="description" class="mt-1 text-sm text-muted-foreground">{{ description }}</p>
  </div>
</template>
