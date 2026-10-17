<script setup lang="ts">
/** An icon that explains itself on hover or focus (a copy: "this song is also in your synced music…"). */
import type { Component } from 'vue'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

defineProps<{ icon: Component; text: string }>()
</script>

<template>
  <Tooltip>
    <TooltipTrigger as-child>
      <span tabindex="0" class="inline-grid place-items-center rounded text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50" :aria-label="text">
        <component :is="icon" class="size-4" />
      </span>
    </TooltipTrigger>
    <TooltipContent class="max-w-56 text-center">{{ text }}</TooltipContent>
  </Tooltip>
</template>
