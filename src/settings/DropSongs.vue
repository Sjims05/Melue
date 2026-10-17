<script setup lang="ts">
/**
 * Dropping music files on Melue (desktop, with a share): an overlay while they're over it, then the question for
 * them and for songs picked with "Add songs…" (addSongs.ts). Shown on the settings page for now; the app's frame
 * takes it over once it's there.
 */
import { Plus } from '@lucide/vue'
import { onBeforeUnmount, onMounted } from 'vue'
import { ConfirmDialog } from '@/components/settings'
import { sync } from '@/library/sync'
import { isDesktop } from '@/platform'
import { addSongs, adding, listenForDrops } from './addSongs'

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
</script>

<template>
  <Transition name="drop">
    <div
      v-if="adding.dragging > 0"
      class="pointer-events-none fixed inset-3 z-50 grid place-content-center justify-items-center gap-1 rounded-xl border-2 border-dashed border-foreground bg-card/90"
    >
      <Plus class="size-6" />
      <b>{{ sync.available ? `Drop to add ${songs(adding.dragging)}` : 'Set up a share to add songs' }}</b>
      <small class="text-muted-foreground">{{ sync.available ? 'They’re copied to your synced music' : 'Songs you add go to your share' }}</small>
    </div>
  </Transition>
  <ConfirmDialog
    :open="sync.available && adding.files.length > 0"
    :title="`Add ${songs(adding.files.length)} to your synced music?`"
    text="They’re copied to your share, so every device gets them. The files you picked stay where they are."
    :action="`Add ${songs(adding.files.length)}`"
    @update:open="adding.files = []"
    @confirm="addSongs"
  />
</template>
