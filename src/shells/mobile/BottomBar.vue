<script setup lang="ts">
/**
 * The phone's bottom bar: Home · Library · Playlists · Search · Settings, the one you're in marked (under a page opened
 * on top, the main page it was opened from): brighter, and a small bar above it that slides to the tab you switch to.
 * Its own one again: back to the top of that page (`top`).
 *
 * Switching keeps one step of history at most, so the phone's back goes to where you started, and then out of the
 * app (not back through every page you've been on): the first switch adds a step, the next ones replace it, and
 * switching to where that step goes back to just goes back.
 */
import { House, Library, ListMusic, Search, Settings } from '@lucide/vue'
import { computed } from 'vue'
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

const at = computed(() => TABS.findIndex((t) => t.path === props.current))

function go(path: string) {
  if (path === props.current && router.currentRoute.value.path === path) return emit('top')
  const back = window.history.state?.back as string | null | undefined
  if (back === path) router.back()
  else if (back) router.replace(path)
  else router.push(path)
}
</script>

<template>
  <nav class="relative flex flex-none border-t bg-background pb-[env(safe-area-inset-bottom)]" aria-label="Pages">
    <!-- The marker: over the tab you're in, sliding when you switch. -->
    <span
      v-if="at >= 0"
      aria-hidden="true"
      class="absolute -top-px left-0 flex h-[3px] justify-center transition-transform duration-300 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none"
      :style="{ width: `${100 / TABS.length}%`, transform: `translateX(${at * 100}%)` }"
    >
      <span class="h-full w-8 rounded-b-full bg-foreground" />
    </span>
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
