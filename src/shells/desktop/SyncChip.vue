<script setup lang="ts">
/**
 * The top bar's sync status (with a share): a dot and a few words ("Synced 12 min ago", "Syncing 3 of 12", "Can't sync:
 * see Settings"). A quick sync is held on screen a moment, like on the status card (shownSync.ts). It opens Settings,
 * where the status card has the details and Sync now.
 */
import { computed } from 'vue'
import { StatusDot } from '@/components/settings'
import { sync } from '@/library/sync'
import { ago } from '@/settings/format'
import { useShownSync } from '@/settings/shownSync'

const shown = useShownSync()

const state = computed(() => {
  const s = sync.status
  const p = shown.value
  if (p) return { dot: 'busy' as const, words: p.filesTotal ? `Syncing ${p.filesDone} of ${p.filesTotal}` : 'Syncing…' }
  if (s?.problem) return { dot: 'bad' as const, words: 'Can’t sync: see Settings' }
  if (s?.last) return { dot: 'ok' as const, words: `Synced ${ago(s.last.atMs)}` }
  return { dot: 'ok' as const, words: 'Not synced yet' }
})
</script>

<template>
  <RouterLink
    v-if="sync.available"
    to="/settings"
    class="flex h-9 items-center gap-2 rounded-md px-2 text-xs whitespace-nowrap text-muted-foreground transition-colors hover:text-foreground"
  >
    <StatusDot :kind="state.dot" />
    <Transition name="swap" mode="out-in"><span :key="state.words">{{ state.words }}</span></Transition>
  </RouterLink>
</template>
