<script setup lang="ts">
/**
 * The library at a glance, at the top of Library: how many songs, albums, artists, playlists and folders, the last
 * scan, and Scan now (while scanning: its progress). Under it, only when there's something to do, a row each with its
 * action: songs that can't be found (Show: the list), songs only on this device (Move), and hidden copies of synced
 * songs (Remove copies; per music folder when they're in more than one).
 */
import { CloudUpload, Copy, FileQuestion } from '@lucide/vue'
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { ConfirmDialog, IconTile, Reveal, SettingsCard, SettingsRow, SettingsTip, StatusDot } from '@/components/settings'
import { library } from '@/library'
import { describeSyncFailure, moving } from '@/library/sync'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import TextSwap from '@/components/page/TextSwap.vue'
import { ago, size, when, whereOriginalsGo } from './format'
import { lastScan, libraryState, refreshLibrary, startLibraryState } from './libraryState'

const asking = ref<'move' | 'copies' | null>(null)
// Remove copies: of one music folder (its id and name), or all (null).
const copiesOf = ref<{ folderId: number; name: string; songs: number } | null>(null)
const showMissing = ref(false)
const error = ref('')

onPageLoad(startLibraryState)

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
const removeCopies = () => act(() => moving.clearCopies(copiesOf.value?.folderId ?? null))
function askCopies(folder: { folderId: number; name: string; songs: number } | null) {
  copiesOf.value = folder
  asking.value = 'copies'
}
const copyFolders = computed(() => libraryState.copies?.folders ?? [])
// Removing copies from a network folder deletes them for good.
const copiesOnNetwork = computed(() => (copiesOf.value ? copyFolders.value.find((f) => f.folderId === copiesOf.value!.folderId)?.network : copyFolders.value.some((f) => f.network)) ?? false)
const removing = computed(() => copiesOf.value?.songs ?? copies.value)

const missing = computed(() => libraryState.missing.length)
// "Missing since 10:42", "since yesterday", "since 3 Oct".
const since = (ms: number) => {
  const w = when(ms)
  return w === 'Yesterday' ? 'yesterday' : w
}

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
          <TextSwap :text="libraryState.scanning?.line ?? counts" />
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
      <Reveal :show="missing > 0">
        <SettingsRow :title="`${plural(missing, 'song')} can’t be found`" description="Deleted, moved, or on a drive that isn’t connected · kept in case they come back" detail>
          <template #lead><IconTile :icon="FileQuestion" /></template>
          <Button variant="outline" size="sm" @click="showMissing = true">Show</Button>
        </SettingsRow>
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
          <Button variant="outline" size="sm" @click="askCopies(null)">{{ copyFolders.length > 1 ? 'Remove all' : 'Remove copies' }}</Button>
        </SettingsRow>
        <template v-if="copyFolders.length > 1">
          <SettingsRow v-for="f in copyFolders" :key="f.folderId" :title="f.name" :description="`${plural(f.songs, 'copy', 'copies')} · ${size(f.bytes)}`" detail>
            <Button variant="outline" size="sm" @click="askCopies(f)">Remove</Button>
          </SettingsRow>
        </template>
      </Reveal>
    </template>
  </SettingsCard>
  <Transition name="drop">
    <SettingsTip v-if="error || libraryState.error" kind="bad" class="mt-3">{{ error || libraryState.error }}</SettingsTip>
  </Transition>

  <ConfirmDialog
    :open="asking === 'move'"
    :title="`Move ${plural(n, 'song')} to sync?`"
    :text="`Each song is copied into your synced music first. Then the original ${whereOriginalsGo(isMobile, libraryState.onlyHere.some((s) => s.network), true)}`"
    :action="`Move ${plural(n, 'song')}`"
    @update:open="asking = null"
    @confirm="moveAll"
  />
  <ConfirmDialog
    :open="asking === 'copies'"
    :title="`Remove ${plural(removing, 'copy', 'copies')}${copiesOf ? ` in ${copiesOf.name}` : ''}?`"
    :text="`These songs are also in your synced music, and Melue plays that one. ${
      isMobile ? 'Android asks you to move the copies here to its trash (kept 30 days).'
      : copiesOnNetwork ? 'The copies here are deleted for good: a network folder has no Recycle Bin.'
      : 'The copies here go to the Recycle Bin.'
    }`"
    action="Remove"
    @update:open="asking = null"
    @confirm="removeCopies"
  />
  <Dialog v-model:open="showMissing">
    <DialogContent class="max-w-lg rounded-xl bg-card" @open-auto-focus.prevent>
      <DialogTitle>Songs that can’t be found</DialogTitle>
      <DialogDescription>
        Melue keeps them, with their likes, plays and places in playlists, in case they come back: put the files back or
        connect the drive, then scan.
      </DialogDescription>
      <ul class="-mx-2 max-h-[55vh] overflow-y-auto">
        <li v-for="song in libraryState.missing" :key="song.id" class="flex flex-col gap-0.5 rounded-md px-2 py-2">
          <span class="truncate text-sm font-medium">{{ song.title }}<span v-if="song.artist" class="text-muted-foreground"> · {{ song.artist }}</span></span>
          <span class="truncate text-[13px] text-muted-foreground">{{ song.folder }} · {{ song.path }}</span>
          <span class="text-xs text-muted-foreground">Missing since {{ since(song.missingSince) }}</span>
        </li>
      </ul>
    </DialogContent>
  </Dialog>
</template>
