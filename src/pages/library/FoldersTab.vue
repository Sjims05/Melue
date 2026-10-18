<script setup lang="ts">
/**
 * Library's Folders: a row per music folder (its name, "From the share" on the synced one, where it is and how many
 * songs). A row opens the folder; its right-click menu plays it, shuffles it, plays it next or adds it to the queue. A
 * switched-off folder says so (its songs are hidden until it's on again, in Settings). It follows the library.
 */
import { Cloud } from '@lucide/vue'
import { onUnmounted, ref } from 'vue'
import { coverMenu, type CoverAction } from '@/components/covers'
import { LinkRow } from '@/components/page'
import { SettingsBadge } from '@/components/settings'
import { library, type MusicFolder } from '@/library'
import { count } from '@/lib/format'
import { listAction } from '@/player/songs'
import { folderIcon, folderPlace } from './folders'

const folders = ref<MusicFolder[] | null>(null)
async function load() {
  folders.value = await library.folders().catch(() => [])
}
load()
const stop = library.onChange((change) => change.reason !== 'likes' && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const MENU = coverMenu({ kind: 'list' })
const line = (f: MusicFolder) => [folderPlace(f), f.enabled ? count(f.tracks, 'song') : 'Switched off'].join(' · ')
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <div v-if="folders?.length" role="list">
      <LinkRow
        v-for="folder in folders"
        :key="folder.id"
        role="listitem"
        :to="`/folder/${folder.id}`"
        :title="folder.name"
        :subtitle="line(folder)"
        :icon="folderIcon(folder)"
        :menu="folder.enabled ? MENU : undefined"
        :class="!folder.enabled && 'opacity-50'"
        @action="(action) => listAction(`folder:${folder.id}`, action as CoverAction)"
      >
        <template v-if="folder.synced" #badges><SettingsBadge class="gap-1"><Cloud class="size-3" />From the share</SettingsBadge></template>
      </LinkRow>
    </div>
    <p v-else-if="folders" class="px-2 text-muted-foreground">No music folders yet: add one in Settings, and its songs show up here.</p>
  </div>
</template>
