<script setup lang="ts">
/**
 * A setting's row in a card: its title and a grey line under it, a control on the right (the default slot), and an
 * optional trailing icon (the `end` slot: a chevron, ⋯ or radio) in the card's icon column.
 *
 * `detail`: a detail row under a setting (indented, quieter title). `wide`: its control is wide (segments, chips),
 * so it moves under the text when there's no room. `dim`: greyed (e.g. a device that isn't connected).
 *
 * Right-click (long-press on a phone): `reset` offers "Reset to default" (`isDefault` greys it out when it already
 * is). `setting` names the setting ("sync.atStart"); where the page provides the menu's extras (settingMenu.ts), it
 * adds the default in words, About this setting, Copy link, and This device only (then the title says so).
 */
import { Info, Link, RotateCcw } from '@lucide/vue'
import { computed, inject, ref } from 'vue'
import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import ConfirmDialog from './ConfirmDialog.vue'
import SettingsBadge from './SettingsBadge.vue'
import { SETTING_MENU } from './settingMenu'

// The menu wraps the row, so what's set on the row (a click, aria-expanded, a class) goes on the row itself.
defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    title?: string
    description?: string
    detail?: boolean
    wide?: boolean
    dim?: boolean
    as?: 'div' | 'button' | 'label'
    reset?: () => void
    isDefault?: boolean
    setting?: string
  }>(),
  { as: 'div' },
)

const menu = inject(SETTING_MENU, null)
const info = computed(() => (menu && props.setting ? menu.info(props.setting) : undefined))
const deviceOnly = computed(() => !!(menu && props.setting && menu.isDeviceOnly(props.setting)))
const about = ref(false)
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!reset && !info">
      <component
        :is="as"
        v-bind="$attrs"
        class="settings-row relative flex w-full items-center gap-x-3.5 gap-y-2.5 text-left"
        :class="{ wide, detail, 'has-end': !!$slots.end, 'cursor-pointer': as !== 'div' }"
        :type="as === 'button' ? 'button' : undefined"
        :data-setting="setting"
        data-row
      >
        <slot name="lead" />
        <span class="settings-row-text flex flex-col gap-0.5">
          <span
            class="text-sm [overflow-wrap:anywhere]"
            :class="[detail ? 'font-medium text-soft' : 'font-semibold', dim && 'text-muted-foreground']"
          >
            <slot name="title">{{ title }}</slot>
            <SettingsBadge v-if="deviceOnly" class="ml-1.5">This device</SettingsBadge>
            <Transition name="drop">
              <SettingsBadge v-if="menu && setting && menu.copied === setting" class="ml-1.5">Link copied</SettingsBadge>
            </Transition>
          </span>
          <span v-if="description || $slots.description" class="text-[13.5px] leading-snug text-muted-foreground">
            <slot name="description">{{ description }}</slot>
          </span>
        </span>
        <span v-if="$slots.default" class="ml-auto flex max-w-full flex-none flex-wrap items-center justify-end gap-2">
          <slot />
        </span>
        <span v-if="$slots.end" class="settings-row-end text-muted-foreground" data-end>
          <slot name="end" />
        </span>
      </component>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="reset || info" class="min-w-56">
      <ContextMenuItem v-if="reset" :disabled="isDefault" @select="reset()">
        <RotateCcw />Reset to default
        <span v-if="info?.default" class="ml-auto pl-4 text-muted-foreground">{{ info.default }}</span>
      </ContextMenuItem>
      <template v-if="menu && setting && info">
        <ContextMenuSeparator v-if="reset" />
        <ContextMenuItem @select="about = true"><Info />About this setting</ContextMenuItem>
        <ContextMenuItem @select="menu.copyLink(setting)"><Link />Copy link to this setting</ContextMenuItem>
        <template v-if="!info.perDevice">
          <ContextMenuSeparator />
          <ContextMenuCheckboxItem :model-value="deviceOnly" @update:model-value="(on: boolean) => menu!.setDeviceOnly(setting!, on)">
            This device only
          </ContextMenuCheckboxItem>
        </template>
      </template>
    </ContextMenuContent>
  </ContextMenu>
  <ConfirmDialog v-if="info" v-model:open="about" :title="info.title" :text="info.help" />
</template>
