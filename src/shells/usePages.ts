/**
 * What both app frames (desktop and phone) do with the pages: which main page shows (kept alive; under a page opened on
 * top, the one it was opened from), which way a switch slides it (down the main pages, or up), each main page scrolled
 * back to where it was, and how layers (pages opened on top) move: opening slides one in over what's there, back
 * slides it out.
 *
 * `under`: main pages that don't replace the one shown (the phone's Now playing slides up over it instead).
 */
import { computed, nextTick, ref, shallowRef, watch, type Component, type Ref } from 'vue'
import { useRoute, useRouter, type RouteLocationNormalizedLoaded } from 'vue-router'
import { MAIN_PAGES } from '@/router'

export function usePages(scroller: Ref<HTMLElement | undefined>, under: (route: RouteLocationNormalizedLoaded) => boolean = () => false) {
  const route = useRoute()
  const router = useRouter()
  const main = shallowRef<{ path: string; component: Component } | null>(null)
  const from = ref<1 | -1>(1)
  const scrolls = new Map<string, number>()
  const dotIndex = (path: string) => MAIN_PAGES.findIndex((p) => p.path === path)
  const home = () => ({ path: '/', component: router.resolve('/').matched[0].components!.default as Component })

  watch(
    () => route.fullPath,
    async () => {
      // Opened straight on a page that goes on top (a reload there): Home underneath.
      if ((!route.meta.main || under(route)) && !main.value) {
        main.value = home()
        return
      }
      if (!route.meta.main || under(route)) return
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

  /** A page is on top of the main page (a layer, or what slides over it). */
  const onTop = computed(() => !route.meta.main || under(route))
  // Each page on top is its own layer (an artist, then its album: two).
  const layerKey = computed(() => route.fullPath.split('?')[0])
  // Which way layers move: a page on top opening slides in over what's there (which stays put under it); going back
  // slides it out, uncovering the one under it. Between two pages on top, the history says which (back is a lower place).
  const motion = ref<'layer-in' | 'layer-out'>('layer-in')
  let place = 0
  router.afterEach((to, before) => {
    const now = (router.options.history.state.position as number | undefined) ?? 0
    motion.value = to.meta.main ? 'layer-out' : before.meta.main ? 'layer-in' : now < place ? 'layer-out' : 'layer-in'
    place = now
  })

  return { main, from, onTop, layerKey, motion }
}
