<script setup lang="ts">
/**
 * A playlist, album, auto playlist or song as a card (Home, Playlists, an artist's albums, search): its cover, its
 * name, and a grey line ("8 songs · 31 min"). The cover's props go to Cover; the `badge` slot holds a CoverBadge.
 * The whole card is a button (it opens what it is); `menu` is its right-click menu (long-press on a phone: coverMenu,
 * or a song's songMenu), and the card shows its highlight while the menu is open. `editing`: its name is a text field
 * with the name in it (Rename): Enter or leaving it saves (`rename`), Escape doesn't (`cancel`). Its spacing and how it
 * shows it's under the pointer: covers.css (the highlight behind the whole card). Put cards in a `.cover-grid`
 * (`--cover-min`: the narrowest a card may get).
 *
 * A song's card (`song`: whether it's the one playing, and playing) works like a song row: a play button on its cover
 * (shown under the pointer, or while it's the song playing) plays it (`play`), a double-click adds it to the queue
 * (`queue`); a click alone does nothing. It's not one button then (the play button is in it): Enter plays it.
 */
import { nextTick, ref, watch, type Component } from 'vue'
import { PauseIcon, PlayIcon } from '@/components/player/icons'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import Cover from './Cover.vue'
import type { MenuItem } from './coverMenu'
import type { CoverColors } from './coverRecipe'

const props = defineProps<{
  title: string
  subtitle?: string
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
  /** Shown when there's no picture: the note, or something that fits better (an artist: a person). */
  placeholder?: Component
  menu?: MenuItem[]
  editing?: boolean
  song?: { current: boolean; playing: boolean }
}>()
const emit = defineEmits<{ open: []; play: []; queue: []; action: [action: string]; rename: [name: string]; cancel: [] }>()

// Renaming: the field starts with the name, all of it selected.
const field = ref<HTMLInputElement>()
const name = ref(props.title)
let done = false
watch(
  () => props.editing,
  async (editing) => {
    if (!editing) return
    name.value = props.title
    done = false
    await nextTick()
    field.value?.focus()
    field.value?.select()
  },
  { immediate: true },
)
// A trial: a click alone on a song's card (it doesn't play: rows don't either) gives its play button a little bounce,
// showing where playing is. To take it out: this, `playButton`'s ref, and `hintPlay` in the card's @click.
const playButton = ref<HTMLElement>()
function hintPlay() {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
  playButton.value?.animate([{ scale: 1 }, { scale: 1.18, offset: 0.4 }, { scale: 1 }], { duration: 380, easing: 'cubic-bezier(.2,.8,.2,1)' })
}

function finish(save: boolean) {
  if (done) return
  done = true
  if (save) emit('rename', name.value.trim() || props.title)
  else emit('cancel')
}
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!menu?.length || editing">
      <component
        :is="editing || song ? 'div' : 'button'"
        :type="editing || song ? undefined : 'button'"
        :tabindex="song ? 0 : undefined"
        :role="song ? 'group' : undefined"
        :aria-label="song ? title : undefined"
        class="group/cover cover-card min-w-0 text-left outline-none"
        @click="!editing && (song ? hintPlay() : emit('open'))"
        @dblclick="song && emit('queue')"
        @keydown.enter.self="song && emit('play')"
      >
        <Cover
          :image="image"
          :colors="colors"
          :icon="icon"
          :collage="collage"
          :placeholder="placeholder"
          class="group-focus-visible/cover:ring-2 group-focus-visible/cover:ring-ring/50"
        >
          <slot name="badge" />
          <button
            v-if="song"
            ref="playButton"
            type="button"
            class="absolute right-2.5 bottom-2.5 grid size-11 place-items-center rounded-full bg-primary text-primary-foreground shadow-md transition-[opacity,translate,scale] duration-200 ease-out outline-none hover:scale-105 active:scale-95 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring/50 [&_svg]:size-5"
            :class="song.current ? 'opacity-100' : 'translate-y-1 opacity-0 group-hover/cover:translate-y-0 group-hover/cover:opacity-100'"
            :aria-label="song.current && song.playing ? 'Pause' : 'Play'"
            @click.stop="emit('play')"
            @dblclick.stop
          >
            <PauseIcon v-if="song.current && song.playing" />
            <PlayIcon v-else class="translate-x-px" />
          </button>
        </Cover>
        <span class="cover-card-text min-w-0">
          <input
            v-if="editing"
            ref="field"
            v-model="name"
            aria-label="Name"
            class="-mx-1.5 -my-0.5 rounded-md bg-background px-1.5 py-0.5 text-[15px] font-medium outline-none ring-2 ring-ring/50"
            @keydown.enter.prevent="finish(true)"
            @keydown.escape.stop.prevent="finish(false)"
            @blur="finish(true)"
          />
          <span v-else class="truncate text-[15px] font-medium phone:text-sm">{{ title }}</span>
          <span v-if="subtitle" class="truncate text-sm text-muted-foreground phone:text-[13px]">{{ subtitle }}</span>
        </span>
      </component>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="menu?.length" class="min-w-52">
      <template v-for="item in menu" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
          <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
