<script setup lang="ts">
/**
 * The mini player (shells/tray/TrayPlayer.vue), live: a few of your most played songs with their covers, one after
 * another, as it shows them (made-up ones in a library without plays). With reduced motion: one song, still.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import TrayPlayer, { type TrayDemo } from '@/shells/tray/TrayPlayer.vue'
import { coverUrl, library } from '@/library'

const MADE_UP = [
  { title: 'Northern Lights', artist: 'The Quiet Hours' },
  { title: 'Paper Boats', artist: 'Lena Moor' },
  { title: 'Static Summer', artist: 'Glasshouse' },
]
// Each song this long, then the next.
const EACH_MS = 3500
const TICK_MS = 250

const songs = ref<TrayDemo['song'][]>(MADE_UP)
const demo = ref<TrayDemo>({ song: MADE_UP[0], position: 48, duration: 214, isPlaying: true })
let at = 0
let timer: ReturnType<typeof setInterval> | undefined

function show(i: number) {
  const song = songs.value[i % songs.value.length]
  demo.value = { song, position: 30 + ((i * 37) % 60), duration: 180 + ((i * 53) % 90), isPlaying: true }
}

onMounted(async () => {
  const played = await library.mostPlayed(5).catch(() => [])
  if (played.length >= 2) songs.value = played.map((t) => ({ title: t.title, artist: t.artist ?? '', cover: t.cover ? coverUrl(t.cover) : undefined, liked: t.liked }))
  show(0)
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
  let shown = 0
  timer = setInterval(() => {
    shown += TICK_MS
    if (shown >= EACH_MS) {
      shown = 0
      show(++at)
    } else {
      demo.value = { ...demo.value, position: demo.value.position + TICK_MS / 1000 }
    }
  }, TICK_MS)
})
onBeforeUnmount(() => clearInterval(timer))
</script>

<template>
  <div class="w-[300px] max-w-full overflow-hidden rounded-xl border shadow-2xl">
    <TrayPlayer :demo="demo" />
  </div>
</template>
