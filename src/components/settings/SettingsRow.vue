<script setup lang="ts">
/**
 * A setting's row in a card: its title and a grey line under it, a control on the right (the default slot), and an
 * optional trailing icon (the `end` slot: a chevron, ⋯ or radio) in the card's icon column.
 *
 * `detail`: a detail row under a setting (indented, quieter title). `wide`: its control is wide (segments, chips),
 * so it moves under the text when there's no room. `dim`: greyed (e.g. a device that isn't connected).
 *
 * `reset`: right-click (long-press on a phone) offers "Reset to default"; `isDefault` greys it out when it already is.
 */
import { RotateCcw } from '@lucide/vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'

// The menu wraps the row, so what's set on the row (a click, aria-expanded, a class) goes on the row itself.
defineOptions({ inheritAttrs: false })

withDefaults(
  defineProps<{
    title?: string
    description?: string
    detail?: boolean
    wide?: boolean
    dim?: boolean
    as?: 'div' | 'button' | 'label'
    reset?: () => void
    isDefault?: boolean
  }>(),
  { as: 'div' },
)
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!reset">
      <component
        :is="as"
        v-bind="$attrs"
        class="settings-row relative flex w-full items-center gap-x-3.5 gap-y-2.5 text-left"
        :class="{ wide, detail, 'has-end': !!$slots.end, 'cursor-pointer': as !== 'div' }"
        :type="as === 'button' ? 'button' : undefined"
        data-row
      >
        <slot name="lead" />
        <span class="settings-row-text flex flex-col gap-0.5">
          <span
            class="text-sm [overflow-wrap:anywhere]"
            :class="[detail ? 'font-medium text-soft' : 'font-semibold', dim && 'text-muted-foreground']"
          >
            <slot name="title">{{ title }}</slot>
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
    <ContextMenuContent v-if="reset">
      <ContextMenuItem :disabled="isDefault" @select="reset()"><RotateCcw />Reset to default</ContextMenuItem>
    </ContextMenuContent>
  </ContextMenu>
</template>
