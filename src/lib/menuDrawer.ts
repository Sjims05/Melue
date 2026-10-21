/**
 * A phone's menus: what a ⋯ or a long press opens there (a drawer from the bottom, MenuDrawer.vue in the phone's frame),
 * instead of the computer's drop-down and right-click menus. Opening one adds a step to the history, so the phone's
 * back closes it (not the page under it). Picking an action closes it first, then does it (an action that opens a
 * page opens it from the page, not from the drawer's step).
 */
import { shallowRef } from 'vue'
import type { Router } from 'vue-router'
import type { Component } from 'vue'
import type { MenuItem } from '@/components/covers/coverMenu'
import type { CoverColors } from '@/components/covers/coverRecipe'
import type { Song } from '@/components/songs/songMenu'

export interface DrawerMenu {
  /** What it's about: shown at its top. */
  title: string
  subtitle?: string
  /** Its cover, as the card or the page shows it (Cover.vue's props). */
  cover?: { image?: string; colors?: CoverColors; icon?: Component; collage?: readonly (string | CoverColors)[]; placeholder?: Component }
  items: MenuItem[]
  pick: (action: string) => unknown
  /** A song's menu: Add to playlist lists the playlists in the drawer. */
  song?: Song
  /** A choice (a setting's value): the items are the choices, this one ticked, and the drawer opens scrolled to it. */
  picked?: string
}

export const drawerMenu = shallowRef<DrawerMenu | null>(null)

const STEP = 'menu'
// The app's router, from the drawer (MenuDrawer.vue): not imported here, as the router's pages use these menus.
let router: Router | null = null

/** The drawer, once it's there: the history steps go through this router, and its back closes the drawer. */
export function connectMenus(appRouter: Router) {
  router = appRouter
  appRouter.afterEach((to) => {
    if (!(STEP in to.query)) drawerMenu.value = null
  })
}

export function openMenu(menu: DrawerMenu) {
  drawerMenu.value = menu
  const route = router?.currentRoute.value
  if (route && !(STEP in route.query)) router!.push({ query: { ...route.query, [STEP]: '1' } })
}

/** Closes it (back out of its step), then `after` (an action picked). */
export function closeMenu(after?: () => unknown) {
  drawerMenu.value = null
  if (!router || !(STEP in router.currentRoute.value.query)) return void after?.()
  const done = router.afterEach(() => {
    done()
    after?.()
  })
  router.back()
}
