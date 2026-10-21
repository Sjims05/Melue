<script setup lang="ts">
/**
 * A song's menu on a part of a page that shows it (Now playing's cover and title): a right-click menu on a computer,
 * a long press for the drawer on a phone. Its one element (the slot) is what's pressed.
 */
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { useLongPress } from '@/lib/longPress'
import { openMenu } from '@/lib/menuDrawer'
import { usePhone } from '@/lib/phone'
import AddToPlaylist from './AddToPlaylist.vue'
import type { Song, SongAction, SongMenuItem } from './songMenu'

const props = defineProps<{ song: Song; items: SongMenuItem[] }>()
const emit = defineEmits<{ action: [action: SongAction] }>()

const phone = usePhone()
const press = useLongPress(() =>
  openMenu({
    title: props.song.title,
    subtitle: props.song.artist,
    cover: { image: props.song.cover },
    items: props.items,
    pick: (a) => emit('action', a as SongAction),
    song: props.song,
  }),
)
</script>

<template>
  <!-- The long press around the right-click menu (which takes the element's own touch handling). -->
  <div class="contents" v-bind="phone ? press : {}">
    <ContextMenu :modal="false">
      <ContextMenuTrigger as-child :disabled="phone"><slot /></ContextMenuTrigger>
      <ContextMenuContent class="min-w-48">
        <template v-for="item in items" :key="item.action">
          <ContextMenuSeparator v-if="item.apart" />
          <AddToPlaylist v-if="item.action === 'addToPlaylist'" :song="song" in="context" />
          <ContextMenuItem v-else :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
            <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
          </ContextMenuItem>
        </template>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>
