<script setup lang="ts" generic="T extends string">
/**
 * A list's sort menu (the Library tabs): a button with the order shown ("Title"), opening the choices (on a phone in
 * the menu drawer).
 */
import { ArrowDownUp } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { openMenu } from '@/lib/menuDrawer'
import { usePhone } from '@/lib/phone'

const props = defineProps<{ options: readonly (readonly [T, string])[] }>()
const model = defineModel<T>({ required: true })
const label = (value: T) => props.options.find(([v]) => v === value)?.[1] ?? value

const phone = usePhone()
function openDrawer() {
  openMenu({
    title: 'Sort by',
    items: props.options.map(([value, text]) => ({ action: value, label: text })),
    picked: model.value,
    pick: (value) => (model.value = value as T),
  })
}
</script>

<template>
  <Button v-if="phone" variant="outline" size="sm" class="size-8 px-0" :aria-label="`Sort by ${label(model)}`" @click="openDrawer">
    <ArrowDownUp />
  </Button>
  <DropdownMenu v-else>
    <DropdownMenuTrigger as-child>
      <Button variant="outline" size="sm" :aria-label="`Sort by ${label(model)}`">
        <ArrowDownUp />{{ label(model) }}
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="min-w-44">
      <DropdownMenuRadioGroup v-model="model">
        <DropdownMenuRadioItem v-for="[value, text] in options" :key="value" :value="value">{{ text }}</DropdownMenuRadioItem>
      </DropdownMenuRadioGroup>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
