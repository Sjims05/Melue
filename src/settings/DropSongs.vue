<script setup lang="ts">
/**
 * Dropping music files on Melue (desktop), on any page: an overlay while they're over it, then where they go (for
 * songs picked with "Add songs…" too; addSongs.ts): your share first when there is one, then the music folders on
 * this computer; with neither, the Music folder (added as a music folder). On a playlist's page they can go into it
 * too (ticked). Adding, it says what it did.
 */
import { Plus } from '@lucide/vue'
import { audioDir } from '@tauri-apps/api/path'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Reveal } from '@/components/settings'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { library } from '@/library'
import { sync } from '@/library/sync'
import { isDesktop } from '@/platform'
import { addSongs, adding, listenForDrops, type AddTo } from './addSongs'

let stop: (() => void) | undefined
onMounted(async () => {
  if (!isDesktop) return
  try {
    stop = await listenForDrops()
  } catch {
    // No window to drop on (a plain browser preview).
  }
})
onBeforeUnmount(() => stop?.())

const songs = (n: number) => `${n} song${n === 1 ? '' : 's'}`

// ---- Where they go
type Place = { key: string; title: string; detail: string; to: AddTo }
const places = ref<Place[]>([])
const chosen = ref('')
async function findPlaces() {
  const found: Place[] = []
  if (sync.available)
    found.push({
      key: 'share',
      title: 'Your synced music',
      detail: 'On the share, so every device gets them',
      to: { kind: 'share' },
    })
  const folders = await library.folders().catch(() => [])
  for (const f of folders.filter((f) => f.kind === 'path' && f.enabled && !f.synced)) {
    found.push({
      key: `folder:${f.id}`,
      title: f.name,
      detail: `Only on this computer · ${f.location}`,
      to: { kind: 'folder', id: f.id },
    })
  }
  if (!found.length) {
    const music = await audioDir().catch(() => '')
    if (music)
      found.push({
        key: 'music',
        title: 'Your Music folder',
        detail: `${music} · added to your music folders`,
        to: { kind: 'music', path: music },
      })
  }
  places.value = found
  chosen.value = found[0]?.key ?? ''
}

// ---- A playlist's page: into it too
const route = useRoute()
const playlist = ref<{ id: number; name: string } | null>(null)
const intoPlaylist = ref(true)

// ---- The window: open while files wait, once it knows where they can go (so it opens as it will look); after
// adding, what it did (until OK). `count` stays as it was while it fades out.
const asking = computed(() => adding.files.length > 0)
const open = ref(false)
const count = ref(0)
const working = ref(false)
const done = ref('')
watch(asking, async (now) => {
  if (!now) return (open.value = false)
  done.value = ''
  intoPlaylist.value = true
  count.value = adding.files.length
  const id = route.name === 'playlist' ? Number(route.params.id) : null
  const found = id == null ? null : await library.playlist(id).catch(() => null)
  playlist.value = found && { id: found.id, name: found.name }
  await findPlaces()
  open.value = asking.value
})
function close(open: boolean) {
  if (open || working.value) return
  adding.files = []
}
async function add() {
  const place = places.value.find((p) => p.key === chosen.value)
  if (!place) return
  working.value = true
  const into = playlist.value && intoPlaylist.value ? playlist.value.id : null
  const said = await addSongs(place.to, into)
  working.value = false
  done.value = adding.error
    ? said
    : `${said}${into ? `, and they go into “${playlist.value!.name}”` : ''}. They show up in a moment, once they’re scanned.`
}
</script>

<template>
  <Transition name="drop">
    <div
      v-if="adding.dragging > 0"
      class="pointer-events-none fixed inset-3 z-50 grid place-content-center justify-items-center gap-1 rounded-xl border-2 border-dashed border-foreground bg-card/90"
    >
      <Plus class="size-6" />
      <b>Drop to add {{ songs(adding.dragging) }}</b>
      <small class="text-muted-foreground">You choose where they go next</small>
    </div>
  </Transition>

  <Dialog :open="open" @update:open="close">
    <!-- One column that can't grow past the window (a long folder path is cut short, not pushing past the padding). -->
    <DialogContent :show-close-button="false" class="max-w-[420px] grid-cols-[minmax(0,1fr)] gap-0 rounded-xl bg-card p-4 sm:max-w-[420px]">
      <Transition name="swap" mode="out-in">
        <DialogTitle :key="done ? 'done' : 'ask'" class="mb-3 text-[15px] font-semibold">
          {{ done ? (adding.error ? 'Not added' : 'Added') : `Add ${songs(count)}` }}
        </DialogTitle>
      </Transition>
      <Transition name="swap" mode="out-in">
        <DialogDescription :key="done ? 'done' : 'ask'" class="text-[13.5px] leading-[1.45]">
          {{ done || 'They’re copied there; the files you picked stay where they are.' }}
        </DialogDescription>
      </Transition>

      <!-- Its space (with the room above it) closes as it goes, so the window shrinks smoothly to what it did. -->
      <Reveal :show="!done" :line="false">
        <div class="grid grid-cols-[minmax(0,1fr)] gap-2 pt-3">
          <div v-if="places.length">
            <p class="mb-1.5 text-[13px] font-medium">Put them in</p>
            <RadioGroup v-model="chosen" class="grid-cols-[minmax(0,1fr)] gap-0.5">
              <label
                v-for="place in places"
                :key="place.key"
                class="flex min-w-0 cursor-pointer items-start gap-3 rounded-lg px-2.5 py-2 transition-colors hover:bg-well"
              >
                <RadioGroupItem :value="place.key" class="mt-0.5" />
                <span class="min-w-0">
                  <span class="block text-[13.5px]">{{ place.title }}</span>
                  <span class="block truncate text-[12.5px] text-muted-foreground">{{ place.detail }}</span>
                </span>
              </label>
            </RadioGroup>
          </div>
          <p v-else class="text-[13px] text-muted-foreground">There’s nowhere to put them yet: add a music folder in Settings.</p>

          <label v-if="playlist" class="flex cursor-pointer items-center gap-3 rounded-lg px-2.5 py-2 transition-colors hover:bg-well">
            <Checkbox v-model="intoPlaylist" />
            <span class="min-w-0 truncate text-[13.5px]">Also add them to “{{ playlist.name }}”</span>
          </label>
        </div>
      </Reveal>

      <div class="mt-4 flex justify-end gap-2">
        <Button v-if="done" size="sm" @click="adding.files = []">OK</Button>
        <template v-else>
          <Button variant="outline" size="sm" :disabled="working" @click="adding.files = []">Cancel</Button>
          <Button size="sm" :disabled="working || !chosen" @click="add">{{ working ? 'Adding…' : `Add ${songs(count)}` }}</Button>
        </template>
      </div>
    </DialogContent>
  </Dialog>
</template>
