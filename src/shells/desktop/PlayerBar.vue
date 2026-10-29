<script setup lang="ts">
/**
 * The player bar, at the bottom of every page but Now playing (Melue's design): the song (its cover and words open Now
 * playing) with the playing bars; then, well apart, shuffle, previous, play / pause, next, repeat (on: white with a dot
 * under it; repeat one: a "1"; the player's own icons, components/player/icons.ts); the seek bar with both times; then
 * the heart, Connect device (with a connection, its pill just above the bar: shells/Toasts.vue), the queue (Now playing with Up next open) and ✕ (stops and empties the
 * player). Always there: with nothing in the queue it's empty ("Nothing playing", the note icon for a cover, as a
 * song that can't play has), only shuffle and repeat on (they're for what's played next).
 */
import LikeHeart from '@/components/player/LikeHeart.vue'
import DevicesMenu from '@/components/player/DevicesMenu.vue'
import { connectedTo, connectionText } from '@/player/playingOn'
import { ListMusic, MonitorSpeaker, Music, Repeat, Shuffle, X } from '@lucide/vue'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import Slider from '@/components/player/Slider.vue'
import { PlayingBars } from '@/components/songs'
import VolumeButton from './VolumeButton.vue'
import { clearPlayer, current, cycleRepeat, like, next, nothingHere, nowPlaying, position, previous, seek, toggleShuffle, togglePlay } from '@/player/nowPlaying'

const router = useRouter()
const state = computed(() => nowPlaying.state)
// Nothing in the queue: the bar stays, empty, with what needs a song off.
// (Another device's song shown with nothing to play here: the same, but it can be cleared.)
const empty = computed(() => !current.value || !state.value || nothingHere.value)
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

// A button's colour is one class or the other (two colour classes: the stylesheet's order would decide, not this).
const ICON = 'relative grid size-9 place-items-center rounded-md transition-colors hover:text-foreground [&_svg]:size-4'
const tone = (on: boolean) => (on ? 'text-foreground' : 'text-muted-foreground')
const OFF = 'pointer-events-none opacity-40'
</script>

<template>
  <!--
    The parts, with the room between them as columns of their own. As the window narrows, the song gives up its
    width first (360 px down to 200), while the progress keeps at least 240; under 1140 px everything moves closer
    together. The song's widest is 360 px or the room the rest leaves it (100% less 828 px: the three 48 px gaps,
    the controls' 232, the progress's 240 and the right side's 212), not 360 with the progress's minimum pushing it
    back: while the columns change between the two, that would let the song grow past what it ends at, and the
    controls run on and back (a pop).
  -->
  <footer
    class="grid h-[92px] flex-none grid-cols-[minmax(0,200px)_16px_auto_16px_minmax(0,1fr)_16px_auto] items-center border-t px-4 transition-[grid-template-columns,padding] duration-300 ease-out min-[1140px]:grid-cols-[minmax(200px,min(360px,calc(100%_-_828px)))_48px_auto_48px_minmax(240px,1fr)_48px_auto] min-[1140px]:px-12"
  >
    <!-- The song. A set width, so the controls stay put: a long title is cut off (the room after it keeps it off them). -->
    <RouterLink v-if="current && state" to="/now-playing" class="col-start-1 flex min-w-0 items-center gap-4">
      <span class="relative grid size-[52px] flex-none place-items-center overflow-hidden rounded-md bg-well text-muted-foreground">
        <!-- No cover (a song only on the other device, or one without): the note, as the mini player shows it. -->
        <Music v-if="!current.cover" class="size-5" />
        <Transition name="fade">
          <img v-if="current.cover" :key="current.cover" :src="current.cover" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
        </Transition>
      </span>
      <Transition name="swap" mode="out-in">
        <span :key="current.id" class="flex min-w-0 flex-col">
          <span class="truncate text-[15px] font-semibold">{{ current.title }}</span>
          <span class="truncate text-[15px] text-muted-foreground">{{ current.artist }}</span>
        </span>
      </Transition>
      <PlayingBars :paused="!state.isPlaying" class="ml-1 flex-none scale-150" />
    </RouterLink>
    <div v-else class="col-start-1 flex min-w-0 items-center gap-4">
      <span class="grid size-[52px] flex-none place-items-center rounded-md bg-well text-muted-foreground"><Music class="size-5" /></span>
      <span class="flex min-w-0 flex-col">
        <span class="truncate text-[15px] font-semibold">Nothing playing</span>
        <span class="truncate text-[15px] text-muted-foreground">Play a song to start</span>
      </span>
    </div>

    <!-- The controls. -->
    <div class="col-start-3 flex items-center gap-1 transition-[gap] duration-300 ease-out min-[1140px]:gap-3">
      <button type="button" :class="[ICON, tone(!!state?.shuffle)]" :aria-pressed="!!state?.shuffle" aria-label="Shuffle" @click="toggleShuffle">
        <Shuffle />
        <span v-if="state?.shuffle" class="absolute bottom-0.5 size-1 rounded-full bg-current" />
      </button>
      <button type="button" :class="[ICON, tone(true), empty && OFF]" :disabled="empty" aria-label="Previous" @click="previous"><PreviousIcon /></button>
      <button
        type="button"
        class="grid size-10 place-items-center rounded-full bg-foreground text-background transition-transform active:scale-95"
        :class="empty && OFF"
        :disabled="empty"
        :aria-label="state?.isPlaying ? 'Pause' : 'Play'"
        @click="togglePlay"
      >
        <PauseIcon v-if="state?.isPlaying" class="size-4" />
        <PlayIcon v-else class="size-4 translate-x-px" />
      </button>
      <button type="button" :class="[ICON, tone(true), empty && OFF]" :disabled="empty" aria-label="Next" @click="next"><NextIcon /></button>
      <button
        type="button"
        :class="[ICON, tone(!!state && state.repeatMode !== 'off')]"
        :aria-label="`Repeat: ${state?.repeatMode ?? 'off'}`"
        @click="cycleRepeat"
      >
        <Repeat />
        <span v-if="state?.repeatMode === 'one'" class="absolute top-0.5 right-0.5 text-[9px] leading-none font-semibold">1</span>
        <span v-if="state && state.repeatMode !== 'off'" class="absolute bottom-0.5 size-1 rounded-full bg-current" />
      </button>
    </div>

    <!-- Where it is. -->
    <div class="col-start-5 flex min-w-0 items-center gap-3 text-sm text-muted-foreground tabular-nums transition-[gap] duration-300 ease-out min-[1140px]:gap-4" :class="empty && OFF" :inert="empty || undefined">
      <span class="w-10 text-right">{{ time(empty ? 0 : position) }}</span>
      <Slider class="flex-1" :value="empty ? 0 : position" :max="state?.duration || 1" label="Seek" @change="seek" />
      <span class="w-10">{{ time(empty ? 0 : (state?.duration ?? 0)) }}</span>
    </div>

    <!-- The rest. -->
    <div class="col-start-7 flex items-center gap-0.5 transition-[gap] duration-300 ease-out min-[1140px]:gap-2">
      <button
        type="button"
        :class="[ICON, tone(!!current?.liked), empty && OFF]"
        :disabled="empty"
        :aria-label="current?.liked ? 'Remove from liked' : 'Add to liked'"
        @click="like(Number(current!.id), !current!.liked)"
      >
        <LikeHeart :liked="!!current?.liked" />
      </button>
      <DevicesMenu>
        <button
          type="button"
          :class="[ICON, connectedTo() ? 'text-primary' : tone(false)]"
          :aria-label="connectionText() ?? 'Play on another device'"
          :title="connectionText() ?? 'Play on another device'"
        >
          <MonitorSpeaker />
        </button>
      </DevicesMenu>
      <VolumeButton :icon-class="[ICON, tone(false)].join(' ')" />
      <button
        type="button"
        :class="[ICON, tone(false), empty && OFF]"
        :disabled="empty"
        aria-label="Queue"
        @click="router.push({ path: '/now-playing', query: { queue: 'open' } })"
      >
        <ListMusic />
      </button>
      <button type="button" :class="[ICON, tone(false), !current && OFF]" :disabled="!current" title="Clear the player" aria-label="Clear the player" @click="clearPlayer">
        <X />
      </button>
    </div>
  </footer>
</template>
