<script setup lang="ts">
/**
 * "Ask first" (Let other devices play here): another of your devices wants to play here; Allow, or Not now. With no
 * answer it goes by itself when the other device stops waiting.
 */
import { MonitorSpeaker } from '@lucide/vue'
import { onBeforeUnmount, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { remote } from '@/library/remote'

const asking = ref<{ id: number; from: string } | null>(null)
let expires: ReturnType<typeof setTimeout> | undefined

const stop = remote.onAsk((a) => {
  asking.value = { id: a.id, from: a.from }
  clearTimeout(expires)
  expires = setTimeout(() => (asking.value = null), a.seconds * 1000)
})
onBeforeUnmount(() => stop.then((s) => s()).catch(() => {}))

function answer(yes: boolean) {
  if (!asking.value) return
  remote.answer(asking.value.id, yes).catch(() => {})
  asking.value = null
  clearTimeout(expires)
}
// Closed without a button (Escape): no.
const closed = (open: boolean) => !open && answer(false)
</script>

<template>
  <Dialog :open="!!asking" @update:open="closed">
    <DialogContent class="max-w-sm rounded-xl bg-card" @open-auto-focus.prevent>
      <div class="flex items-center gap-3">
        <span class="grid size-10 flex-none place-items-center rounded-full bg-well"><MonitorSpeaker class="size-5" /></span>
        <div class="flex min-w-0 flex-col">
          <DialogTitle>{{ asking?.from }} wants to play here</DialogTitle>
          <DialogDescription>Its music plays on this device, controlled from {{ asking?.from }}.</DialogDescription>
        </div>
      </div>
      <div class="flex justify-end gap-2 pt-1">
        <Button variant="outline" size="sm" @click="answer(false)">Not now</Button>
        <Button size="sm" @click="answer(true)">Allow</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
