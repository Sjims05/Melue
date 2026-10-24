<script setup lang="ts">
/**
 * The questions and explanations of your playlists' menus: Delete (what happens, by how it lives), Keep only on this
 * device (the other devices keep theirs, or it's taken off them), Stop following the
 * file (the songs stay, the file is left alone), and the About items (synced, only on this device, following a file
 * or Spotify).
 */
import { computed } from 'vue'
import { ConfirmDialog } from '@/components/settings'
import type { PlaylistAsk } from './actions'

const asking = defineModel<PlaylistAsk | null>({ required: true })
const emit = defineEmits<{ confirm: [ask: PlaylistAsk, alternative: boolean] }>()

const dialog = computed(() => {
  const a = asking.value
  if (!a) return null
  const p = a.playlist
  const file = p.file ? `“${p.file}”` : 'its playlist file'
  if (a.what === 'delete') {
    const text = p.spotify
      ? p.synced && !p.localOnly
        ? 'It leaves your other devices too, and is kept in Recently deleted for 30 days. The playlist on Spotify stays as it is.'
        : 'It leaves Melue; the playlist on Spotify stays as it is.'
      : p.kind === 'linked'
        ? `It leaves Melue; ${file} stays where it is. If you changed it, it’s kept in Recently deleted for 30 days (Settings), where the file can also be followed again.`
        : p.synced && !p.localOnly
          ? 'It leaves your other devices too. It’s kept in Recently deleted for 30 days.'
          : 'Its songs stay in your library.'
    return { title: `Delete “${p.name}”?`, text, action: 'Delete', danger: true }
  }
  if (a.what === 'keepHereOnly') {
    return {
      title: `Keep “${p.name}” only on this device?`,
      choices: [
        { title: 'Keep it on your other devices', detail: 'This device gets its own local copy. Changes made here won’t sync to your other devices.' },
        { title: 'Delete it from your other devices', detail: 'Sync it again later to bring it back.' },
      ],
      action: 'Keep only here',
    }
  }
  if (a.what === 'unlink' && p.spotify) {
    return {
      title: 'Stop following Spotify?',
      text: `“${p.name}” stays as it is, as your own playlist${p.localOnly ? ' on this device' : ', on all your devices'}. Its changes on Spotify don’t come in any more.`,
      action: 'Stop following',
    }
  }
  if (p.spotify) {
    return {
      title: 'About Spotify playlists',
      text: `“${p.name}” follows a playlist on Spotify, read from its link (no sign-in): songs added or removed there show up here, matched to the songs you have; the others show greyed. It syncs to your other devices like your own playlists, with your changes. Spotify only shares the first 100 songs by itself: a longer playlist gets the rest from a CSV (Update list). Stop following Spotify keeps it as your own playlist.`,
    }
  }
  if (a.what === 'unlink') {
    return {
      title: `Stop following ${file}?`,
      text: `“${p.name}” stays as it is, with your changes, only on this device. ${file} is left alone: its changes don’t come in any more. Follow the file again (its menu) to see how it differs from the file again.`,
      action: 'Stop following',
    }
  }
  if (p.kind === 'linked' && !p.following) {
    return {
      title: 'About “Unfollowed”',
      text: `“${p.name}” came from ${file}, but doesn’t follow it any more: the file’s changes don’t come in, and it’s only on this device. Your changes are kept: Follow the file again shows how it differs from the file (songs new in it, songs you removed) once more.`,
    }
  }
  if (p.kind === 'linked') {
    return {
      title: 'About following a file',
      text: `“${p.name}” follows ${file}, a playlist file in your music folder (from another player, or kept next to your music). Songs added to or removed from the file show up here. Stop following the file keeps it as it is, with your changes, on this device; the file's changes stop coming in.`,
    }
  }
  if (p.localOnly) {
    return {
      title: 'About “This device”',
      text: `“${p.name}” is only on this device: it isn’t on your share, so your other devices don’t have it. Sync to your other devices puts it on your share.`,
    }
  }
  return {
    title: 'About syncing',
    text: `“${p.name}” is synced: it’s on your share, so your other devices with Melue have it too, and a change on any of them shows on all. Keep only on this device stops that: the other devices keep theirs, or it’s taken off them.`,
  }
})

// The second choice: taken off the other devices (Keep only on this device).
function confirm(choice: number) {
  if (asking.value) emit('confirm', asking.value, choice === 1)
}
</script>

<template>
  <ConfirmDialog
    :open="!!dialog"
    :title="dialog?.title ?? ''"
    :text="dialog?.text"
    :action="dialog?.action"
    :danger="dialog?.danger"
    :choices="dialog?.choices"
    @update:open="asking = null"
    @confirm="confirm"
  />
</template>
