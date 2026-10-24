<script setup lang="ts">
/**
 * A question before something that's hard to undo (Move 6 songs to sync? Remove 12 copies?): a title, a line saying
 * what happens, Cancel and the action. `danger`: the action in red (it can't simply come back). `choices`: ways to
 * go on, each with a line saying what it does (the first one chosen at first); `confirm` says which one. Without an
 * `action` it only tells something (About this setting), with OK.
 */
import { ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

export type Choice = { title: string; detail: string }

defineProps<{ title: string; text?: string; action?: string; danger?: boolean; choices?: Choice[] }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ confirm: [choice: number] }>()

const chosen = ref('0')
watch(open, (now) => now && (chosen.value = '0'))

// The action first: closing can clear what the question was about.
function confirm() {
  emit('confirm', Number(chosen.value))
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
      <RadioGroup v-if="choices?.length" v-model="chosen" class="mb-1.5 grid-cols-[minmax(0,1fr)] gap-0.5">
        <label
          v-for="(choice, i) in choices"
          :key="i"
          class="flex min-w-0 cursor-pointer items-start gap-3 rounded-lg px-2.5 py-2 transition-colors hover:bg-well"
        >
          <RadioGroupItem :value="String(i)" class="mt-0.5" />
          <span class="min-w-0">
            <span class="block text-[13.5px]">{{ choice.title }}</span>
            <span class="block text-[12.5px] text-muted-foreground">{{ choice.detail }}</span>
          </span>
        </label>
      </RadioGroup>
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
