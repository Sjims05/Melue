<script setup lang="ts">
/**
 * The phone's bottom bar: Home · Library · Playlists · Search · Settings, the one you're in marked (under a page opened
 * on top, the main page it was opened from). Its own one again: back to the top of that page (`top`).
 *
 * Switching keeps one step of history at most, so the phone's back goes to where you started, and then out of the
 * app (not back through every page you've been on): the first switch adds a step, the next ones replace it, and
 * switching to where that step goes back to just goes back.
 */
import { House, Library, ListMusic, Search, Settings } from '@lucide/vue'
import { useRouter } from 'vue-router'

const props = defineProps<{ current: string }>()
const emit = defineEmits<{ top: [] }>()
const router = useRouter()

const TABS = [
  { path: '/', label: 'Home', icon: House },
  { path: '/library', label: 'Library', icon: Library },
  { path: '/playlists', label: 'Playlists', icon: ListMusic },
  { path: '/search', label: 'Search', icon: Search },
  { path: '/settings', label: 'Settings', icon: Settings },
]

function go(path: string) {
  if (path === props.current && router.currentRoute.value.path === path) return emit('top')
  const back = window.history.state?.back as string | null | undefined
  if (back === path) router.back()
  else if (back) router.replace(path)
  else router.push(path)
}
</script>

<template>
  <nav class="flex flex-none border-t bg-background pb-[env(safe-area-inset-bottom)]" aria-label="Pages">
    <button
      v-for="tab in TABS"
      :key="tab.path"
      type="button"
      class="flex h-16 min-w-0 flex-1 flex-col items-center justify-center gap-1 text-[11px] font-medium transition-colors"
      :class="tab.path === current ? 'text-foreground' : 'text-muted-foreground'"
      :aria-current="tab.path === current ? 'page' : undefined"
      @click="go(tab.path)"
    >
      <component :is="tab.icon" class="size-[22px]" :stroke-width="tab.path === current ? 2.25 : 1.75" />
      {{ tab.label }}
    </button>
  </nav>
</template>
