<script setup lang="ts">
/**
 * The phone's menu drawer (lib/menuDrawer.ts): up from the bottom, over a dimmed page. At its top what it's about (the
 * cover, the title and a line), then the actions as rows, those that take something away in red. A song's Add to
 * playlist turns it into the list of playlists (✓ on those that have the song; New playlist… first), with a way back.
 * A choice (a setting's value) lists the choices, ✓ on the one picked, opening scrolled to it.
 * Tapping outside, pulling it down or the phone's back closes it.
 */
import { Check, ChevronLeft, ListMusic, Plus } from '@lucide/vue'
import { computed, inject, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Cover } from '@/components/covers'
import { PLAYLIST_PICKER, type PlaylistChoice } from '@/components/songs'
import { closeMenu, connectMenus, drawerMenu } from '@/lib/menuDrawer'
import { usePullToClose } from './pullToClose'

connectMenus(useRouter())
const menu = computed(() => drawerMenu.value)
const picker = inject(PLAYLIST_PICKER, null)

// Add to playlist: the playlists, in the drawer.
const choosing = ref<PlaylistChoice[] | null>(null)
watch(menu, () => (choosing.value = null))
async function choosePlaylist() {
  const song = menu.value?.song
  if (song && picker) choosing.value = await picker.list(song).catch(() => [])
}

function pick(action: string) {
  const m = menu.value
  if (!m) return
  if (action === 'addToPlaylist' && m.song) return void choosePlaylist()
  closeMenu(() => m.pick(action))
}
function addTo(id: number | 'new') {
  const song = menu.value?.song
  if (!song || !picker) return
  closeMenu(() => (id === 'new' ? picker.create(song) : picker.add(song, id)))
}

// Tapping outside closes it, but only a tap that started there: the finger of a long press is still down when it
// opens, and letting go would close it at once.
const downOutside = ref(false)
watch(menu, () => (downOutside.value = false))

const panel = ref<HTMLElement | null>(null)
const pull = usePullToClose(panel, 'down', () => closeMenu())

// A long choice (a time of day): the picked one in view.
watch(menu, async (m) => {
  if (!m?.picked) return
  await nextTick()
  panel.value?.querySelector('[aria-checked=true]')?.scrollIntoView({ block: 'center' })
})
</script>

<template>
  <Transition name="fade" @after-leave="pull.reset">
    <div v-if="menu" class="fixed inset-0 z-50 bg-black/40" aria-hidden="true" @pointerdown="downOutside = true" @click="downOutside && closeMenu()" />
  </Transition>
  <Transition name="drawer">
    <div
      v-if="menu"
      ref="panel"
      role="dialog"
      aria-modal="true"
      :aria-label="menu.title"
      class="fixed inset-x-0 bottom-0 z-50 flex max-h-[80dvh] flex-col rounded-t-2xl bg-card pb-[env(safe-area-inset-bottom)] text-foreground shadow-xl"
      :class="!pull.pulling.value && 'transition-[translate,transform] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]'"
      :style="{ translate: `0 ${pull.pull.value}px` }"
    >
      <span class="mx-auto mt-2 h-1 w-9 flex-none rounded-full bg-muted-foreground/30" aria-hidden="true" />
      <!-- What it's about. -->
      <div class="flex flex-none items-center gap-3 border-b px-4 pt-2 pb-3">
        <button v-if="choosing" type="button" class="-ml-2 grid size-10 flex-none place-items-center" aria-label="Back to the menu" @click="choosing = null">
          <ChevronLeft class="size-5" />
        </button>
        <span v-else-if="menu.cover" class="w-11 flex-none [&>div]:rounded-md">
          <Cover v-bind="menu.cover" />
        </span>
        <span class="flex min-w-0 flex-col">
          <span class="truncate text-[15px] font-semibold">{{ choosing ? 'Add to playlist' : menu.title }}</span>
          <span v-if="choosing || menu.subtitle" class="truncate text-[13px] text-muted-foreground">{{ choosing ? menu.title : menu.subtitle }}</span>
        </span>
      </div>
      <div class="min-h-0 overflow-y-auto py-1.5">
        <template v-if="choosing">
          <button type="button" class="flex h-12 w-full items-center gap-4 px-5 text-left text-[15px]" @click="addTo('new')"><Plus class="size-5" />New playlist…</button>
          <button v-for="c in choosing" :key="c.id" type="button" class="flex h-12 w-full items-center gap-4 px-5 text-left text-[15px]" @click="addTo(c.id)">
            <ListMusic class="size-5 text-muted-foreground" />
            <span class="min-w-0 flex-1 truncate">{{ c.name }}</span>
            <Check v-if="c.has" class="size-5" aria-label="Has this song" />
          </button>
        </template>
        <template v-for="item in menu.items" v-else :key="item.action">
          <div v-if="item.apart" class="mx-5 my-1.5 border-t" />
          <button
            type="button"
            :role="menu.picked !== undefined ? 'menuitemradio' : undefined"
            :aria-checked="menu.picked !== undefined ? item.action === menu.picked : undefined"
            class="flex h-12 w-full flex-none items-center gap-4 px-5 text-left text-[15px] active:bg-well"
            :class="item.danger && 'text-destructive'"
            @click="pick(item.action)"
          >
            <component :is="item.icon" v-if="item.icon" class="size-5" :class="[item.filled && 'fill-current', !item.danger && 'text-muted-foreground']" />
            <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
            <Check v-if="menu.picked !== undefined && item.action === menu.picked" class="size-5" aria-hidden="true" />
          </button>
        </template>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.drawer-enter-active,
.drawer-leave-active {
  transition: transform 300ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.drawer-enter-from,
.drawer-leave-to {
  transform: translateY(100%);
}
@media (prefers-reduced-motion: reduce) {
  .drawer-enter-active,
  .drawer-leave-active {
    transition: none;
  }
}
</style>
