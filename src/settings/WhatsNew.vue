<script setup lang="ts">
/** What's new (About): each version's summary and points, newest first (lib/whatsNew.ts, from CHANGELOG.md). */
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { versions } from '@/lib/whatsNew'

const open = defineModel<boolean>('open', { required: true })
defineProps<{ current?: string }>()

/** "22 Oct 2026" */
const day = (date: string) => (date ? new Date(`${date}T12:00:00`).toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }) : '')
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-xl rounded-xl bg-card" @open-auto-focus.prevent>
      <DialogTitle>What’s new</DialogTitle>
      <DialogDescription>What changed in each version of Melue, newest first.</DialogDescription>
      <div class="-mx-2 max-h-[60vh] overflow-y-auto px-2">
        <section v-for="v in versions" :key="v.version" class="border-b py-4 first:pt-1 last:border-0">
          <h3 class="flex items-baseline gap-2 text-[15px] font-semibold">
            {{ v.version }}
            <span v-if="v.version === current" class="rounded bg-well px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">This version</span>
            <span class="ml-auto text-xs font-normal text-muted-foreground">{{ day(v.date) }}</span>
          </h3>
          <p class="mt-1 text-sm">{{ v.summary }}</p>
          <ul v-if="v.points.length" class="mt-2 flex list-disc flex-col gap-1.5 pl-5 text-[13px] text-muted-foreground">
            <li v-for="(point, i) in v.points" :key="i">{{ point }}</li>
          </ul>
        </section>
      </div>
    </DialogContent>
  </Dialog>
</template>
