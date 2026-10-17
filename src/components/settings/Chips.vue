<script setup lang="ts" generic="T extends string | number">
/**
 * One of a few values as pills (Sync when coming back: Never / 5 min / 15 min / 1 hour). Always one is picked; the
 * colour fades to it.
 */
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'

defineProps<{ options: readonly (readonly [T, string])[] }>()
const model = defineModel<T>({ required: true })

const pick = (value: unknown) => {
  if (value !== undefined && value !== '') model.value = value as T
}
</script>

<template>
  <ToggleGroupRoot type="single" :model-value="model" class="inline-flex flex-wrap gap-1.5" @update:model-value="pick">
    <ToggleGroupItem
      v-for="[value, label] in options"
      :key="value"
      :value="value"
      class="h-8 rounded-full border border-border bg-card px-3 text-[13px] font-medium whitespace-nowrap transition-colors duration-150 outline-none motion-reduce:transition-none focus-visible:ring-2 focus-visible:ring-ring/50 dark:border-secondary dark:bg-secondary data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground dark:data-[state=on]:border-primary dark:data-[state=on]:bg-primary"
    >
      {{ label }}
    </ToggleGroupItem>
  </ToggleGroupRoot>
</template>
