<script setup lang="ts">
/**
 * The library at a glance, at the top of Library: how many songs, albums, artists, playlists and folders, the last
 * scan, and Scan now (while scanning: its progress). Under it, only when there's something to do, a row each: songs
 * that can't be found, songs only on this device, songs on all your devices (not on the share), and hidden copies of
 * synced songs (Remove copies; per music folder when they're in more than one). A row with songs opens their list:
 * tick some (or all), and its action at the bottom does those: Forget, Move to sync, Put on share.
 */
import { CloudUpload, Copy, Ellipsis, FileQuestion, MonitorSmartphone } from '@lucide/vue'
import { computed, reactive, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Chevron, ConfirmDialog, IconTile, Reveal, SettingsCard, SettingsRow, SettingsTip, StatusDot } from '@/components/settings'
import { library } from '@/library'
import { describeSyncFailure, moving, syncing } from '@/library/sync'
import { loadSyncSettings, saveSyncSettings, syncSettings } from './syncSettings'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import TextSwap from '@/components/page/TextSwap.vue'
import { ago, readablePath, size, when, whereOriginalsGo } from './format'
import { lastScan, libraryState, onlyHereSongs, refreshLibrary, startLibraryState } from './libraryState'

const asking = ref<'move' | 'copies' | 'forget' | null>(null)
// Remove copies: of one music folder (its id and name), or all (null).
const copiesOf = ref<{ folderId: number; name: string; songs: number } | null>(null)
const error = ref('')

onPageLoad(startLibraryState)

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`
const counts = computed(() => {
  const s = libraryState.status
  if (!s) return ''
  return [plural(s.albums, 'album'), plural(s.artists, 'artist'), plural(s.playlists, 'playlist'), `in ${plural(s.folders, 'folder')}`].join(' · ')
})
// Songs all your devices have, each its own copy, but not on the share: put them there? (Songs on all your devices,
// Ask: sync/to_share.rs.) They're left out of "only on this device" (onlyHereSongs).
loadSyncSettings().catch(() => {})
const onAll = computed(() => ((syncSettings.value?.songsOnMoreDevices ?? 'ask') === 'ask' ? libraryState.offered.filter((s) => !s.approved) : []))
const onAllDevices = computed(() => [...new Set(onAll.value.flatMap((s) => s.devices))].join(' and '))
const onlyHereBytes = computed(() => onlyHereSongs.value.reduce((n, s) => n + s.size, 0))
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
const n = computed(() => onlyHereSongs.value.length)
const copies = computed(() => libraryState.copies?.songs ?? 0)

// ---- A row's list: its songs, ticked to do its action with.

type ListKind = 'missing' | 'onlyHere' | 'onAll'
const listOpen = ref<ListKind | null>(null)
const picked = reactive(new Set<number>())
function openList(kind: ListKind) {
  picked.clear()
  listOpen.value = kind
}
const list = computed(() => {
  switch (listOpen.value) {
    case 'missing':
      return {
        title: 'Songs that can’t be found',
        text: 'Melue keeps them, with their likes, plays and places in playlists, in case they come back: put the files back or connect the drive, then scan. Or forget them.',
        songs: libraryState.missing.map((s) => ({ id: s.id, title: s.title, artist: s.artist, lines: [`${s.folder} · ${readablePath(s.path)}`, `Missing since ${since(s.missingSince)}`] })),
        action: (k: number) => `Forget ${k ? plural(k, 'song') : 'songs'}…`,
      }
    case 'onAll':
      return {
        title: 'Songs on all your devices',
        text: 'Each of your devices has its own copy, but they aren’t on your share. Put on share puts the best copy there (the device that has it does), and the others use that one.',
        songs: onAll.value.map((s) => ({ id: s.id, title: s.title, artist: s.artist, lines: [s.bestHere ? 'The best copy is here' : `The best copy is on ${s.devices.join(' or ')}`] })),
        action: (k: number) => `Put ${k ? plural(k, 'song') : 'songs'} on share`,
      }
    default:
      return {
        title: 'Songs only on this device',
        text: 'Your other devices haven’t got them: they aren’t on your share. Move puts them there.',
        songs: onlyHereSongs.value.map((s) => ({ id: s.id, title: s.title, artist: s.artist, lines: [`${s.folder} · ${size(s.size)}`] })),
        action: (k: number) => `Move ${k ? plural(k, 'song') : 'songs'}…`,
      }
  }
})
const allPicked = computed<boolean | 'indeterminate'>(() => (picked.size === 0 ? false : picked.size === list.value.songs.length ? true : 'indeterminate'))
function pickAll() {
  const on = allPicked.value !== true
  picked.clear()
  if (on) list.value.songs.forEach((s) => picked.add(s.id))
}
const tick = (id: number) => (picked.has(id) ? picked.delete(id) : picked.add(id))
// The list's action, on the ticked songs: Forget and Move ask first; Put on share just goes.
function doAction() {
  const kind = listOpen.value
  listOpen.value = null
  if (kind === 'missing') asking.value = 'forget'
  else if (kind === 'onlyHere') asking.value = 'move'
  else if (kind === 'onAll') act(() => syncing.putOnShare([...picked]))
}
const pickedOnlyHere = computed(() => onlyHereSongs.value.filter((s) => picked.has(s.id)))
const moveThem = () => act(() => moving.moveToSync(pickedOnlyHere.value.map((s) => ({ kind: 'song' as const, id: s.id }))))
const forgetThem = () => act(() => library.forgetMissing([...picked]))
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
      <!-- A row with songs opens their list (the › says so): what to do with them is done there. -->
      <Reveal :show="missing > 0">
        <SettingsRow
          :title="`${plural(missing, 'song')} can’t be found`"
          description="Deleted, moved, or on a drive that isn’t connected · kept in case they come back"
          detail
          class="cursor-pointer"
          @click="openList('missing')"
        >
          <template #lead><IconTile :icon="FileQuestion" /></template>
          <template #end><Chevron :open="false" points="right" /></template>
        </SettingsRow>
      </Reveal>
      <Reveal :show="n > 0">
        <SettingsRow
          :title="`${plural(n, 'song')} only on this device`"
          :description="`Not on your share yet · ${size(onlyHereBytes)}`"
          detail
          class="cursor-pointer"
          @click="openList('onlyHere')"
        >
          <template #lead><IconTile :icon="CloudUpload" /></template>
          <template #end><Chevron :open="false" points="right" /></template>
        </SettingsRow>
      </Reveal>
      <Reveal :show="onAll.length > 0">
        <SettingsRow
          :title="`${plural(onAll.length, 'song')} on all your devices`"
          :description="`Also on ${onAllDevices} · not on your share yet`"
          detail
          class="cursor-pointer"
          @click="openList('onAll')"
        >
          <template #lead><IconTile :icon="MonitorSmartphone" /></template>
          <template #end><Chevron :open="false" points="right" /></template>
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

  <!-- A row's songs: tick some (or all), then its action. -->
  <Dialog :open="!!listOpen" @update:open="(open) => !open && (listOpen = null)">
    <DialogContent class="max-w-lg rounded-xl bg-card" @open-auto-focus.prevent>
      <DialogTitle>{{ list.title }}</DialogTitle>
      <DialogDescription>{{ list.text }}</DialogDescription>
      <ul class="-mx-2 max-h-[55vh] overflow-y-auto">
        <li class="flex items-center gap-3 rounded-md px-2 py-1.5">
          <Checkbox :model-value="allPicked" aria-label="Select all" @update:model-value="pickAll" />
          <span class="text-[13px] text-muted-foreground">Select all</span>
        </li>
        <!-- A tap anywhere on a song ticks it. -->
        <li
          v-for="song in list.songs"
          :key="song.id"
          class="flex cursor-pointer items-start gap-3 rounded-md px-2 py-2 hover:bg-muted/40"
          @click="(e: MouseEvent) => !(e.target as HTMLElement).closest('[role=checkbox]') && tick(song.id)"
        >
          <Checkbox class="mt-0.5" :model-value="picked.has(song.id)" :aria-label="song.title" @update:model-value="tick(song.id)" />
          <span class="flex min-w-0 flex-col gap-0.5">
            <span class="truncate text-sm font-medium">{{ song.title }}<span v-if="song.artist" class="text-muted-foreground"> · {{ song.artist }}</span></span>
            <span v-for="(line, i) in song.lines" :key="i" class="truncate text-[13px] text-muted-foreground">{{ line }}</span>
          </span>
        </li>
      </ul>
      <div class="flex items-center justify-end gap-1">
        <!-- Songs on all your devices: the setting's other choices. -->
        <DropdownMenu v-if="listOpen === 'onAll'">
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="icon-sm" aria-label="More"><Ellipsis /></Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem @select="saveSyncSettings({ songsOnMoreDevices: 'auto' }); listOpen = null">Do this automatically</DropdownMenuItem>
            <DropdownMenuItem @select="saveSyncSettings({ songsOnMoreDevices: 'off' }); listOpen = null">Never ask again</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button variant="outline" size="sm" :disabled="picked.size === 0" @click="doAction">{{ list.action(picked.size) }}</Button>
      </div>
    </DialogContent>
  </Dialog>

  <ConfirmDialog
    :open="asking === 'move'"
    :title="`Move ${plural(pickedOnlyHere.length, 'song')} to sync?`"
    :text="`Each song is copied into your synced music first. Then the original ${whereOriginalsGo(isMobile, pickedOnlyHere.some((s) => s.network), true)}`"
    :action="`Move ${plural(pickedOnlyHere.length, 'song')}`"
    @update:open="asking = null"
    @confirm="moveThem"
  />
  <ConfirmDialog
    :open="asking === 'forget'"
    :title="`Forget ${plural(picked.size, 'song')} that can’t be found?`"
    text="They leave your library, with their likes, plays and places in playlists. If the files come back, a scan finds them as new songs."
    action="Forget"
    danger
    @update:open="asking = null"
    @confirm="forgetThem"
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
</template>
