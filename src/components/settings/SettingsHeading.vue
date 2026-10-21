<script setup lang="ts">
/**
 * A section's heading ("Syncing") with its description under it; `mark` adds a badge (e.g. "Phones"). Its right-click
 * menu (a long press on a phone) resets the changed settings under it, up to the next heading, after asking.
 */
import { RotateCcw } from '@lucide/vue'
import { inject, ref } from 'vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import { ask } from '@/lib/confirm'
import { useLongPress } from '@/lib/longPress'
import { openMenu } from '@/lib/menuDrawer'
import { usePhone } from '@/lib/phone'
import SettingsBadge from './SettingsBadge.vue'
import { changedUnder } from './rowResets'
import { SETTING_MENU } from './settingMenu'

const props = defineProps<{ title: string; description?: string; mark?: string }>()

const menu = inject(SETTING_MENU, null)
const heading = ref<HTMLElement>()
// Counted as the menu opens (what's changed now).
const changed = ref(0)
const look = () => (changed.value = heading.value ? changedUnder(heading.value).length : 0)

async function resetAll() {
  const rows = heading.value ? changedUnder(heading.value) : []
  if (!rows.length) return
  const yes = await ask({
    title: `Reset ${props.title} to default?`,
    text: rows.length === 1 ? 'The setting you changed here goes back to its default.' : `The ${rows.length} settings you changed here go back to their defaults.`,
    action: 'Reset',
  })
  if (!yes) return
  if (menu) await menu.reset(rows.map((row) => ({ setting: row.setting(), reset: row.reset })))
  else rows.forEach((row) => row.reset())
}

const phone = usePhone()
const press = useLongPress(() => {
  if (!look()) return
  openMenu({ title: props.title, items: [{ action: 'reset', label: 'Reset these to default', icon: RotateCcw }], pick: resetAll })
})
</script>

<template>
  <ContextMenu :modal="false" @update:open="(open: boolean) => open && look()">
    <ContextMenuTrigger as-child :disabled="phone">
      <div ref="heading" v-bind="phone ? press : {}" class="mt-7 mb-2.5 first:mt-0" data-settings-heading>
        <h2 class="flex items-center gap-3 text-[22px] font-bold tracking-tight">
          {{ title }}
          <SettingsBadge v-if="mark">{{ mark }}</SettingsBadge>
        </h2>
        <p v-if="description" class="mt-1 text-sm text-muted-foreground">{{ description }}</p>
      </div>
    </ContextMenuTrigger>
    <ContextMenuContent class="min-w-56">
      <ContextMenuItem :disabled="!changed" @select="resetAll">
        <RotateCcw />Reset these to default
        <span v-if="changed" class="ml-auto pl-4 text-muted-foreground">{{ changed }}</span>
      </ContextMenuItem>
    </ContextMenuContent>
  </ContextMenu>
</template>
