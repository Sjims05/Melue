<script setup lang="ts">
/**
 * Playlists: how many songs the Top songs lists hold, the auto playlists you hid (Show again), and synced playlists
 * deleted in the last 30 days (Restore, on every device). Hiding and deleting say where to find these.
 */
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsLabel, SettingsRow, SettingsTip, Stepper } from '@/components/settings'
import { AUTO_PLAYLIST_SIZES, library, type AutoPlaylist } from '@/library'
import { librarySync, type DeletedPlaylist } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { when } from './format'

const auto = ref<AutoPlaylist[] | null>(null)
const deleted = ref<DeletedPlaylist[]>([])
const error = ref('')

const read = async () => {
  ;[auto.value, deleted.value] = await Promise.all([library.autoPlaylists(), librarySync.deletedPlaylists().catch(() => [])])
}
onPageLoad(() => read().catch((e) => (error.value = `Can’t read the playlists: ${e}`)))

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
      <SettingsRow description="Deleted playlists are kept for 30 days, on every device." />
    </SettingsCard>
  </template>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
