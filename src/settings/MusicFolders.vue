<script setup lang="ts">
/**
 * The folders Melue plays from (this device's). A folder with songs not on the share opens to list them (those on all
 * your devices marked): its tile turns into a "select all" checkbox, a tap on a song ticks it, and the folder's last
 * row moves the ticked songs to sync. The ⋯ menu has the folder's actions: Move all to sync, Switch off / on, Rename
 * (opens in place), Remove.
 *
 * Copies of synced songs are handled on the status card above (Melue counts them for the whole library).
 */
import { Cloud, CloudUpload, Ellipsis, FilePlus, Folder, Plus, Smartphone } from '@lucide/vue'
import { computed, reactive, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Chevron, ConfirmDialog, EditRow, IconTile, Reveal, SettingsBadge, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library, type MusicFolder } from '@/library'
import { describeSyncFailure, moving, sync, type LocalSong } from '@/library/sync'
import { isAndroid, isDesktop, isMobile, storage } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { adding, pickSongs } from './addSongs'
import { size, whereOriginalsGo } from './format'
import { libraryState, onlyHereSongs, refreshLibrary, startLibraryState } from './libraryState'

const open = ref<number | null>(null)
const renaming = ref<number | null>(null)
const picked = reactive(new Set<number>())
const asking = ref<{ what: 'move'; songs: LocalSong[] } | { what: 'remove'; folder: MusicFolder } | null>(null)
const error = ref('')

onPageLoad(startLibraryState)

// Your folders first (by name), the synced one last.
const folders = computed(() => [...libraryState.folders].sort((a, b) => Number(a.synced) - Number(b.synced) || a.name.localeCompare(b.name)))
const shareLike = computed(() => folders.value.filter((f) => libraryState.shareFolders.has(f.id)))
// A folder's songs that aren't on the share (the synced folder's too: songs put there by hand): only on this device,
// and those all your devices have (offered for the share, marked "All devices").
const offeredIds = computed(() => new Set(libraryState.offered.map((s) => s.id)))
const notOnShareIn = (folder: MusicFolder) => libraryState.onlyHere.filter((s) => s.folder === folder.name)
const onlyHereIn = (folder: MusicFolder) => onlyHereSongs.value.filter((s) => s.folder === folder.name)
const onAllIn = (folder: MusicFolder) => notOnShareIn(folder).filter((s) => offeredIds.value.has(s.id))
const plural = (n: number, one: string) => `${n.toLocaleString()} ${one}${n === 1 ? '' : 's'}`

// A folder that is the share (or looks like it): its songs are in the library already, as your synced music.
const SHARE_LINE: Record<string, string> = {
  share: 'This is your share: its songs are already in your library',
  insideShare: 'This is inside your share: its songs are already in your library',
  looksLikeShare: 'This looks like your share: its songs are already in your library',
}
function line(folder: MusicFolder) {
  const share = libraryState.shareFolders.get(folder.id)
  if (share) return SHARE_LINE[share]
  if (!folder.enabled) return 'Switched off: its songs are hidden'
  const where = folder.kind === 'android_media' ? ' · all music on this phone' : folder.synced ? ' · your share, copied here' : folder.network ? ' · on the network' : ''
  const [here, onAll] = [onlyHereIn(folder).length, onAllIn(folder).length]
  return `${plural(folder.tracks, 'song')}${where}${here ? ` · ${here} only here` : ''}${onAll ? ` · ${onAll} on all your devices` : ''}`
}
const tile = (folder: MusicFolder) => (folder.synced ? Cloud : folder.kind === 'android_media' ? Smartphone : Folder)

// Opening a folder: only one with songs to move.
const canOpen = (folder: MusicFolder) => sync.available && folder.enabled && notOnShareIn(folder).length > 0
function toggle(folder: MusicFolder, event: Event) {
  if (!canOpen(folder) || (event.target as HTMLElement).closest('button, [role=checkbox]')) return
  open.value = open.value === folder.id ? null : folder.id
  picked.clear()
}

const openSongs = computed(() => {
  const folder = folders.value.find((f) => f.id === open.value)
  return folder ? notOnShareIn(folder) : []
})
const all = computed<boolean | 'indeterminate'>(() => {
  const n = openSongs.value.filter((s) => picked.has(s.id)).length
  return n === 0 ? false : n === openSongs.value.length ? true : 'indeterminate'
})
function pickAll() {
  const on = all.value !== true
  for (const s of openSongs.value) {
    if (on) picked.add(s.id)
    else picked.delete(s.id)
  }
}
const pick = (song: LocalSong, on: boolean | 'indeterminate') => (on === true ? picked.add(song.id) : picked.delete(song.id))
const pickedSongs = computed(() => openSongs.value.filter((s) => picked.has(s.id)))
const bytes = (songs: LocalSong[]) => songs.reduce((n, s) => n + s.size, 0)

async function act(action: () => Promise<unknown>) {
  error.value = ''
  try {
    await action()
  } catch (e) {
    error.value = describeSyncFailure(e)
  }
  await refreshLibrary()
}

const addFolder = () =>
  act(async () => {
    const location = await storage.pickMusicFolder()
    if (location) await library.addFolder(isAndroid ? 'android_tree' : 'path', location)
  })
const move = (songs: LocalSong[]) =>
  act(async () => {
    await moving.moveToSync(songs.map((s) => ({ kind: 'song' as const, id: s.id })))
    picked.clear()
    open.value = null
  })
const rename = (folder: MusicFolder, name: string) =>
  act(async () => {
    await library.renameFolder(folder.id, name.trim() || folder.name)
    renaming.value = null
  })
const switchFolder = (folder: MusicFolder) => act(() => library.setFolderEnabled(folder.id, !folder.enabled))
const remove = (folder: MusicFolder) => act(() => library.removeFolder(folder.id))

const ask = computed(() => {
  const a = asking.value
  if (a?.what === 'move') {
    const n = plural(a.songs.length, 'song')
    return {
      title: `Move ${n} to sync?`,
      text: `Each song is copied into your synced music first. Then the original ${whereOriginalsGo(isMobile, a.songs.some((s) => s.network), true)}`,
      action: `Move ${n}`,
      danger: false,
    }
  }
  if (a?.what === 'remove') {
    return { title: `Remove ${a.folder.name}?`, text: 'Its songs leave your library on this device. The files stay where they are.', action: 'Remove', danger: true }
  }
  return null
})
function confirm() {
  const a = asking.value
  if (a?.what === 'move') move(a.songs)
  else if (a?.what === 'remove') remove(a.folder)
}
</script>

<template>
  <SettingsLabel mark="This device">Music folders</SettingsLabel>
  <SettingsCard>
    <template v-for="folder in folders" :key="folder.id">
      <SettingsRow :description="line(folder)" :dim="!folder.enabled" @click="toggle(folder, $event)">
        <template #lead>
          <!-- In the tile's place, so the name doesn't move when the folder opens. -->
          <span v-if="open === folder.id" class="grid size-[34px] flex-none place-items-center">
            <Checkbox :model-value="all" aria-label="Select all" @update:model-value="pickAll" />
          </span>
          <IconTile v-else :icon="tile(folder)" />
        </template>
        <template #title>
          {{ folder.name }}
          <SettingsBadge v-if="folder.synced" class="ml-1.5">Synced</SettingsBadge>
          <SettingsBadge v-if="!folder.enabled" class="ml-1.5">Off</SettingsBadge>
        </template>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="icon-sm" :aria-label="`${folder.name}: more`"><Ellipsis /></Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem v-if="sync.available && folder.enabled && onlyHereIn(folder).length" @select="asking = { what: 'move', songs: onlyHereIn(folder) }">
              Move all to sync
            </DropdownMenuItem>
            <DropdownMenuItem v-if="!folder.synced" @select="switchFolder(folder)">{{ folder.enabled ? 'Switch off' : 'Switch on' }}</DropdownMenuItem>
            <DropdownMenuItem @select="renaming = renaming === folder.id ? null : folder.id">Rename</DropdownMenuItem>
            <DropdownMenuItem v-if="!folder.synced" variant="destructive" @select="asking = { what: 'remove', folder }">Remove…</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <template v-if="canOpen(folder)" #end><Chevron :open="open === folder.id" /></template>
      </SettingsRow>
      <Reveal :show="renaming === folder.id" :line="false">
        <EditRow class="pt-0" :value="folder.name" @save="(name) => rename(folder, name)" @cancel="renaming = null" />
      </Reveal>
      <Reveal :show="open === folder.id">
        <!-- A tap anywhere on a song ticks it (the folder's row opens and closes instead). -->
        <SettingsRow
          v-for="song in openSongs"
          :key="song.id"
          :description="song.artist ?? undefined"
          detail
          class="cursor-pointer"
          @click="(e: MouseEvent) => !(e.target as HTMLElement).closest('[role=checkbox]') && pick(song, !picked.has(song.id))"
        >
          <template #title>
            {{ song.title }}
            <SettingsBadge v-if="offeredIds.has(song.id)" class="ml-1.5">All devices</SettingsBadge>
          </template>
          <template #lead>
            <span class="grid size-[34px] flex-none place-items-center">
              <Checkbox :model-value="picked.has(song.id)" :aria-label="song.title" @update:model-value="(on) => pick(song, on)" />
            </span>
          </template>
          <span class="text-[13px] text-muted-foreground">{{ size(song.size) }}</span>
        </SettingsRow>
        <!-- What's ticked, to move: the open folder's last row, so it opens and closes with its songs. -->
        <SettingsRow :title="`${plural(pickedSongs.length, 'song')} to move · ${size(bytes(pickedSongs))}`" wide>
          <template #lead><IconTile :icon="CloudUpload" /></template>
          <Button size="sm" :disabled="pickedSongs.length === 0" @click="asking = { what: 'move', songs: pickedSongs }">
            Move {{ plural(pickedSongs.length, 'song') }} to sync
          </Button>
        </SettingsRow>
      </Reveal>
    </template>

    <SettingsRow as="button" title="Add folder" @click="addFolder">
      <template #lead><IconTile :icon="Plus" /></template>
    </SettingsRow>
    <!-- Desktop, with a share: songs from anywhere (or dropped on Melue: DropSongs.vue), copied to the share. -->
    <SettingsRow
      v-if="isDesktop && sync.available"
      as="button"
      title="Add songs…"
      description="From anywhere, to your synced music: the files you pick stay where they are"
      @click="pickSongs"
    >
      <template #lead><IconTile :icon="FilePlus" /></template>
    </SettingsRow>
  </SettingsCard>
  <SettingsTip v-for="folder in shareLike" :key="folder.id" class="mt-3">
    {{ folder.name }} {{ libraryState.shareFolders.get(folder.id) === 'looksLikeShare' ? 'looks like your share' : 'is your share' }}: Melue
    never removes files from it. Take it out of your folders? The files stay.
    <template #action>
      <Button size="sm" variant="secondary" @click="asking = { what: 'remove', folder }">Remove from your folders</Button>
    </template>
  </SettingsTip>
  <Transition name="drop">
    <SettingsTip v-if="adding.result && !adding.error" kind="ok" class="mt-3">Added songs: {{ adding.result }}</SettingsTip>
  </Transition>
  <Transition name="drop">
    <SettingsTip v-if="error || adding.error" kind="bad" class="mt-3">{{ error || adding.error }}</SettingsTip>
  </Transition>

  <ConfirmDialog
    v-if="ask"
    :open="!!asking"
    :title="ask.title"
    :text="ask.text"
    :action="ask.action"
    :danger="ask.danger"
    @update:open="asking = null"
    @confirm="confirm"
  />
</template>
