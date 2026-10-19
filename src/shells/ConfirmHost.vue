<script setup lang="ts">
/**
 * Shows the question asked with lib/confirm.ts's `ask`. Its words stay while it fades out (the question is kept until
 * the next one).
 */
import { ref, watch } from 'vue'
import { ConfirmDialog } from '@/components/settings'
import { asked, type Question } from '@/lib/confirm'

const open = ref(false)
const shown = ref<Question>({ title: '', text: '', action: '' })
watch(asked, (question) => {
  if (!question) return
  shown.value = question
  open.value = true
})

let chosen = false
function confirm() {
  chosen = true
}
function closed(now: boolean) {
  if (now) return
  open.value = false
  asked.value?.answer(chosen)
  asked.value = null
  chosen = false
}
</script>

<template>
  <ConfirmDialog
    :open="open"
    :title="shown.title"
    :text="shown.text"
    :action="shown.action"
    :danger="shown.danger"
    @update:open="closed"
    @confirm="confirm"
  />
</template>
