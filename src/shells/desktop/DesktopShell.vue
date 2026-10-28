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
import { provide, ref } from 'vue'
import { useRoute } from 'vue-router'
import { TooltipProvider } from '@/components/ui/tooltip'
import { backLabel, goBack } from '@/router'
import { usePages } from '../usePages'
import { PLAYLIST_PICKER } from '@/components/songs'
import { startNowPlaying } from '@/player/nowPlaying'
import { playlistPicker } from '@/player/playlistPicker'
import { followOnlyHere } from '@/library/onlyHere'
import { followSharedSettings } from '@/settings/sharedSettings'
import ConfirmHost from '../ConfirmHost.vue'
import DropSongs from '@/settings/DropSongs.vue'
import UpdateDialog from './UpdateDialog.vue'
import CopyMenu from './CopyMenu.vue'
import KeyboardShortcuts from './KeyboardShortcuts.vue'
import ReportDialog from '@/components/report/ReportDialog.vue'
import RemoteAsk from '@/components/player/RemoteAsk.vue'
import { startHotkeys } from '@/lib/hotkeys'
import DotsNav from './DotsNav.vue'
import PlayerBar from './PlayerBar.vue'
import Toasts from '../Toasts.vue'
import TopBar from './TopBar.vue'

const route = useRoute()
// What's playing, for the player bar and Now playing; the song menus' Add to playlist.
startNowPlaying()
// Which songs are only on this device (their badge, Move to sync).
followOnlyHere()
// Settings shared across devices (sharedSettings.ts).
followSharedSettings()
provide(PLAYLIST_PICKER, playlistPicker)

// The main page shown, the switches' way, each main page's scroll, and the layers (usePages, shared with the phone).
const scroller = ref<HTMLElement>()
const { main, from, onTop, layerKey, motion } = usePages(scroller)
// The keyboard shortcuts (lib/hotkeys.ts).
startHotkeys()
// Escape: back from a page on top (a field handles its own Escape first).
onKeyStroke('Escape', (e) => {
  if (onTop.value && !(e.target as HTMLElement | null)?.closest?.('input, textarea, [role=dialog], [role=menu]')) goBack()
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
            <!-- Over the page, under the top bar (its window buttons and dragging stay): Now playing's lyrics full size. -->
            <div id="page-overlay" class="contents" />
            <!-- The scrollbar's room is always kept, so the page doesn't shift when it comes or goes. No scroll anchoring:
                 the page being replaced is above the new one for a moment, and the browser would move the new one's
                 place up by its height when it goes. -->
            <div ref="scroller" data-scroller class="absolute inset-0 overflow-x-hidden overflow-y-auto [overflow-anchor:none] [scrollbar-gutter:stable]" :style="{ '--page-from': from }">
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
        <!-- Notes like "Added to the queue", just above the player bar (over any page). -->
        <Toasts />
      </div>
      <!-- The player bar, except on Now playing itself (not under a page opened from it): it slides away down. With the
           lyrics full size (?lyrics) it's back: the controls for them. -->
      <div
        class="grid transition-[grid-template-rows] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]"
        :class="route.path === '/now-playing' && !('lyrics' in route.query) ? 'grid-rows-[0fr]' : 'grid-rows-[1fr]'"
      >
        <div class="min-h-0 overflow-hidden"><PlayerBar /></div>
      </div>
      <!-- Music files dragged over Melue, on any page: the drop area, then the question. -->
      <DropSongs />
      <!-- An update found: Melue asks (lib/updates.ts). -->
      <UpdateDialog />
      <!-- Copy, for selected text or an error tip (lib/copyMenu.ts). -->
      <CopyMenu />
      <!-- Keyboard shortcuts (lib/hotkeys.ts) and their list (?). -->
      <KeyboardShortcuts />
      <!-- Report a bug (About, or Ctrl+Shift+B). -->
      <ReportDialog />
      <!-- Another device asking to play here (Let other devices play here: Ask first). -->
      <RemoteAsk />
      <!-- Questions asked from anywhere (lib/confirm.ts: a song's Move to sync). -->
      <ConfirmHost />
    </div>
  </TooltipProvider>
</template>
