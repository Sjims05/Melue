<script setup lang="ts">
/**
 * The top bar's search field: clicking (or tabbing) into it opens Search, which slides in from the right with its own
 * big field (this one goes). Ctrl Space or "/" opens it from anywhere (not while typing in another field).
 */
import { onKeyStroke } from '@vueuse/core'
import { Search } from '@lucide/vue'
import { useRouter } from 'vue-router'

const router = useRouter()
const open = () => router.push({ name: 'search' })

onKeyStroke((e) => {
  // A key can come from the page itself (not an element: Escape sent to close a menu), which has no closest().
  const typing = (e.target as HTMLElement | null)?.closest?.('input, textarea, [contenteditable]')
  if ((e.code === 'Space' && e.ctrlKey && !e.altKey && !e.metaKey) || (e.key === '/' && !typing)) {
    e.preventDefault()
    open()
  }
})
</script>

<template>
  <button
    type="button"
    class="flex h-9 w-[250px] items-center gap-2 rounded-lg border border-border bg-card px-3 text-sm text-muted-foreground transition-colors hover:text-foreground dark:border-transparent"
    @click="open"
    @focus="open"
  >
    <Search class="size-4 flex-none" />
    <span class="flex-1 text-left">Search…</span>
    <kbd class="flex-none rounded bg-well px-1.5 py-0.5 font-sans text-[11px] text-muted-foreground shadow-[inset_0_0_0_1px_var(--well-line)]">Ctrl Space</kbd>
  </button>
</template>
