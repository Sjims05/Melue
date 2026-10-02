<script setup lang="ts">
/**
 * The frame of a spike test page: the sections every platform has (SMB, storage, wake lock,
 * SQLite, the log), with the platform's own sections in the slot between them.
 */
import {
  log,
  pickLocalStorageFolder,
  releaseWakeLock,
  requestWakeLock,
  smbDownloadOneFile,
  smbListFiles,
  sqliteTest,
} from './spike'

defineProps<{ title: string }>()
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

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">2. Local storage picker (future feature, not wired up)</h2>
      <button class="border rounded px-3 py-1" @click="pickLocalStorageFolder">Pick storage folder</button>
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
      <h2 class="font-semibold">6. SQLite write/read</h2>
      <button class="border rounded px-3 py-1" @click="sqliteTest">Run SQLite test</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">Log</h2>
      <div class="text-xs font-mono max-h-96 overflow-y-auto space-y-1">
        <div v-for="(line, i) in log" :key="i">{{ line }}</div>
      </div>
    </section>
  </div>
</template>
