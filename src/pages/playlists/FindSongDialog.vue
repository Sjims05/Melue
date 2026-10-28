<script setup lang="ts">
/**
 * Find song: one of your songs for a playlist's song that isn't here (a Spotify playlist's song the automatic match
 * left out, like a longer version you have). Searches the library, starting with its title; the one picked is used
 * for it from then on, kept when the playlist updates and on your other devices (src-tauri playlists.rs
 * choose_playlist_song). Emits once it's chosen.
 */
import { refDebounced } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Field } from '@/components/settings'
import { library, type Track } from '@/library'

const props = defineProps<{ playlist: number; entry: number | null; title: string; artist: string }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ chosen: [] }>()

const query = ref('')
const typed = refDebounced(query, 200)
const found = ref<Track[]>([])
const error = ref('')
const choosing = ref(false)

watch(open, (now) => {
  if (!now) return
  query.value = props.title
  error.value = ''
})
watch(
  [typed, open],
  async ([text, now]) => {
    if (!now) return
    const q = text.trim()
    found.value = q ? (await library.search(q, 30).catch(() => ({ tracks: [] as Track[] }))).tracks : []
  },
  { immediate: true },
)
// Its artist's songs first (the one you have in another version, say).
const ranked = computed(() => {
  const artist = props.artist.toLowerCase()
  const theirs = (t: Track) => !!artist && (t.artist ?? '').toLowerCase().includes(artist.split(/[,/&]/)[0].trim())
  return [...found.value.filter(theirs), ...found.value.filter((t) => !theirs(t))].slice(0, 12)
})
const time = (ms: number | null) => (ms ? `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, '0')}` : '')

async function choose(track: Track) {
  if (props.entry === null) return
  error.value = ''
  choosing.value = true
  try {
    await library.choosePlaylistSong(props.playlist, props.entry, track.id)
    open.value = false
    emit('chosen')
  } catch (e) {
    error.value = String(e)
  } finally {
    choosing.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false" class="max-w-[440px] gap-3 rounded-xl bg-card p-4 sm:max-w-[440px]">
      <DialogTitle class="text-[15px] font-semibold">Find song</DialogTitle>
      <DialogDescription class="text-[13.5px] leading-[1.45]">
        Which of your songs is “{{ title }}”{{ artist ? ` by ${artist}` : '' }}? It’s used for it from now on, here and on your
        other devices.
      </DialogDescription>
      <Field v-model="query" label="Search your songs" :error="error" />
      <div class="-mx-1 max-h-[45vh] overflow-y-auto">
        <button
          v-for="track in ranked"
          :key="track.id"
          type="button"
          class="flex w-full min-w-0 items-center gap-3 rounded-lg px-2 py-2 text-left transition-colors hover:bg-well disabled:opacity-60"
          :disabled="choosing"
          @click="choose(track)"
        >
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-[13.5px] font-medium">{{ track.title }}</span>
            <span class="truncate text-[12.5px] text-muted-foreground">{{ track.artist }}</span>
          </span>
          <span class="flex-none text-[12.5px] text-muted-foreground tabular-nums">{{ time(track.durationMs) }}</span>
        </button>
        <p v-if="!ranked.length" class="px-2 py-3 text-[13.5px] text-muted-foreground">No songs found. Try other words.</p>
      </div>
      <div class="flex justify-end">
        <Button variant="outline" size="sm" @click="open = false">Cancel</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
