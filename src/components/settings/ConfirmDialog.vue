<script setup lang="ts">
/**
 * A question before something that's hard to undo (Move 6 songs to sync? Remove 12 copies?): a title, a line saying
 * what happens, Cancel and the action. `danger`: the action in red (it can't simply come back). Without an `action`
 * it only tells something (About this setting), with OK.
 */
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'

defineProps<{ title: string; text?: string; action?: string; danger?: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ confirm: [] }>()

// The action first: closing can clear what the question was about.
function confirm() {
  emit('confirm')
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false" class="max-w-[380px] gap-2 rounded-xl bg-card p-4 sm:max-w-[380px]">
      <DialogTitle class="text-[15px] font-semibold">{{ title }}</DialogTitle>
      <DialogDescription v-if="text || $slots.default" class="mb-1.5 text-[13.5px] leading-[1.45]">
        <slot>{{ text }}</slot>
      </DialogDescription>
      <div class="flex justify-end gap-2">
        <template v-if="action">
          <Button variant="outline" size="sm" @click="open = false">Cancel</Button>
          <Button :variant="danger ? 'danger' : 'default'" size="sm" @click="confirm">{{ action }}</Button>
        </template>
        <Button v-else size="sm" @click="open = false">OK</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
