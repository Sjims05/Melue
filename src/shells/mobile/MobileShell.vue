<script setup lang="ts">
/**
 * The phone app's frame: the same pages as on a computer, with the phone's way around them. A top bar, the page, the
 * mini player and the bottom bar (Home · Library · Playlists · Search · Settings). Main pages are kept alive and slide
 * in like on a computer; a page opened from one (an artist, an album, Search) is a layer sliding in from the right,
 * with its own top bar; a swipe to the right (or the phone's back) closes it. Now playing slides up over everything
 * from the mini player; the chevron, a pull down, or the phone's back closes it. Pulling a main page down from its top
 * syncs (pullToRefresh.ts).
 */
import { ChevronDown, RefreshCw } from '@lucide/vue'
import { computed, provide, ref, watch } from 'vue'
import { useRoute, type RouteLocationNormalizedLoaded } from 'vue-router'
import { PLAYLIST_PICKER } from '@/components/songs'
import { followOnlyHere } from '@/library/onlyHere'
import { followSharedSettings } from '@/settings/sharedSettings'
import { PHONE } from '@/lib/phone'
import NowPlayingPage from '@/pages/NowPlayingPage.vue'
import { startNowPlaying } from '@/player/nowPlaying'
import { playlistPicker } from '@/player/playlistPicker'
import { backLabel, goBack } from '@/router'
import ConfirmHost from '../ConfirmHost.vue'
import Toasts from '../Toasts.vue'
import { usePages } from '../usePages'
import BottomBar from './BottomBar.vue'
import MiniPlayer from './MiniPlayer.vue'
import MobileTopBar from './MobileTopBar.vue'
import { usePullToClose } from './pullToClose'
import { READY, usePullToRefresh } from './pullToRefresh'
import MenuDrawer from './MenuDrawer.vue'
import ReportDialog from '@/components/report/ReportDialog.vue'
import RemoteAsk from '@/components/player/RemoteAsk.vue'

const route = useRoute()
// What's playing; the song menus' Add to playlist; which songs are only on this device.
startNowPlaying()
followOnlyHere()
// Settings shared across devices (sharedSettings.ts).
followSharedSettings()
provide(PLAYLIST_PICKER, playlistPicker)
// The pages are in the phone's frame (their phone sizes, a tap on a song plays it).
provide(PHONE, true)

// Now playing isn't a page of its own here: it slides up over the page that was showing.
const isNowPlaying = (r: RouteLocationNormalizedLoaded) => r.name === 'now-playing'
const scroller = ref<HTMLElement>()
const { main, from, onTop, layerKey, motion } = usePages(scroller, isNowPlaying)
const nowPlayingOpen = computed(() => isNowPlaying(route))
const tab = computed(() => (route.name === 'search' ? '/search' : (main.value?.path ?? '/')))
const toTop = () => scroller.value?.scrollTo({ top: 0, behavior: 'smooth' })

// Pulling away to close, from anywhere on the screen (pullToClose.ts): Now playing down, a page on top to the right.
const sheet = ref<HTMLElement | null>(null)
const nowPlayingPull = usePullToClose(sheet, 'down', goBack)
const layer = ref<HTMLElement | null>(null)
const layerPull = usePullToClose(layer, 'right', goBack)
// A new page on top starts where it should (not where the last one was let go).
watch(layerKey, () => layerPull.reset())
// Pulling a main page down from its top: everything up to date.
const refresh = usePullToRefresh(scroller)
</script>

<template>
  <div class="mobile-shell flex h-dvh flex-col overflow-hidden bg-background pt-[env(safe-area-inset-top)] text-foreground">
    <div class="relative min-h-0 flex-1">
      <!-- The main pages: one top bar, and the page, kept alive. Out of reach while something covers them. -->
      <div class="absolute inset-0 flex flex-col" :inert="onTop || undefined">
        <MobileTopBar />
        <div ref="scroller" data-scroller class="relative min-h-0 flex-1 overflow-y-auto [overflow-anchor:none]" :style="{ '--page-from': from }">
          <!-- Pull to refresh's mark: comes down with the finger, turning; spins while it refreshes. -->
          <div class="pointer-events-none sticky top-0 z-10 flex h-0 justify-center" aria-hidden="true">
            <span
              class="grid size-10 place-items-center rounded-full border bg-card text-muted-foreground shadow-md"
              :class="[!refresh.pulling.value && 'transition-[translate,opacity] duration-200 ease-out', refresh.pull.value >= READY && 'text-foreground']"
              :style="{ translate: `0 ${refresh.pull.value - 44}px`, opacity: Math.min(1, refresh.pull.value / READY) }"
            >
              <RefreshCw class="size-5" :class="refresh.refreshing.value && 'animate-spin'" :style="!refresh.refreshing.value ? { rotate: `${refresh.pull.value * 4}deg` } : undefined" />
            </span>
          </div>
          <main class="app-column pb-8">
            <Transition name="page" :duration="{ enter: 320, leave: 0 }">
              <KeepAlive>
                <component :is="main.component" v-if="main" :key="main.path" />
              </KeepAlive>
            </Transition>
          </main>
        </div>
      </div>
      <!-- A page opened on top: a layer with its own top bar, sliding in from the right. Its transition covers both its
           slide (transform, page.css) and following a swipe (translate): one of them alone would stop the other. -->
      <RouterView v-slot="{ Component, route: shown }">
        <Transition :name="motion" :duration="340">
          <div
            v-if="shown.matched.length && !shown.meta.main"
            :key="layerKey"
            :ref="(el) => (layer = el as HTMLElement | null)"
            class="absolute inset-0 z-20 flex flex-col bg-background"
            :class="!layerPull.pulling.value && 'transition-[translate,transform] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]'"
            :style="{ translate: `${layerPull.pull.value}px 0` }"
          >
            <MobileTopBar :back="backLabel" />
            <div data-scroller class="min-h-0 flex-1 overflow-y-auto">
              <main class="app-column pb-8"><component :is="Component" /></main>
            </div>
          </div>
        </Transition>
      </RouterView>
      <!-- Notes like "Added to the queue", above the mini player. -->
      <Toasts />
    </div>
    <MiniPlayer />
    <BottomBar :current="tab" @top="toTop" />

    <!-- Now playing: up over everything (the bars too), from the bottom. -->
    <Transition name="sheet" @after-leave="nowPlayingPull.reset">
      <div
        v-if="nowPlayingOpen"
        ref="sheet"
        class="fixed inset-0 z-40 flex flex-col bg-background pt-[env(safe-area-inset-top)]"
        :class="!nowPlayingPull.pulling.value && 'transition-[translate,transform] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]'"
        :style="{ translate: `0 ${nowPlayingPull.pull.value}px` }"
      >
        <div class="flex h-14 flex-none items-center gap-2 px-2">
          <button type="button" class="grid size-10 place-items-center rounded-md" aria-label="Close Now playing" @click="goBack">
            <ChevronDown class="size-6" />
          </button>
          <span class="text-[15px] font-semibold">Now playing</span>
          <span class="absolute top-[calc(env(safe-area-inset-top)+6px)] left-1/2 h-1 w-9 -translate-x-1/2 rounded-full bg-muted-foreground/30" aria-hidden="true" />
        </div>
        <div data-scroller class="min-h-0 flex-1 overflow-y-auto">
          <main class="app-column pb-10">
            <KeepAlive><NowPlayingPage /></KeepAlive>
          </main>
        </div>
      </div>
    </Transition>
    <!-- Menus: up from the bottom (⋯, a long press). -->
    <MenuDrawer />
    <!-- Report a bug (About). -->
    <ReportDialog />
    <!-- Another device asking to play here (Let other devices play here: Ask first). -->
    <RemoteAsk />
    <ConfirmHost />
  </div>
</template>

<style scoped>
/* The page's side room on a phone: a little, the same on both sides (page.css: --col-left / --col-right). */
.mobile-shell {
  --col-left: 1rem;
  --col-right: 1rem;
  /* A long press opens Melue's own menus, not the browser's (copy, open link). */
  -webkit-touch-callout: none;
}
/* Now playing sliding up from the bottom, and back down. */
.sheet-enter-active,
.sheet-leave-active {
  transition: transform 340ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.sheet-enter-from,
.sheet-leave-to {
  transform: translateY(100%);
}
@media (prefers-reduced-motion: reduce) {
  .sheet-enter-active,
  .sheet-leave-active {
    transition: none;
  }
}
</style>
