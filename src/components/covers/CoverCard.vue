<script setup lang="ts">
/**
 * A playlist, album, auto playlist or song as a card (Home, Playlists, an artist's albums, search): its cover, its
 * name, and a grey line ("8 songs · 31 min"). The cover's props go to Cover; the `badge` slot holds a CoverBadge.
 * The whole card is a button (it opens what it is); `menu` is its right-click menu (long-press on a phone: coverMenu,
 * or a song's songMenu), and the card shows its highlight while the menu is open. Its spacing and how it shows it's under the pointer: covers.css
 * (the highlight behind the whole card). Put cards in a `.cover-grid` (`--cover-min`: the narrowest a card may get).
 */
import type { Component } from 'vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import Cover from './Cover.vue'
import type { MenuItem } from './coverMenu'
import type { CoverColors } from './coverRecipe'

defineProps<{
  title: string
  subtitle?: string
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
  menu?: MenuItem[]
}>()
const emit = defineEmits<{ open: []; action: [action: string] }>()
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!menu?.length">
      <button type="button" class="group/cover cover-card min-w-0 text-left outline-none" @click="emit('open')">
        <Cover
          :image="image"
          :colors="colors"
          :icon="icon"
          :collage="collage"
          class="group-focus-visible/cover:ring-2 group-focus-visible/cover:ring-ring/50"
        >
          <slot name="badge" />
        </Cover>
        <span class="cover-card-text min-w-0">
          <span class="truncate text-[15px] font-medium">{{ title }}</span>
          <span v-if="subtitle" class="truncate text-sm text-muted-foreground">{{ subtitle }}</span>
        </span>
      </button>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="menu?.length" class="min-w-52">
      <template v-for="item in menu" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
          <component :is="item.icon" :class="[item.filled && 'fill-current', item.small && 'm-0.5 size-3']" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
