<script setup lang="ts">
/**
 * An update was found (lib/updates.ts): Melue asks. Update installs it (and starts Melue again); Not now leaves it
 * offered in About; Don't ask again switches the asking off ("Ask when an update is found", in About).
 */
import { Sparkles } from '@lucide/vue'
import { computed } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { installUpdate, notNow, setAskForUpdates, updates } from '@/lib/updates'

const open = computed({ get: () => updates.asking, set: (v) => !v && notNow() })
// The release's notes, the start of them.
const notes = computed(() => updates.notes.split('\n').filter((l) => l.trim()).slice(0, 6).join('\n'))
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md rounded-xl bg-card" @open-auto-focus.prevent>
      <div class="flex items-center gap-3">
        <span class="grid size-10 flex-none place-items-center rounded-full bg-well"><Sparkles class="size-5" /></span>
        <div class="flex min-w-0 flex-col">
          <DialogTitle>Melue {{ updates.version }} is ready</DialogTitle>
          <DialogDescription>Update now? Melue starts again by itself, and your music and settings stay as they are.</DialogDescription>
        </div>
      </div>
      <p v-if="notes" class="max-h-48 overflow-y-auto rounded-lg bg-well px-3.5 py-2.5 text-[13px] whitespace-pre-line text-muted-foreground">{{ notes }}</p>
      <div class="flex flex-wrap items-center gap-2 pt-1">
        <Button variant="ghost" size="sm" class="mr-auto text-muted-foreground" @click="setAskForUpdates(false)">Don’t ask again</Button>
        <Button variant="outline" size="sm" @click="notNow">Not now</Button>
        <Button size="sm" @click="installUpdate">Update</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
