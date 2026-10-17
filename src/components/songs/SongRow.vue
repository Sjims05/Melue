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
import { Ellipsis, GripVertical, Heart, Music, X } from '@lucide/vue'
import { popHeart } from '@/components/player/heart'
import { PauseIcon, PlayIcon } from '@/components/player/icons'
import { computed, ref } from 'vue'
import SettingsBadge from '@/components/settings/SettingsBadge.vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { artistParts } from './artistParts'
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
  /** Picked up (the queue: being dragged, or gliding into its place): highlighted. */
  lifted?: boolean
  /** Just put down: its highlight fades away (instead of going at once). */
  fading?: boolean
  /** With `current`: playing, not paused. */
  playing?: boolean
  /** A short grey note after the title ("Plays next"). */
  note?: string
  /** The file isn't there (missing, its folder off): greyed, can't play. */
  unavailable?: boolean
  /** Where it is, for the menu's extras. */
  place?: SongMenuPlace
  /** Only on this device (not on the share): the "This device" badge after the title. */
  localOnly?: boolean
}>()
const emit = defineEmits<{ play: []; pause: []; like: [liked: boolean]; remove: []; action: [action: SongAction] }>()

const menuOpen = ref(false)
const contextOpen = ref(false)
const items = computed(() => songMenu(props.song, props.place))
const highlighted = computed(() => props.current || props.lifted || menuOpen.value || contextOpen.value)
// A greyed song (unavailable, or shown ahead) isn't highlighted, by the pointer or otherwise.
const greyed = computed(() => props.unavailable || !!props.song.later)

const length = computed(() => {
  if (!props.song.duration) return ''
  const s = Math.round(props.song.duration)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
})

function pick(action: SongAction) {
  if (action === 'like') emit('like', !props.song.liked)
  else emit('action', action)
}
// Double-click on the row plays it, but not on something with its own action (a button, the album link, the handle).
function playOnDoubleClick(e: MouseEvent) {
  if (props.unavailable || props.song.later) return
  if ((e.target as HTMLElement).closest('button, a, input, [role=button], [role=menuitem], .song-handle')) return
  emit('play')
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
        class="flex h-[68px] items-center gap-4 rounded-lg px-4 text-left select-none"
        :class="
          unavailable
            ? 'opacity-45'
            : song.later
              ? 'opacity-50'
              : [
                  fading && 'transition-colors duration-300 [&_*]:transition-colors [&_*]:duration-300',
                  'group/song hover:bg-song-highlight hover:text-song-highlight-foreground data-[highlight]:bg-song-highlight data-[highlight]:text-song-highlight-foreground',
                ]
        "
        :data-highlight="highlighted && !greyed ? '' : undefined"
        :aria-current="current || undefined"
        @dblclick="playOnDoubleClick"
      >
        <!-- A song shown ahead (later) keeps the handle's space, but can't be moved. -->
        <span v-if="handle && song.later" class="invisible -mx-1 size-6 flex-none" />
        <span v-else-if="handle" class="song-handle -mx-1 grid size-6 flex-none cursor-grab touch-none place-items-center active:cursor-grabbing" aria-label="Drag to move">
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
          <!-- Each artist's name links to their page, when the song knows them (underlined under the pointer). -->
          <span class="truncate text-[13px]" :class="MUTED"><template v-for="(part, i) in artistParts(song.artist, song.artists)" :key="i"><RouterLink v-if="part.artist && !greyed" :to="`/artist/${part.artist}`" class="hover:underline" @dblclick.stop>{{ part.text }}</RouterLink><template v-else>{{ part.text }}</template></template></span>
        </span>
        <!-- The album opens its page (like Go to album in the menu). -->
        <span class="hidden w-[28%] min-w-0 truncate text-right text-sm @2xl:block" :class="MUTED">
          <button
            v-if="song.album && !song.later"
            type="button"
            class="max-w-full truncate underline-offset-2 hover:text-current hover:underline"
            @click.stop="emit('action', 'goToAlbum')"
          >
            {{ song.album }}
          </button>
          <template v-else>{{ song.album }}</template>
        </span>
        <span class="hidden w-14 flex-none text-right text-sm tabular-nums @md:block" :class="MUTED">{{ length }}</span>
        <span class="ml-2 flex flex-none items-center gap-1">
          <!-- A missing song keeps the button's space (invisible), so its album and length line up with the others. -->
          <button type="button" :class="[BUTTON, unavailable ? 'invisible' : ON_HIGHLIGHT]" :disabled="unavailable" :aria-label="current && playing ? 'Pause' : 'Play'" @click="playOrPause">
            <!-- Smaller than the other icons, as drawn: filled shapes look bigger than outlines. -->
            <PauseIcon v-if="current && playing" class="size-4!" />
            <PlayIcon v-else class="size-4!" />
          </button>
          <button
            type="button"
            :class="[BUTTON, song.liked ? '' : ON_HIGHLIGHT]"
            :aria-label="song.liked ? 'Remove from liked' : 'Add to liked'"
            :aria-pressed="!!song.liked"
            @click="(e: MouseEvent) => (popHeart(e.currentTarget, !song.liked), emit('like', !song.liked))"
          >
            <Heart class="heart" :class="song.liked && 'fill-current'" />
          </button>
          <DropdownMenu @update:open="(open) => (menuOpen = open)">
            <DropdownMenuTrigger :class="BUTTON" :aria-label="`${song.title}: more`"><Ellipsis /></DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="min-w-48">
              <template v-for="item in items" :key="item.action">
                <DropdownMenuSeparator v-if="item.apart" />
                <DropdownMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="pick(item.action)">
                  <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenu>
          <button v-if="removable" type="button" :class="[BUTTON, song.later && 'invisible']" :disabled="!!song.later" aria-label="Take out" @click="emit('remove')">
            <X />
          </button>
        </span>
      </div>
    </ContextMenuTrigger>
    <ContextMenuContent class="min-w-48">
      <template v-for="item in items" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="pick(item.action)">
          <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
