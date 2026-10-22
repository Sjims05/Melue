<script setup lang="ts">
/**
 * The side dots (Melue's design, the app playground's): one per main page (Home, Library, Playlists, Now playing,
 * Settings; Dev in dev builds). The current page's dot is bright, with its name next to it; the others show theirs on
 * hover. ↑ and ↓ move between them (not while typing). A page opened on top (an artist, Search) slides over them.
 */
import { onKeyStroke } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { MAIN_PAGES } from '@/router'

const route = useRoute()
const router = useRouter()
// The main page shown: under a page on top, still the one it was opened from (the dots stay as they were under it).
const mainPath = ref(route.meta.main ? route.path : '/')
watch(() => route.path, (path) => route.meta.main && (mainPath.value = path))
const current = computed(() => MAIN_PAGES.findIndex((p) => p.path === mainPath.value))

function step(e: KeyboardEvent, by: number) {
  // Not with other keys held (Windows+↑ is the window's), not while a dialog or menu is open, and not where the arrows
  // move within something (a song list, cards, segments, a menu, a field, a button opening a menu: ↓ opens it).
  if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return
  if (document.querySelector('[role=dialog], [role=alertdialog], [role=menu]')) return
  const own = 'input, textarea, [contenteditable], [role=menu], [role=listbox], [role=list], [role=radiogroup], [role=slider], .cover-grid, [aria-haspopup], [data-own-keys]'
  if (!route.meta.main || (e.target as HTMLElement).closest(own)) return
  const next = MAIN_PAGES[current.value + by]
  if (!next) return
  e.preventDefault()
  router.push(next.path)
}
// A dot clicked keeps the focus; once the page changes (the arrows, a link) it lets go, or it would keep its name shown.
watch(
  () => route.path,
  () => {
    const active = document.activeElement as HTMLElement | null
    if (active?.closest('nav[aria-label=Pages]')) active.blur()
  },
)
onKeyStroke('ArrowDown', (e) => step(e, 1))
onKeyStroke('ArrowUp', (e) => step(e, -1))
</script>

<template>
  <nav class="fixed top-1/2 left-11 z-10 flex -translate-y-1/2 flex-col gap-5" aria-label="Pages">
    <RouterLink
      v-for="(page, i) in MAIN_PAGES"
      :key="page.path"
      :to="page.path"
      class="group flex h-6 items-center gap-6 outline-none"
      :aria-current="current === i ? 'page' : undefined"
    >
      <span class="grid w-2 place-items-center">
        <span
          class="block rounded-full transition-all duration-200"
          :class="current === i ? 'size-2 bg-foreground' : 'size-1.5 bg-muted-foreground/60 group-hover:bg-muted-foreground group-focus-visible:bg-muted-foreground'"
        />
      </span>
      <span
        class="text-sm font-medium whitespace-nowrap transition-[opacity,translate] duration-200"
        :class="current === i ? 'opacity-100' : '-translate-x-1 text-muted-foreground opacity-0 group-hover:translate-x-0 group-hover:opacity-100 group-focus-visible:translate-x-0 group-focus-visible:opacity-100'"
      >
        {{ page.meta.main }}
      </span>
    </RouterLink>
  </nav>
</template>
