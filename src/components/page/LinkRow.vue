<script setup lang="ts">
/**
 * A row that opens a page (Library's artists and folders; Melue's design): an optional icon, a name (badges after it,
 * the `badges` slot), a grey line under it, and › at the right. Under the pointer, focused or with its menu open it
 * gets the songs' highlight. `menu`: its right-click menu (Play, Shuffle, Play next, Add to queue).
 */
import { ChevronRight, User } from '@lucide/vue'
import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import type { MenuItem } from '@/components/covers'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { usePhone } from '@/lib/phone'
import { useLongPress } from '@/lib/longPress'
import { openMenu } from '@/lib/menuDrawer'

// Its attributes (a role) go on the row itself, not the menu around it.
defineOptions({ inheritAttrs: false })
const props = defineProps<{ to: RouteLocationRaw; title: string; subtitle?: string; icon?: Component; menu?: MenuItem[] }>()
const emit = defineEmits<{ action: [action: string] }>()
// A phone: a long press opens its menu as a drawer (instead of the right-click menu).
const phone = usePhone()
const press = useLongPress(() => {
  // An artist (no picture): a person.
  if (props.menu?.length) openMenu({ title: props.title, subtitle: props.subtitle, cover: { placeholder: props.icon ?? User }, items: props.menu, pick: (a) => emit('action', a) })
})
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!menu?.length || phone">
      <RouterLink
        v-bind="{ ...$attrs, ...(phone ? press : {}) }"
        :to="to"
        class="flex h-[68px] items-center gap-4 rounded-lg px-4 outline-none hover:bg-song-highlight focus-visible:bg-song-highlight data-[state=open]:bg-song-highlight"
      >
        <component :is="icon" v-if="icon" class="size-5 flex-none text-muted-foreground" />
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="flex min-w-0 items-center gap-2">
            <span class="truncate text-[15px] font-medium">{{ title }}</span>
            <slot name="badges" />
          </span>
          <span v-if="subtitle" class="truncate text-sm text-muted-foreground">{{ subtitle }}</span>
        </span>
        <ChevronRight class="size-5 flex-none text-muted-foreground" />
      </RouterLink>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="menu?.length" class="min-w-52">
      <template v-for="item in menu" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem @select="emit('action', item.action)"><component :is="item.icon" />{{ item.label }}</ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
