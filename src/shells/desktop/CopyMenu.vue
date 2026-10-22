<script setup lang="ts">
/** An error's Copy menu (lib/copyMenu.ts): at the pointer, in the right-click menus' style; anywhere else closes it. */
import { useEventListener } from '@vueuse/core'
import { ClipboardList, Copy } from '@lucide/vue'
import { nextTick, ref, watch } from 'vue'
import { copyChosen, copyMenu } from '@/lib/copyMenu'

const menu = ref<HTMLElement>()
const place = ref({ left: 0, top: 0 })
watch(
  () => copyMenu.open && `${copyMenu.x},${copyMenu.y}`,
  async (open) => {
    if (!open) return
    place.value = { left: copyMenu.x, top: copyMenu.y }
    await nextTick()
    // Inside the window.
    const box = menu.value?.getBoundingClientRect()
    if (box) place.value = { left: Math.min(copyMenu.x, innerWidth - box.width - 8), top: Math.min(copyMenu.y, innerHeight - box.height - 8) }
    menu.value?.querySelector('button')?.focus()
  },
)
const close = () => (copyMenu.open = false)
const ITEM =
  'relative flex w-full cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm whitespace-nowrap outline-hidden select-none hover:bg-accent focus:bg-accent focus:text-accent-foreground [&_svg]:size-4 [&_svg]:text-muted-foreground'
useEventListener('pointerdown', (e: PointerEvent) => copyMenu.open && !menu.value?.contains(e.target as Node) && close(), true)
useEventListener('keydown', (e: KeyboardEvent) => copyMenu.open && e.key === 'Escape' && close())
useEventListener('scroll', close, true)
useEventListener('blur', close)
</script>

<template>
  <div
    v-if="copyMenu.open"
    ref="menu"
    role="menu"
    class="fixed z-50 min-w-[8rem] animate-in rounded-md border bg-popover p-1 text-popover-foreground shadow-md fade-in-0 zoom-in-95"
    :style="{ left: `${place.left}px`, top: `${place.top}px` }"
  >
    <button type="button" role="menuitem" :class="ITEM" @click="copyChosen(false)"><Copy />Copy</button>
    <button type="button" role="menuitem" :class="ITEM" @click="copyChosen(true)"><ClipboardList />Copy with troubleshooting details</button>
  </div>
</template>
