<script setup lang="ts" generic="T extends string | number">
/**
 * A value with a small chevron that opens its choices (Limit for songs: 250 MB ⌄). A long list (a time of day) is
 * capped in height and opens scrolled to the current value. On a phone the choices open in the menu drawer, under the
 * row's title (like the app's other menus there).
 */
import { ChevronDown } from '@lucide/vue'
import { computed, inject } from 'vue'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { openMenu } from '@/lib/menuDrawer'
import { usePhone } from '@/lib/phone'
import { ROW_TITLE } from './settingMenu'

const props = defineProps<{ options: readonly (readonly [T, string])[] }>()
const model = defineModel<T>({ required: true })

const TRIGGER =
  'inline-flex items-center gap-1.5 rounded-md text-[13px] whitespace-nowrap text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50'
const label = computed(() => props.options.find(([value]) => value === model.value)?.[1] ?? '')
// The menu's values are strings; the model keeps the option's own value (a number stays a number).
const pick = (value: unknown) => {
  const option = props.options.find(([v]) => String(v) === String(value))
  if (option) model.value = option[0]
}

const phone = usePhone()
const rowTitle = inject(ROW_TITLE, () => undefined)
function openDrawer() {
  openMenu({
    title: rowTitle() ?? 'Choose',
    items: props.options.map(([value, text]) => ({ action: String(value), label: text })),
    picked: String(model.value),
    pick,
  })
}

// Opening: the picked value in view and focused (the keys move on from there).
function showPicked(event: Event) {
  const picked = (event.target as HTMLElement | null)?.querySelector<HTMLElement>('[role=menuitemradio][data-state=checked]')
  if (!picked) return
  event.preventDefault()
  picked.scrollIntoView({ block: 'center' })
  picked.focus({ preventScroll: true })
}
</script>

<template>
  <button v-if="phone" type="button" :class="TRIGGER" @click="openDrawer">
    {{ label }}
    <ChevronDown class="size-3.5" />
  </button>
  <DropdownMenu v-else>
    <DropdownMenuTrigger :class="TRIGGER">
      {{ label }}
      <ChevronDown class="size-3.5" />
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="max-h-72" @open-auto-focus="showPicked">
      <DropdownMenuRadioGroup :model-value="String(model)" @update:model-value="pick">
        <DropdownMenuRadioItem v-for="[value, text] in options" :key="value" :value="String(value)">{{ text }}</DropdownMenuRadioItem>
      </DropdownMenuRadioGroup>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
