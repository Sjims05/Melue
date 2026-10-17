<script setup lang="ts">
/**
 * A song in a list (Melue's design, 2026-10-17): cover, title and artist, album, length, and its buttons: play, the
 * heart, ⋯ (its menu, also on right-click) and, where it can be taken out (the queue, a playlist), ✕. The queue adds
 * a drag handle and the number.
 *
 * Only on this device (not on the share): the "This device" badge after its title.
 * Unavailable (its file missing): greyed, the note icon in place of its cover, it can't play.
 *
 * Highlighted (light: white like a card, grey in a list on a card; dark: a raised grey): the song playing (with the bars before its title),
 * the one under the pointer, and the one whose menu is open. Play and the heart show on a highlighted row; a liked
 * song keeps its filled heart.
 */
import { Ellipsis, GripVertical, Heart, Music, Pause, Play, X } from '@lucide/vue'
import { computed, ref } from 'vue'
import SettingsBadge from '@/components/settings/SettingsBadge.vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import PlayingBars from './PlayingBars.vue'
import { songMenu, type Song, type SongAction, type SongMenuPlace } from './songMenu'

// The menu wraps the row, so what's set on the row (a class) goes on the row itself.
defineOptions({ inheritAttrs: false })

const props = defineProps<{
  song: Song
  /** Its place in the list (the queue). */
  number?: number
  /** A drag handle (the queue: reorder). */
  handle?: boolean
  /** ✕: take it out of the list (the queue, a playlist). */
  removable?: boolean
  /** The song playing (or paused on). */
  current?: boolean
  /** With `current`: playing, not paused. */
  playing?: boolean
  /** A short grey note after the title ("Plays next"). */
  note?: string
  /** The file isn't there (missing, its folder off): greyed, can't play. */
  unavailable?: boolean
  /** Where it is, for the menu's extras. */
  place?: SongMenuPlace
  /** Only on this device (not on the share): the "This device" badge after the title (Simon picked it, 2026-10-17). */
  localOnly?: boolean
}>()
const emit = defineEmits<{ play: []; pause: []; like: [liked: boolean]; remove: []; action: [action: SongAction] }>()

const menuOpen = ref(false)
const contextOpen = ref(false)
const items = computed(() => songMenu(props.song, props.place))
const highlighted = computed(() => props.current || menuOpen.value || contextOpen.value)

const length = computed(() => {
  const s = Math.round(props.song.duration)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
})

function pick(action: SongAction) {
  if (action === 'like') emit('like', !props.song.liked)
  else emit('action', action)
}
function playOrPause() {
  if (props.current && props.playing) emit('pause')
  else emit('play')
}

// Text that dims on a plain row and turns light on a highlighted one (light mode's dark highlight).
const MUTED = 'text-muted-foreground group-hover/song:text-song-highlight-muted group-data-[highlight]/song:text-song-highlight-muted'
// The row's small buttons: a square in the page's colour under the pointer, or while its menu is open.
const BUTTON =
  'grid size-8 flex-none place-items-center rounded-md outline-none transition-colors hover:bg-background hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=open]:bg-background data-[state=open]:text-foreground [&_svg]:size-4'
// Shown on a highlighted row only.
const ON_HIGHLIGHT = 'invisible group-hover/song:visible group-data-[highlight]/song:visible'
</script>

<template>
  <ContextMenu :modal="false" @update:open="(open) => (contextOpen = open)">
    <ContextMenuTrigger as-child :disabled="unavailable">
      <div
        v-bind="$attrs"
        class="group/song flex h-[68px] items-center gap-4 rounded-lg px-4 text-left select-none"
        :class="
          unavailable
            ? 'opacity-45'
            : 'hover:bg-song-highlight hover:text-song-highlight-foreground data-[highlight]:bg-song-highlight data-[highlight]:text-song-highlight-foreground'
        "
        :data-highlight="highlighted && !unavailable ? '' : undefined"
        :aria-current="current || undefined"
        @dblclick="!unavailable && emit('play')"
      >
        <span v-if="handle" class="song-handle -mx-1 grid size-6 flex-none cursor-grab touch-none place-items-center active:cursor-grabbing" aria-label="Drag to move">
          <GripVertical class="size-4" />
        </span>
        <span v-if="number !== undefined" class="w-6 flex-none text-center text-[13px] tabular-nums" :class="MUTED">
          <Transition name="swap" mode="out-in"><span :key="number" class="inline-block">{{ number }}</span></Transition>
        </span>
        <span class="grid size-12 flex-none place-items-center overflow-hidden rounded-md bg-well text-muted-foreground">
          <!-- A missing song shows the note icon, not its cover: it stands out as one that can't play. -->
          <img v-if="song.cover && !unavailable" :src="song.cover" alt="" class="size-full object-cover" draggable="false" />
          <Music v-else class="size-5" />
        </span>
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
          <span class="flex min-w-0 items-center gap-2">
            <PlayingBars v-if="current" :paused="!playing" class="flex-none" />
            <span class="truncate text-[15px] font-medium">{{ song.title }}</span>
            <SettingsBadge v-if="localOnly" class="flex-none" title="Only on this device, not on your share">This device</SettingsBadge>
            <Transition name="drop">
              <span v-if="note" class="flex-none text-xs" :class="MUTED">{{ note }}</span>
            </Transition>
          </span>
          <span class="truncate text-[13px]" :class="MUTED">{{ song.artist }}</span>
        </span>
        <span class="hidden w-[28%] min-w-0 truncate text-right text-sm @2xl:block" :class="MUTED">{{ song.album }}</span>
        <span class="hidden w-14 flex-none text-right text-sm tabular-nums @md:block" :class="MUTED">{{ length }}</span>
        <span class="ml-2 flex flex-none items-center gap-1">
          <!-- A missing song keeps the button's space (invisible), so its album and length line up with the others. -->
          <button type="button" :class="[BUTTON, unavailable ? 'invisible' : ON_HIGHLIGHT]" :disabled="unavailable" :aria-label="current && playing ? 'Pause' : 'Play'" @click="playOrPause">
            <!-- Smaller than the other icons, as drawn: filled shapes look bigger than outlines. -->
            <Pause v-if="current && playing" class="size-3! fill-current" />
            <Play v-else class="size-3! fill-current" />
          </button>
          <button
            type="button"
            :class="[BUTTON, song.liked ? '' : ON_HIGHLIGHT]"
            :aria-label="song.liked ? 'Remove from liked' : 'Add to liked'"
            :aria-pressed="!!song.liked"
            @click="emit('like', !song.liked)"
          >
            <Heart :class="song.liked && 'fill-current'" />
          </button>
          <DropdownMenu @update:open="(open) => (menuOpen = open)">
            <DropdownMenuTrigger :class="BUTTON" :aria-label="`${song.title}: more`"><Ellipsis /></DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="min-w-48">
              <template v-for="item in items" :key="item.action">
                <DropdownMenuSeparator v-if="item.apart" />
                <DropdownMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="pick(item.action)">
                  <component :is="item.icon" :class="[item.filled && 'fill-current', item.small && 'm-0.5 size-3']" />{{ item.label }}
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenu>
          <button v-if="removable" type="button" :class="BUTTON" aria-label="Take out" @click="emit('remove')"><X /></button>
        </span>
      </div>
    </ContextMenuTrigger>
    <ContextMenuContent class="min-w-48">
      <template v-for="item in items" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="pick(item.action)">
          <component :is="item.icon" :class="[item.filled && 'fill-current', item.small && 'm-0.5 size-3']" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
