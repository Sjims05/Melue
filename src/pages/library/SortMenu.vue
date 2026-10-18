<script setup lang="ts" generic="T extends string">
/** A list's sort menu (the Library tabs): a button with the order shown ("Title"), opening the choices. */
import { ArrowDownUp } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'

const props = defineProps<{ options: readonly (readonly [T, string])[] }>()
const model = defineModel<T>({ required: true })
const label = (value: T) => props.options.find(([v]) => v === value)?.[1] ?? value
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="outline" size="sm" :aria-label="`Sort by ${label(model)}`"><ArrowDownUp />{{ label(model) }}</Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="min-w-44">
      <DropdownMenuRadioGroup v-model="model">
        <DropdownMenuRadioItem v-for="[value, text] in options" :key="value" :value="value">{{ text }}</DropdownMenuRadioItem>
      </DropdownMenuRadioGroup>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
