<script setup lang="ts">
/**
 * Library's Folders: a row per music folder (its name, "From the share" on the synced one, where it is and how many
 * songs). A row opens the folder; its right-click menu plays it, shuffles it, plays it next or adds it to the queue. A
 * switched-off folder says so (its songs are hidden until it's on again, in Settings). It follows the library.
 */
import { CircleAlert, Cloud, Folder } from '@lucide/vue'
import { ref } from 'vue'
import { coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, LinkRow } from '@/components/page'
import { SettingsBadge } from '@/components/settings'
import { library, type MusicFolder } from '@/library'
import { count } from '@/lib/format'
import { listAction } from '@/player/songs'
import { folderIcon, folderPlace } from './folders'
import { useRouter } from 'vue-router'
import { Button } from '@/components/ui/button'
import { changesTheLibrary, onLibraryChange } from '@/library/changes'

const folders = ref<MusicFolder[] | null>(null)
// Why the list couldn't be read (shown instead of an empty list).
const problem = ref('')
const router = useRouter()
async function load() {
  problem.value = ''
  folders.value = await library.folders().catch((e) => ((problem.value = String(e)), []))
}
load()
onLibraryChange(changesTheLibrary, load)

const MENU = coverMenu({ kind: 'list' })
const line = (f: MusicFolder) => [folderPlace(f), f.enabled ? count(f.tracks, 'song') : 'Switched off'].join(' · ')
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
    <Transition name="part" mode="out-in">
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
      <!-- Can't be read (Try again), or nothing yet (where to add music). -->
      <EmptyState v-else-if="problem" :icon="CircleAlert" title="Can’t read your library" :text="problem" problem>
        <Button size="sm" variant="outline" @click="load">Try again</Button>
      </EmptyState>
      <EmptyState v-else-if="folders" :icon="Folder" title="No music folders yet" text="Add one, and its songs show up in your library.">
        <Button size="sm" @click="router.push('/settings')">Add a music folder</Button>
      </EmptyState>
    </Transition>
  </div>
</template>
