<script setup lang="ts">
/** The keyboard shortcuts (lib/hotkeys.ts), opened with ? (and from About). */
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { SHORTCUTS, shortcuts } from '@/lib/hotkeys'
import { nextTick, ref, watch } from 'vue'

// ↑ / ↓ scroll the list (it takes the focus when it opens).
const list = ref<HTMLElement>()
const scroll = (by: number) => list.value?.scrollBy({ top: by * 56, behavior: 'smooth' })
watch(
  () => shortcuts.open,
  async (open) => {
    if (!open) return
    await nextTick()
    setTimeout(() => list.value?.focus({ preventScroll: true }), 50)
  },
)
</script>

<template>
  <Dialog v-model:open="shortcuts.open">
    <DialogContent class="max-w-lg rounded-xl bg-card" @open-auto-focus.prevent>
      <DialogTitle>Keyboard shortcuts</DialogTitle>
      <DialogDescription>Anywhere in Melue, except while typing.</DialogDescription>
      <div ref="list" class="-mx-2 max-h-[65vh] overflow-y-auto px-2 outline-none" tabindex="-1" @keydown.up.prevent="scroll(-1)" @keydown.down.prevent="scroll(1)">
        <section v-for="group in SHORTCUTS" :key="group.group" class="py-2">
          <h3 class="mb-1.5 text-xs font-semibold tracking-wide text-muted-foreground uppercase">{{ group.group }}</h3>
          <div v-for="item in group.items" :key="item.what" class="flex items-center justify-between gap-4 py-1.5 text-[13.5px]">
            <span>{{ item.what }}</span>
            <span class="flex flex-none items-center gap-1.5">
              <template v-for="(combo, i) in item.keys" :key="i">
                <span v-if="i > 0" class="text-xs text-muted-foreground">or</span>
                <span class="flex items-center gap-0.5">
                  <kbd v-for="k in combo" :key="k" class="min-w-6 rounded border border-b-2 bg-well px-1.5 py-0.5 text-center font-sans text-xs">{{ k }}</kbd>
                </span>
              </template>
            </span>
          </div>
        </section>
      </div>
    </DialogContent>
  </Dialog>
</template>
