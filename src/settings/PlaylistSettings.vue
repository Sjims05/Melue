<script setup lang="ts">
/**
 * Playlists: how many songs the Top songs lists hold, the auto playlists you hid (Show again), synced playlists
 * deleted in the last 30 days (Restore, on every device), and playlist files you let go of (Follow again: linked as a
 * new playlist). Hiding, deleting and stopping following say where to find these.
 */
import { computed, onActivated, onUnmounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsLabel, SettingsRow, SettingsTip, Stepper } from '@/components/settings'
import { AUTO_PLAYLIST_SIZES, library, type AutoPlaylist, type DeletedLinkedPlaylist, type LetGoFile } from '@/library'
import { librarySync, type DeletedPlaylist } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { when } from './format'

const auto = ref<AutoPlaylist[] | null>(null)
const deleted = ref<DeletedPlaylist[]>([])
const letGo = ref<LetGoFile[]>([])
const deletedLinked = ref<DeletedLinkedPlaylist[]>([])
const error = ref('')

const read = async () => {
  ;[auto.value, deleted.value, letGo.value, deletedLinked.value] = await Promise.all([
    library.autoPlaylists(),
    librarySync.deletedPlaylists().catch(() => []),
    library.letGoFiles().catch(() => []),
    library.deletedLinkedPlaylists().catch(() => []),
  ])
}
onPageLoad(() => read().catch((e) => (error.value = `Can’t read the playlists: ${e}`)))
// Kept up to date: a playlist deleted or let go of elsewhere shows here at once.
onActivated(() => read().catch(() => {}))
const stop = library.onChange((change) => change.reason === 'playlists' && read().catch(() => {}))
onUnmounted(() => stop.then((unlisten) => unlisten()))

async function act(action: () => Promise<unknown>) {
  error.value = ''
  try {
    await action()
    await read()
  } catch (e) {
    error.value = String(e)
  }
}

const top = computed(() => auto.value?.filter((p) => p.kind === 'top') ?? [])
const hidden = computed(() => auto.value?.filter((p) => p.hidden) ?? [])
// One size for all the Top songs lists.
const topSize = computed(() => top.value[0]?.size ?? 50)
const setTopSize = (size: number) => act(() => Promise.all(top.value.map((p) => library.setAutoPlaylist(p.source, { size }))))
</script>

<template>
  <template v-if="auto">
    <template v-if="top.length">
      <SettingsLabel>Auto playlists</SettingsLabel>
      <SettingsCard>
        <SettingsRow
          title="Top songs lists hold"
          setting="playlists.topSize"
          :description="top.map((p) => p.name).join(', ')"
          :reset="() => setTopSize(50)"
          :is-default="topSize === 50"
        >
          <Stepper :model-value="topSize" :steps="[...AUTO_PLAYLIST_SIZES]" @update:model-value="setTopSize" />
        </SettingsRow>
      </SettingsCard>
    </template>

    <SettingsLabel>Hidden auto playlists</SettingsLabel>
    <SettingsCard>
      <SettingsRow v-for="p in hidden" :key="p.source" :title="p.name" :description="`${p.songs.toLocaleString()} songs`">
        <Button variant="outline" size="sm" @click="act(() => library.setAutoPlaylist(p.source, { hidden: false }))">Show again</Button>
      </SettingsRow>
      <SettingsRow v-if="hidden.length === 0" description="None hidden" />
    </SettingsCard>

    <SettingsLabel>Recently deleted</SettingsLabel>
    <SettingsCard>
      <SettingsRow v-for="p in deleted" :key="p.syncId" :title="p.name" :description="`${p.songs.toLocaleString()} songs · deleted ${when(p.deletedAt)}`">
        <Button variant="outline" size="sm" @click="act(() => librarySync.restorePlaylist(p.syncId))">Restore</Button>
      </SettingsRow>
      <SettingsRow
        v-for="p in deletedLinked"
        :key="`linked:${p.id}`"
        :title="p.name"
        :description="`${p.songs.toLocaleString()} songs · from ${p.file}, with your changes · deleted ${when(p.deletedAt)}`"
      >
        <Button variant="outline" size="sm" @click="act(() => library.restoreLinkedPlaylist(p.id))">Restore</Button>
      </SettingsRow>
      <SettingsRow description="Deleted playlists are kept for 30 days: synced ones on every device, a playlist file’s with your changes on this one." />
    </SettingsCard>

    <template v-if="letGo.length">
      <SettingsLabel>Playlist files you let go of</SettingsLabel>
      <SettingsCard>
        <SettingsRow v-for="f in letGo" :key="`${f.folderId}:${f.path}`" :title="f.path.split(/[\\/]/).pop() ?? f.path" :description="`${f.folder} · ${f.path}`">
          <Button variant="outline" size="sm" @click="act(() => library.followFileAgain(f.folderId, f.path))">Follow again</Button>
        </SettingsRow>
        <SettingsRow description="Files you stopped following, or whose playlist you deleted. Following one again makes it a new playlist." />
      </SettingsCard>
    </template>
  </template>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
