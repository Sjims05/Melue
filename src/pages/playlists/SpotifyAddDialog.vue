<script setup lang="ts">
/**
 * "Add from Spotify": a Spotify playlist's link (Share → Copy link on Spotify; a public playlist) becomes a playlist
 * here that follows it (src-tauri library/spotify.rs). Emits the new playlist's id.
 */
import { ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Field } from '@/components/settings'
import { library } from '@/library'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ added: [id: number] }>()

const link = ref('')
const error = ref('')
const adding = ref(false)
watch(open, (now) => now && ((link.value = ''), (error.value = '')))

async function add() {
  error.value = ''
  adding.value = true
  try {
    const id = await library.addSpotifyPlaylist(link.value)
    open.value = false
    emit('added', id)
  } catch (e) {
    error.value = String(e)
  } finally {
    adding.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false" class="max-w-[420px] gap-3 rounded-xl bg-card p-4 sm:max-w-[420px]">
      <DialogTitle class="text-[15px] font-semibold">Add a playlist from Spotify</DialogTitle>
      <DialogDescription class="text-[13.5px] leading-[1.45]">
        On Spotify: the playlist’s ⋯ → Share → Copy link (a public playlist). Melue keeps it up to date, with the songs
        you have; the others show greyed.
      </DialogDescription>
      <form class="grid gap-3" @submit.prevent="add">
        <Field v-model="link" label="Link" placeholder="https://open.spotify.com/playlist/…" :error="error" />
        <div class="flex justify-end gap-2">
          <Button type="button" variant="outline" size="sm" @click="open = false">Cancel</Button>
          <Button type="submit" size="sm" :disabled="!link.trim() || adding">{{ adding ? 'Adding…' : 'Add' }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
