<script setup lang="ts">
/**
 * The frame of a spike test page: the sections every platform has (SMB, storage, wake lock,
 * the library, the log), with the platform's own sections in the slot between them.
 */
import {
  log,
  releaseWakeLock,
  requestWakeLock,
  smbDownloadOneFile,
  smbListFiles,
  logMsg,
} from './spike'
import { ref } from 'vue'
import { library, type MusicFolder } from '@/library'
import { isAndroid, storage } from '@/platform'

defineProps<{ title: string }>()

const folders = ref<MusicFolder[]>([])

async function refreshFolders() {
  folders.value = await library.folders()
}

async function run(label: string, action: () => Promise<unknown>) {
  try {
    await action()
  } catch (e) {
    logMsg(`${label} FAILED: ${e}`)
  }
  await refreshFolders().catch(() => {})
}

const addFolder = () =>
  run('Add folder', async () => {
    const location = await storage.pickMusicFolder()
    if (!location) return
    const folder = await library.addFolder(isAndroid ? 'android_tree' : 'path', location)
    logMsg(`Folder added: ${folder.name} (${folder.location})`)
  })

const useAllMusic = () =>
  run('All music on this phone', async () => {
    const folder = await library.addFolder('android_media', '')
    logMsg(`Folder added: ${folder.name}`)
  })

const musicPermission = () =>
  run('Music permission', async () => {
    const access = await library.musicAccess(true)
    logMsg(`Music permission: ${access.permission}${access.songs === null ? '' : `, ${access.songs} songs on the phone`}`)
  })

const toggleFolder = (folder: MusicFolder) =>
  run('Switch folder', async () => {
    const changed = await library.setFolderEnabled(folder.id, !folder.enabled)
    logMsg(`${changed.name}: ${changed.enabled ? 'on' : 'off'}`)
  })

const removeFolder = (folder: MusicFolder) =>
  run('Remove folder', async () => {
    await library.removeFolder(folder.id)
    logMsg(`Folder removed: ${folder.name}`)
  })

async function libraryStatus() {
  try {
    const status = await library.status()
    logMsg(
      `Library: schema version ${status.schemaVersion} | ${status.folders} folders, ${status.tracks} tracks, ` +
        `${status.albums} albums, ${status.artists} artists, ${status.playlists} playlists`,
    )
  } catch (e) {
    logMsg(`Library FAILED: ${e}`)
  }
}
</script>

<template>
  <div class="p-4 space-y-6 max-w-xl mx-auto">
    <h1 class="text-xl font-bold">{{ title }}</h1>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">1. SMB — connect, list, download</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="smbListFiles">List files</button>
        <button class="border rounded px-3 py-1" @click="smbDownloadOneFile">Download one file</button>
      </div>
    </section>


    <slot />

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">5. Screen wake lock</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="requestWakeLock">Request wake lock</button>
        <button class="border rounded px-3 py-1" @click="releaseWakeLock">Release wake lock</button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">6. Library</h2>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="libraryStatus">Library status</button>
        <button class="border rounded px-3 py-1" @click="run('Folders', refreshFolders)">Show folders</button>
        <button class="border rounded px-3 py-1" @click="addFolder">Add folder</button>
        <template v-if="isAndroid">
          <button class="border rounded px-3 py-1" @click="useAllMusic">Use all music on this phone</button>
          <button class="border rounded px-3 py-1" @click="musicPermission">Music permission</button>
        </template>
      </div>
      <ul class="text-sm space-y-1">
        <li v-for="folder in folders" :key="folder.id" class="flex flex-wrap items-center gap-2">
          <span :class="{ 'opacity-50': !folder.enabled }">{{ folder.name }} ({{ folder.tracks }} songs)</span>
          <span class="text-xs text-muted-foreground break-all">{{ folder.kind === 'android_media' ? '' : folder.location }}</span>
          <button class="border rounded px-2 text-xs" @click="toggleFolder(folder)">{{ folder.enabled ? 'Switch off' : 'Switch on' }}</button>
          <button class="border rounded px-2 text-xs" @click="removeFolder(folder)">Remove</button>
        </li>
      </ul>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">Log</h2>
      <div class="text-xs font-mono max-h-96 overflow-y-auto space-y-1">
        <div v-for="(line, i) in log" :key="i">{{ line }}</div>
      </div>
    </section>
  </div>
</template>
