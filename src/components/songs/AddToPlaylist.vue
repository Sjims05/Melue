<script setup lang="ts">
/**
 * A song menu's "Add to playlist", a submenu (in a ⋯ menu or a right-click menu: `in`): New playlist…, then your
 * playlists, a ✓ on the ones that have the song (picking one adds it again). The list is read as it opens. Nothing
 * without the app's picker (PLAYLIST_PICKER).
 */
import { Check, ListMusic, Plus } from '@lucide/vue'
import { computed, inject, ref } from 'vue'
import {
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from '@/components/ui/context-menu'
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from '@/components/ui/dropdown-menu'
import { PLAYLIST_PICKER, type PlaylistChoice } from './playlistPicker'
import type { Song } from './songMenu'

const props = defineProps<{ song: Song; in: 'dropdown' | 'context' }>()
const picker = inject(PLAYLIST_PICKER, null)
const parts = computed(() =>
  props.in === 'dropdown'
    ? { Sub: DropdownMenuSub, Trigger: DropdownMenuSubTrigger, Content: DropdownMenuSubContent, Item: DropdownMenuItem, Separator: DropdownMenuSeparator }
    : { Sub: ContextMenuSub, Trigger: ContextMenuSubTrigger, Content: ContextMenuSubContent, Item: ContextMenuItem, Separator: ContextMenuSeparator },
)

const choices = ref<PlaylistChoice[] | null>(null)
async function opened(open: boolean) {
  if (open && picker) choices.value = await picker.list(props.song).catch(() => [])
}
</script>

<template>
  <component :is="parts.Sub" v-if="picker" @update:open="opened">
    <component :is="parts.Trigger" class="gap-2"><ListMusic />Add to playlist</component>
    <component :is="parts.Content" class="max-h-80 min-w-52 overflow-y-auto">
      <component :is="parts.Item" @select="picker.create(song)"><Plus />New playlist…</component>
      <template v-if="choices?.length">
        <component :is="parts.Separator" />
        <component :is="parts.Item" v-for="c in choices" :key="c.id" @select="picker.add(song, c.id)">
          <span class="min-w-0 flex-1 truncate">{{ c.name }}</span>
          <Check v-if="c.has" class="ml-auto" aria-label="Has this song" />
        </component>
      </template>
    </component>
  </component>
</template>
