/**
 * Melue's pages. The main ones are the side dots (Home, Library, Playlists, Now playing, Settings); the others open on
 * top of them (an artist, an album, a folder, a playlist, Search) with a back link named after the page they came
 * from ("‹ Library"). Melue Dev also has the dev page (its tests and tools: lib/devBuild.ts, `addDevPage`).
 *
 * A page's title: the main pages' are here; a page that only knows its title once loaded (an artist's name) sets it
 * with `setPageTitle`, so a page opened from it can say "‹ Juniper Vale".
 */
import { computed, ref, shallowReactive } from 'vue'
import { createRouter, createWebHashHistory, type RouteLocationNormalized } from 'vue-router'
import type { StartOn } from '@/lib/appearance'
// The main pages load with the app (switching to one slides it in at once); the others when first opened.
import HomePage from '@/pages/HomePage.vue'
import LibraryPage from '@/pages/LibraryPage.vue'
import NowPlayingPage from '@/pages/NowPlayingPage.vue'
import PlaylistsPage from '@/pages/PlaylistsPage.vue'
import SettingsRoute from '@/pages/SettingsRoute.vue'

declare module 'vue-router' {
  interface RouteMeta {
    /** A main page (a side dot), and its name there. */
    main?: string
    title?: string
  }
}

export const router = createRouter({
  // Hash addresses: Melue's pages aren't files the app's server knows.
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomePage, meta: { main: 'Home', title: 'Home' } },
    { path: '/library', name: 'library', component: LibraryPage, meta: { main: 'Library', title: 'Library' } },
    { path: '/playlists', name: 'playlists', component: PlaylistsPage, meta: { main: 'Playlists', title: 'Playlists' } },
    { path: '/now-playing', name: 'now-playing', component: NowPlayingPage, meta: { main: 'Now playing', title: 'Now playing' } },
    { path: '/settings', name: 'settings', component: SettingsRoute, meta: { main: 'Settings', title: 'Sync & settings' } },
    { path: '/search', name: 'search', component: () => import('@/pages/SearchPage.vue'), meta: { title: 'Search' } },
    { path: '/artist/:id', name: 'artist', component: () => import('@/pages/ArtistPage.vue'), meta: { title: 'Artist' } },
    { path: '/album/:id', name: 'album', component: () => import('@/pages/AlbumPage.vue'), meta: { title: 'Album' } },
    { path: '/folder/:id', name: 'folder', component: () => import('@/pages/FolderPage.vue'), meta: { title: 'Folder' } },
    { path: '/playlist/:id(\\d+)', name: 'playlist', component: () => import('@/pages/PlaylistPage.vue'), meta: { title: 'Playlist' } },
    // Made for you: All songs, Liked songs, the Top songs lists (/playlist/songs, /playlist/liked, /playlist/top-month).
    { path: '/playlist/:auto', name: 'auto-playlist', component: () => import('@/pages/AutoPlaylistPage.vue'), meta: { title: 'Playlist' } },
    { path: '/:rest(.*)*', redirect: '/' },
  ],
  // A page opens at its top; going back returns to where it was.
  scrollBehavior: (_to, _from, saved) => saved ?? { top: 0 },
})

/**
 * The pages opened on top load when first opened; once Melue has started and is idle, they're loaded ahead, so
 * opening one the first time doesn't stutter while it slides in (on a phone especially).
 */
export function preloadPages() {
  const load = () => {
    for (const route of router.getRoutes()) {
      const page = route.components?.default
      if (typeof page === 'function' && !('setup' in page) && !('render' in page)) (page as () => Promise<unknown>)().catch(() => {})
    }
    // Settings' page, loaded on its own (SettingsRoute.vue).
    import('@/settings/SettingsPage.vue').catch(() => {})
  }
  if ('requestIdleCallback' in window) requestIdleCallback(load, { timeout: 3000 })
  else setTimeout(load, 1500)
}

/** The main pages, in the side dots' order. */
export const MAIN_PAGES = shallowReactive(mainPages())
function mainPages() {
  return router.getRoutes().filter((r) => r.meta.main).sort((a, b) => order(a.path) - order(b.path))
}

/** Melue Dev only: the dev page, a main page after Settings (main.ts adds it before the app is drawn). */
export function addDevPage() {
  router.addRoute({ path: '/dev', name: 'dev', component: () => import('@/pages/DevPage.vue'), meta: { main: 'Dev', title: 'Dev' } })
  MAIN_PAGES.splice(0, MAIN_PAGES.length, ...mainPages())
}
function order(path: string) {
  return ['/', '/library', '/playlists', '/now-playing', '/settings', '/dev'].indexOf(path)
}

// ---- Titles, and the back link

const titles = ref(new Map<string, string>())
/** A page's title once it knows it (an artist's name): what a page opened from it calls it ("‹ Juniper Vale"). */
export function setPageTitle(route: RouteLocationNormalized, title: string) {
  titles.value.set(route.fullPath, title)
}
const titleOf = (path: string) => titles.value.get(path) ?? router.resolve(path).meta.title ?? 'Back'

/** Where the back link goes: the page this one was opened from (vue-router keeps it in the history's state). */
const backPath = ref<string | null>(null)
router.afterEach(() => {
  backPath.value = (window.history.state?.back as string | null) ?? null
})
/** The back link's words, or null on a main page. */
export const backLabel = computed(() => {
  if (router.currentRoute.value.meta.main) return null
  return backPath.value ? titleOf(backPath.value) : 'Home'
})
export function goBack() {
  if (backPath.value) router.back()
  else router.push('/')
}

// ---- Start on (Display settings): the page Melue opens on. The last main page shown is kept for "Where I left off".
const LAST_PAGE = 'melue-last-page'
router.afterEach((to) => {
  if (!to.meta.main || to.name === 'now-playing' || to.name === 'dev') return
  try {
    localStorage.setItem(LAST_PAGE, to.path)
  } catch {
    // Not kept: Home next time.
  }
})
/** The page to open at start when none is asked for (the address has none). */
export function startPage(startOn: StartOn): string {
  if (startOn === 'library') return '/library'
  if (startOn === 'last') {
    try {
      const last = localStorage.getItem(LAST_PAGE)
      if (last && router.resolve(last).meta.main) return last
    } catch {
      // Nothing kept.
    }
  }
  return '/'
}

// Development: a change to this file reloads the page. Swapped in place (hot reload), the app would keep the router it
// started with while the top bar and the back link read a second, new one (it would always think it's on Home).
if (import.meta.hot) import.meta.hot.accept(() => import.meta.hot!.invalidate())
