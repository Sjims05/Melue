<script setup lang="ts">
/**
 * The desktop app's frame (Melue's design, the app playground's motion): the top bar, the side dots, and the pages.
 *
 * Main pages (the dots) are kept alive (they keep what they loaded, and where they were scrolled); switching slides
 * the new one in from below or above, the way the dots go, under a top bar that stays put. A page opened from one (an
 * artist, a folder, Search) is a layer of its own, with its own top bar: it slides in from the right over everything
 * (the top bar and the dots too), the main page staying as it was underneath; back slides it out again. Another page
 * opened from it slides in over it the same way. The player bar sits at the bottom, except on Now playing.
 */
import { onKeyStroke } from '@vueuse/core'
import { computed, nextTick, provide, ref, shallowRef, watch, type Component } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { TooltipProvider } from '@/components/ui/tooltip'
import { backLabel, goBack, MAIN_PAGES } from '@/router'
import { PLAYLIST_PICKER } from '@/components/songs'
import { startNowPlaying } from '@/player/nowPlaying'
import { playlistPicker } from '@/player/playlistPicker'
import DotsNav from './DotsNav.vue'
import PlayerBar from './PlayerBar.vue'
import TopBar from './TopBar.vue'

const route = useRoute()
const router = useRouter()
// What's playing, for the player bar and Now playing; the song menus' Add to playlist.
startNowPlaying()
provide(PLAYLIST_PICKER, playlistPicker)

// The main page shown (under a page opened on top, the one it was opened from), and which way the dots went.
const main = shallowRef<{ path: string; component: Component } | null>(null)
const from = ref<1 | -1>(1)
const scroller = ref<HTMLElement>()
const scrolls = new Map<string, number>()
const dotIndex = (path: string) => MAIN_PAGES.findIndex((p) => p.path === path)

watch(
  () => route.fullPath,
  async () => {
    // Opened straight on a page that goes on top (a reload there): Home underneath.
    if (!route.meta.main && !main.value) {
      main.value = { path: '/', component: router.resolve('/').matched[0].components!.default as Component }
      return
    }
    if (!route.meta.main) return
    const component = route.matched[0]?.components?.default as Component | undefined
    if (!component || main.value?.path === route.path) return
    if (main.value) {
      from.value = dotIndex(route.path) >= dotIndex(main.value.path) ? 1 : -1
      scrolls.set(main.value.path, scroller.value?.scrollTop ?? 0)
    }
    main.value = { path: route.path, component }
    // Each main page comes back where it was scrolled.
    await nextTick()
    scroller.value?.scrollTo({ top: scrolls.get(route.path) ?? 0 })
  },
  { immediate: true },
)

const onTop = computed(() => !route.meta.main)
// Escape: back from a page on top (a field handles its own Escape first).
onKeyStroke('Escape', (e) => {
  if (onTop.value && !(e.target as HTMLElement).closest('input, textarea, [role=dialog], [role=menu]')) goBack()
})
// Each page on top is its own layer (an artist, then its album: two).
const layerKey = computed(() => route.fullPath.split('?')[0])
// Which way layers move: a page on top opening slides in over what's there (which stays put under it); going back
// slides it out, uncovering the one under it. Between two pages on top, the history says which (back is a lower place).
const motion = ref<'layer-in' | 'layer-out'>('layer-in')
let place = 0
router.afterEach((to, from) => {
  const now = (router.options.history.state.position as number | undefined) ?? 0
  motion.value = to.meta.main ? 'layer-out' : from.meta.main ? 'layer-in' : now < place ? 'layer-out' : 'layer-in'
  place = now
})
</script>

<template>
  <TooltipProvider>
    <div class="flex h-dvh flex-col overflow-hidden bg-background text-foreground">
      <div class="relative min-h-0 flex-1">
        <!-- The main pages: one top bar that stays put, the dots, and the page, kept alive, sliding in the way the dots
             went. Out of reach (keys, focus) while a page on top covers them. -->
        <div class="absolute inset-0 flex flex-col" :inert="onTop || undefined">
          <TopBar />
          <div class="relative min-h-0 flex-1">
            <DotsNav />
            <!-- The scrollbar's room is always kept, so the page doesn't shift when it comes or goes. No scroll anchoring:
                 the page being replaced is above the new one for a moment, and the browser would move the new one's
                 place up by its height when it goes. -->
            <div ref="scroller" data-scroller class="absolute inset-0 overflow-y-auto [overflow-anchor:none] [scrollbar-gutter:stable]" :style="{ '--page-from': from }">
              <main class="app-column pb-24">
                <Transition name="page" :duration="{ enter: 320, leave: 0 }">
                  <KeepAlive>
                    <component :is="main.component" v-if="main" :key="main.path" />
                  </KeepAlive>
                </Transition>
              </main>
            </div>
          </div>
        </div>
        <!-- A page opened on top: a layer with its own top bar, sliding in from the right over everything. -->
        <RouterView v-slot="{ Component, route: shown }">
          <Transition :name="motion" :duration="340">
            <!-- Not before a page is known (starting, or a reload): the blank starting point isn't a page on top. -->
            <div v-if="shown.matched.length && !shown.meta.main" :key="layerKey" class="absolute inset-0 z-20 flex flex-col bg-background">
              <TopBar :back="backLabel" :search="shown.name !== 'search'" />
              <div data-scroller class="min-h-0 flex-1 overflow-y-auto [scrollbar-gutter:stable]">
                <main class="app-column pb-24"><component :is="Component" /></main>
              </div>
            </div>
          </Transition>
        </RouterView>
      </div>
      <!-- The player bar, except on Now playing itself (not under a page opened from it): it slides away down. -->
      <div
        class="grid transition-[grid-template-rows] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]"
        :class="route.path === '/now-playing' ? 'grid-rows-[0fr]' : 'grid-rows-[1fr]'"
      >
        <div class="min-h-0 overflow-hidden"><PlayerBar /></div>
      </div>
    </div>
  </TooltipProvider>
</template>
