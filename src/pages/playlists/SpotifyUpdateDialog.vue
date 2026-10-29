<script setup lang="ts">
/**
 * "Update list" for a Spotify playlist with more than 100 songs (Spotify only shares 100 by itself): its link is
 * copied and Chosic's exporter opens in the browser (no sign-in); paste it there and download the CSV. On a computer
 * Melue picks the download up from the Downloads folder by itself; on a phone (or any time) choose the file.
 */
import { open as openFile } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { LoaderCircle } from '@lucide/vue'
import { onBeforeUnmount, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { library, type Playlist } from '@/library'
import { isDesktop } from '@/platform'

const props = defineProps<{ playlist: Playlist }>()
const open = defineModel<boolean>('open', { required: true })

/** The exporter: no sign-in for a public playlist, up to 1,000 songs, CSV. */
const EXPORTER = 'https://www.chosic.com/spotify-playlist-exporter/'

const waiting = ref(false)
const error = ref('')
const link = () => `https://open.spotify.com/playlist/${props.playlist.spotify?.id}`

// Opened (also: shown open, from a playlist's menu): the link copied, and (on a computer) Melue watches Downloads for
// the CSV.
watch(open, async (now) => {
  error.value = ''
  if (!now) return void stopWaiting()
  await navigator.clipboard.writeText(link()).catch(() => {})
  if (!isDesktop) return
  waiting.value = true
  const done = await library.waitForSpotifyCsv(props.playlist.id).catch((e) => ((error.value = String(e)), false))
  waiting.value = false
  if (done) open.value = false
}, { immediate: true })
function stopWaiting() {
  if (waiting.value) library.stopWaitingForSpotifyCsv().catch(() => {})
  waiting.value = false
}
onBeforeUnmount(stopWaiting)

const openExporter = () => openUrl(EXPORTER).catch(() => {})

/** Choosing the CSV by hand (a phone, or a file saved elsewhere). */
async function choose() {
  error.value = ''
  try {
    if (isDesktop) {
      const path = await openFile({ filters: [{ name: 'CSV', extensions: ['csv'] }] })
      if (typeof path !== 'string') return
      await library.importSpotifyCsvFile(props.playlist.id, path)
    } else {
      const { readTextFile, showOpenFilePicker } = await import('tauri-plugin-android-fs-api')
      const [file] = await showOpenFilePicker({ mimeTypes: ['text/csv', 'text/comma-separated-values', 'text/plain', 'application/octet-stream'] })
      if (!file) return
      await library.importSpotifyCsv(props.playlist.id, await readTextFile(file))
    }
    open.value = false
  } catch (e) {
    error.value = String(e)
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false" class="max-w-[440px] gap-3 rounded-xl bg-card p-4 sm:max-w-[440px]">
      <DialogTitle class="text-[15px] font-semibold">Update “{{ playlist.name }}”</DialogTitle>
      <DialogDescription class="text-[13.5px] leading-[1.45]">
        Spotify only shares the first 100 of its {{ playlist.spotify?.total ?? 'songs' }}{{ playlist.spotify?.total ? ' songs' : '' }}. For all of
        them: its link is copied, so on Chosic paste it and download the CSV.
        {{ isDesktop ? 'Melue picks the download up by itself.' : 'Then choose the file here.' }}
      </DialogDescription>
      <p v-if="waiting" class="flex items-center gap-2 text-[13px] text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />Waiting for the CSV in your Downloads…</p>
      <p v-if="error" class="text-[13px] text-destructive">{{ error }}</p>
      <div class="flex flex-wrap justify-end gap-2">
        <Button variant="outline" size="sm" @click="open = false">Cancel</Button>
        <Button variant="outline" size="sm" @click="choose">Choose the CSV…</Button>
        <Button size="sm" @click="openExporter">Open Chosic</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
