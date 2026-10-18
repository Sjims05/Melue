<script setup lang="ts">
/**
 * Change cover: pick a picture (JPEG, PNG or WebP) or drop one on Melue, then choose its square: drag the picture to
 * move it, zoom with the slider or the mouse wheel. Save hands back the square, 1000 px, as a JPEG (`save`).
 */
import { ImagePlus } from '@lucide/vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { library } from '@/library'
import { adding } from '@/settings/addSongs'

const props = defineProps<{ name: string }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ save: [picture: Uint8Array] }>()

// The crop square on screen, and the cover made from it.
const VIEW = 320
const OUT = 1000
const MAX_ZOOM = 3

const file = ref<HTMLInputElement>()
const picture = ref<HTMLImageElement | null>(null)
const url = ref('')
const zoom = ref(1)
const offset = ref({ x: 0, y: 0 })
watch(open, (now) => {
  if (now) return
  if (url.value) URL.revokeObjectURL(url.value)
  picture.value = null
  url.value = ''
})

const problem = ref('')
function show(picked: Blob) {
  if (url.value) URL.revokeObjectURL(url.value)
  url.value = URL.createObjectURL(picked)
  const image = new Image()
  image.onload = () => {
    problem.value = ''
    picture.value = image
    zoom.value = 1
    offset.value = { x: 0, y: 0 }
  }
  image.onerror = () => (problem.value = 'That picture couldn’t be read. Try a JPEG, PNG or WebP.')
  image.src = url.value
}
function chosen(event: Event) {
  const picked = (event.target as HTMLInputElement).files?.[0]
  if (picked) show(picked)
}

// Dropping a picture anywhere on Melue while this is open. In the app, the window gets dropped files (with their
// paths, read by the library); in a plain browser, the page does. Songs aren't added meanwhile (addSongs.ts).
const PICTURES = ['jpg', 'jpeg', 'png', 'webp']
const isPicture = (path: string) => PICTURES.includes(path.split('.').pop()?.toLowerCase() ?? '')
const over = ref(false)
adding.paused = true
onBeforeUnmount(() => (adding.paused = false))
let stopListening: (() => void) | undefined
let gone = false
onBeforeUnmount(() => ((gone = true), stopListening?.()))
try {
  getCurrentWebview()
    .onDragDropEvent(({ payload }) => {
      if (payload.type === 'enter') over.value = payload.paths.some(isPicture)
      else if (payload.type === 'leave') over.value = false
      else if (payload.type === 'drop') {
        over.value = false
        const path = payload.paths.find(isPicture)
        if (path) library.readPicture(path).then((bytes) => show(new Blob([bytes])), (e) => (problem.value = String(e)))
        else if (payload.paths.length) problem.value = 'That isn’t a picture Melue can read (JPEG, PNG or WebP).'
      }
    })
    .then((stop) => (gone ? stop() : (stopListening = stop)))
    .catch(() => {})
} catch {
  // No app window: a plain browser preview (the page's own drop below).
}
const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes('Files')
function dragOver(e: DragEvent) {
  if (!hasFiles(e)) return
  e.preventDefault()
  over.value = true
}
function dropped(e: DragEvent) {
  if (!hasFiles(e)) return
  e.preventDefault()
  over.value = false
  const picked = [...(e.dataTransfer?.files ?? [])].find((f) => isPicture(f.name))
  if (picked) show(picked)
  else problem.value = 'That isn’t a picture Melue can read (JPEG, PNG or WebP).'
}

// The picture fills the square at zoom 1 (its shorter side fits); it can't be moved off it.
const base = computed(() => (picture.value ? Math.max(VIEW / picture.value.naturalWidth, VIEW / picture.value.naturalHeight) : 1))
const size = computed(() => ({
  w: (picture.value?.naturalWidth ?? VIEW) * base.value * zoom.value,
  h: (picture.value?.naturalHeight ?? VIEW) * base.value * zoom.value,
}))
function clamp(x: number, y: number) {
  const maxX = (size.value.w - VIEW) / 2
  const maxY = (size.value.h - VIEW) / 2
  return { x: Math.max(-maxX, Math.min(maxX, x)), y: Math.max(-maxY, Math.min(maxY, y)) }
}
watch(zoom, () => (offset.value = clamp(offset.value.x, offset.value.y)))

let drag: { x: number; y: number; from: { x: number; y: number } } | null = null
function grab(e: PointerEvent) {
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  drag = { x: e.clientX, y: e.clientY, from: { ...offset.value } }
}
function move(e: PointerEvent) {
  if (drag) offset.value = clamp(drag.from.x + e.clientX - drag.x, drag.from.y + e.clientY - drag.y)
}
const letGo = () => (drag = null)
// The keyboard, on the square: the arrows move the picture (Shift: further), + and - zoom.
function key(e: KeyboardEvent) {
  const by = e.shiftKey ? 40 : 10
  const moves: Record<string, [number, number]> = { ArrowLeft: [by, 0], ArrowRight: [-by, 0], ArrowUp: [0, by], ArrowDown: [0, -by] }
  if (moves[e.key]) offset.value = clamp(offset.value.x + moves[e.key][0], offset.value.y + moves[e.key][1])
  else if (e.key === '+' || e.key === '=') zoom.value = Math.min(MAX_ZOOM, zoom.value + 0.1)
  else if (e.key === '-') zoom.value = Math.max(1, zoom.value - 0.1)
  else return
  e.preventDefault()
}
function wheel(e: WheelEvent) {
  zoom.value = Math.max(1, Math.min(MAX_ZOOM, zoom.value - e.deltaY * 0.0015))
}

async function save() {
  const image = picture.value
  if (!image) return
  const scale = base.value * zoom.value
  // The square's top left corner in the picture, and its side, in the picture's own pixels.
  const left = (VIEW - size.value.w) / 2 + offset.value.x
  const top = (VIEW - size.value.h) / 2 + offset.value.y
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = OUT
  canvas.getContext('2d')!.drawImage(image, -left / scale, -top / scale, VIEW / scale, VIEW / scale, 0, 0, OUT, OUT)
  const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, 'image/jpeg', 0.9))
  if (!blob) return
  emit('save', new Uint8Array(await blob.arrayBuffer()))
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      :show-close-button="false"
      class="max-w-[368px] gap-3 rounded-xl bg-card p-4 sm:max-w-[368px]"
      @dragover="dragOver"
      @dragleave.self="over = false"
      @drop="dropped"
    >
      <DialogTitle class="text-[15px] font-semibold">Cover for “{{ props.name }}”</DialogTitle>
      <DialogDescription class="text-[13.5px] leading-[1.45]">
        {{ picture ? 'Drag the picture to place it; zoom with the slider or the mouse wheel.' : 'Pick a picture (JPEG, PNG or WebP), or drop one here.' }}
      </DialogDescription>
      <p v-if="problem" class="text-[13px] text-destructive">{{ problem }}</p>
      <input ref="file" type="file" accept="image/jpeg,image/png,image/webp" class="hidden" @change="chosen" />

      <div
        v-if="picture"
        class="relative mx-auto cursor-grab touch-none overflow-hidden rounded-lg bg-well outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 active:cursor-grabbing"
        tabindex="0"
        role="group"
        aria-label="The picture: arrow keys move it, + and - zoom"
        @keydown="key"
        :style="{ width: `${VIEW}px`, height: `${VIEW}px` }"
        @pointerdown="grab"
        @pointermove="move"
        @pointerup="letGo"
        @pointercancel="letGo"
        @wheel.prevent="wheel"
      >
        <div
          v-if="over"
          class="absolute inset-0 z-10 grid place-items-center rounded-lg border-2 border-dashed border-foreground bg-card/85 text-sm font-medium"
        >
          Drop to use this picture
        </div>
        <img
          :src="url"
          alt=""
          draggable="false"
          class="pointer-events-none absolute top-1/2 left-1/2 max-w-none"
          :style="{ width: `${size.w}px`, height: `${size.h}px`, transform: `translate(calc(-50% + ${offset.x}px), calc(-50% + ${offset.y}px))` }"
        />
      </div>
      <button
        v-else
        type="button"
        class="mx-auto grid place-items-center gap-2 rounded-lg border-2 border-dashed text-muted-foreground transition-colors hover:bg-well hover:text-foreground"
        :class="over ? 'border-foreground bg-well text-foreground' : 'border-border'"
        :style="{ width: `${VIEW}px`, height: `${VIEW}px` }"
        @click="file?.click()"
      >
        <span class="grid place-items-center gap-2"><ImagePlus class="size-8" :stroke-width="1.5" />{{ over ? 'Drop to use this picture' : 'Choose a picture… or drop one here' }}</span>
      </button>
      <input v-if="picture" v-model.number="zoom" type="range" :min="1" :max="MAX_ZOOM" step="0.01" aria-label="Zoom" class="w-full accent-foreground" />

      <div class="flex justify-between gap-2">
        <Button v-if="picture" variant="outline" size="sm" @click="file?.click()">Another picture…</Button>
        <span v-else />
        <div class="flex gap-2">
          <Button variant="outline" size="sm" @click="open = false">Cancel</Button>
          <Button size="sm" :disabled="!picture" @click="save">Save</Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
