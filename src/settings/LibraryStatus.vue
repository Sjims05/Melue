<script setup lang="ts">
/**
 * The library at a glance, at the top of Library: how many songs, albums, artists, playlists and folders, the last
 * scan, and Scan now (while scanning: its progress). Under it, only when there's something to do, a row each with its
 * action: songs only on this device (Move), and hidden copies of synced songs (Remove copies).
 *
 * Not wired yet: songs that can't be found (Melue doesn't count them yet).
 */
import { CloudUpload, Copy } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { ConfirmDialog, IconTile, Reveal, SettingsCard, SettingsRow, SettingsTip, StatusDot } from '@/components/settings'
import { library } from '@/library'
import { describeSyncFailure, moving } from '@/library/sync'
import { isMobile } from '@/platform'
import { ago, size } from './format'
import { lastScan, libraryState, refreshLibrary, startLibraryState } from './libraryState'

const asking = ref<'move' | 'copies' | null>(null)
const error = ref('')

onMounted(startLibraryState)

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`
const counts = computed(() => {
  const s = libraryState.status
  if (!s) return ''
  return [plural(s.albums, 'album'), plural(s.artists, 'artist'), plural(s.playlists, 'playlist'), `in ${plural(s.folders, 'folder')}`].join(' · ')
})
const onlyHereBytes = computed(() => libraryState.onlyHere.reduce((n, s) => n + s.size, 0))
const scanned = computed(() => {
  const at = lastScan()
  return at ? `Scanned ${ago(at)}` : ''
})

async function act(action: () => Promise<unknown>) {
  error.value = ''
  try {
    await action()
  } catch (e) {
    error.value = describeSyncFailure(e)
  }
  await refreshLibrary()
}
const scan = () => act(() => library.scan())
const moveAll = () => act(() => moving.moveToSync(libraryState.onlyHere.map((s) => ({ kind: 'song' as const, id: s.id }))))
const removeCopies = () => act(() => moving.clearCopies())

const n = computed(() => libraryState.onlyHere.length)
const copies = computed(() => libraryState.copies?.songs ?? 0)
</script>

<template>
  <SettingsCard v-if="libraryState.status">
    <SettingsRow v-if="libraryState.folders.length === 0" title="No music yet" description="Add a folder below: Melue looks through it for songs">
      <template #lead><StatusDot kind="off" /></template>
    </SettingsRow>
    <template v-else>
      <SettingsRow :title="plural(libraryState.status.tracks, 'song')" wide>
        <template #lead><StatusDot :kind="libraryState.scanning ? 'busy' : 'ok'" /></template>
        <template #description>
          <Transition name="swap" mode="out-in">
            <span :key="libraryState.scanning ? 'scanning' : 'counts'">{{ libraryState.scanning?.line ?? counts }}</span>
          </Transition>
        </template>
        <template v-if="!libraryState.scanning">
          <span class="text-[13px] text-muted-foreground">{{ scanned }}</span>
          <Button variant="outline" size="sm" @click="scan">Scan now</Button>
        </template>
      </SettingsRow>
      <Reveal :show="!!libraryState.scanning" :line="false">
        <div class="px-4 pb-3.5 pl-10">
          <div class="h-1 overflow-hidden rounded-full bg-well shadow-[inset_0_0_0_1px_var(--well-line)]">
            <div
              class="h-full rounded-full bg-primary transition-[width] duration-300"
              :class="libraryState.scanning?.progress == null && 'animate-pulse'"
              :style="{ width: `${Math.round((libraryState.scanning?.progress ?? 1) * 100)}%` }"
            />
          </div>
        </div>
      </Reveal>
      <Reveal :show="n > 0">
        <SettingsRow :title="`${plural(n, 'song')} only on this device`" :description="`Not on your share yet · ${size(onlyHereBytes)}`" detail>
          <template #lead><IconTile :icon="CloudUpload" /></template>
          <Button variant="outline" size="sm" @click="asking = 'move'">Move</Button>
        </SettingsRow>
      </Reveal>
      <Reveal :show="copies > 0">
        <SettingsRow :title="plural(copies, 'copy', 'copies')" :description="`Also in your synced music · ${size(libraryState.copies?.bytes ?? 0)}`" detail>
          <template #lead><IconTile :icon="Copy" /></template>
          <Button variant="outline" size="sm" @click="asking = 'copies'">Remove copies</Button>
        </SettingsRow>
      </Reveal>
    </template>
  </SettingsCard>
  <SettingsTip v-if="error || libraryState.error" kind="bad" class="mt-3">{{ error || libraryState.error }}</SettingsTip>

  <ConfirmDialog
    :open="asking === 'move'"
    :title="`Move ${plural(n, 'song')} to sync?`"
    :text="`Each song is copied into your synced music first. Then the original ${isMobile ? 'is removed: Android asks you to move them to its trash (kept 30 days).' : 'goes to the Recycle Bin.'}`"
    :action="`Move ${plural(n, 'song')}`"
    @update:open="asking = null"
    @confirm="moveAll"
  />
  <ConfirmDialog
    :open="asking === 'copies'"
    :title="`Remove ${plural(copies, 'copy', 'copies')}?`"
    :text="`These songs are also in your synced music, and Melue plays that one. ${isMobile ? 'Android asks you to move the copies here to its trash (kept 30 days).' : 'The copies here go to the Recycle Bin.'}`"
    action="Remove"
    @update:open="asking = null"
    @confirm="removeCopies"
  />
</template>
