<script setup lang="ts">
/**
 * A synced playlist that changed on two devices in ways that don't merge: what happened, and the choices (this
 * device's version, the other's, or both; or, when one deleted it, keep it or delete it).
 */
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { librarySync, type ConflictChoice, type PlaylistConflict } from '@/library/sync'

const props = defineProps<{ conflict: PlaylistConflict }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ answered: [] }>()

async function answer(choice: ConflictChoice) {
  await librarySync.answer(props.conflict.syncId, choice)
  open.value = false
  emit('answered')
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false" class="max-w-[420px] gap-2 rounded-xl bg-card p-4 sm:max-w-[420px]">
      <DialogTitle class="text-[15px] font-semibold">“{{ conflict.name }}” needs a choice</DialogTitle>
      <DialogDescription class="mb-1.5 text-[13.5px] leading-[1.45]">
        <template v-if="conflict.kind === 'changed'">It was changed on this device and on {{ conflict.deviceName }} while they couldn’t sync.</template>
        <template v-else-if="conflict.kind === 'deletedThere'">It was deleted on {{ conflict.deviceName }}, but changed on this device since.</template>
        <template v-else>It was deleted on this device, but changed on {{ conflict.deviceName }} since.</template>
      </DialogDescription>
      <div class="flex flex-wrap justify-end gap-2">
        <template v-if="conflict.kind === 'changed'">
          <Button variant="outline" size="sm" @click="answer('both')">Keep both</Button>
          <Button variant="outline" size="sm" @click="answer('theirs')">Keep {{ conflict.deviceName }}’s</Button>
          <Button size="sm" @click="answer('mine')">Keep this device’s</Button>
        </template>
        <template v-else-if="conflict.kind === 'deletedThere'">
          <Button variant="danger" size="sm" @click="answer('theirs')">Delete it</Button>
          <Button size="sm" @click="answer('mine')">Keep it</Button>
        </template>
        <template v-else>
          <Button variant="danger" size="sm" @click="answer('mine')">Delete it</Button>
          <Button size="sm" @click="answer('theirs')">Keep it</Button>
        </template>
      </div>
    </DialogContent>
  </Dialog>
</template>
