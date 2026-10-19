<script setup lang="ts">
/**
 * The heading of a page that is a list to play (an album, a folder, a playlist; Melue's design): its big cover on the
 * left; at its right, badges (the `badges` slot), the title, a grey line (the `line` slot: "Album · Juniper Vale ·
 * 2023 · 12 songs · 48 min", with links), and Play and Shuffle (the `actions` slot after them: a Top songs list's
 * size), then ⋯ with `menu`.
 * `editable`: the title is a button that turns into a text field with the name in it (also `rename()` from outside,
 * a menu's Rename): Enter or leaving it saves (`rename`), Escape doesn't. `menu`: the list's card menu (`action`), in
 * the ⋯ button and as the right-click menu of the whole heading (the cover, the title, anywhere in it).
 */
import { Ellipsis, Shuffle } from '@lucide/vue'
import { nextTick, ref, type Component } from 'vue'
import { Cover, type CoverColors, type MenuItem } from '@/components/covers'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { PlayIcon } from '@/components/player/icons'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  title: string
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
  /** Nothing to play (an empty list): Play and Shuffle are off. */
  empty?: boolean
  editable?: boolean
  menu?: MenuItem[]
  /** The cover is a button (a playlist's: Change cover), with this as its words for screen readers and the pointer. */
  coverLabel?: string
}>()
const emit = defineEmits<{ play: []; shuffle: []; rename: [name: string]; action: [action: string]; cover: [] }>()

const editing = ref(false)
const name = ref('')
const field = ref<HTMLInputElement>()
async function rename() {
  name.value = props.title
  editing.value = true
  await nextTick()
  field.value?.focus()
  field.value?.select()
}
function finish(save: boolean) {
  if (!editing.value) return
  editing.value = false
  const named = name.value.trim()
  if (save && named && named !== props.title) emit('rename', named)
}
defineExpose({ rename })
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!menu?.length || editing">
      <div class="mb-10 flex items-center gap-12 pt-2 phone:mb-6 phone:flex-col phone:items-stretch phone:gap-5 phone:pt-1">
        <button
          v-if="coverLabel"
          type="button"
          class="w-[min(16.5rem,30%)] flex-none rounded-lg transition-opacity outline-none hover:opacity-85 focus-visible:ring-2 focus-visible:ring-ring/50 phone:w-52 phone:self-center"
          :aria-label="coverLabel"
          :title="coverLabel"
          @click="emit('cover')"
        >
          <Cover :image="image" :colors="colors" :icon="icon" :collage="collage" />
        </button>
        <div v-else class="w-[min(16.5rem,30%)] flex-none phone:w-52 phone:self-center"><Cover :image="image" :colors="colors" :icon="icon" :collage="collage" /></div>
        <div class="min-w-0 flex-1">
          <div v-if="$slots.badges" class="mb-4 flex flex-wrap gap-2 phone:mb-2"><slot name="badges" /></div>
          <input
            v-if="editing"
            ref="field"
            v-model="name"
            aria-label="Name"
            class="-mx-2 w-full rounded-lg bg-card px-2 text-4xl leading-tight font-bold tracking-tight outline-none ring-2 ring-ring/50 phone:text-[28px]"
            @keydown.enter.prevent="finish(true)"
            @keydown.escape.stop.prevent="finish(false)"
            @blur="finish(true)"
          />
          <h1 v-else-if="editable" class="text-4xl leading-tight font-bold tracking-tight phone:text-[28px]">
            <button type="button" class="-mx-2 line-clamp-2 rounded-lg px-2 text-left hover:bg-card" title="Rename" @click="rename">{{ title }}</button>
          </h1>
          <h1 v-else class="line-clamp-2 text-4xl leading-tight font-bold tracking-tight phone:text-[28px]">{{ title }}</h1>
          <p class="mt-2 text-lg text-muted-foreground phone:mt-1 phone:text-[15px]"><slot name="line" /></p>
          <div class="mt-6 flex gap-4 phone:mt-4 phone:flex-wrap phone:gap-3">
            <Button size="lg" :disabled="empty" @click="emit('play')"><PlayIcon class="size-3.5" />Play</Button>
            <Button size="lg" variant="outline" :disabled="empty" @click="emit('shuffle')"><Shuffle />Shuffle</Button>
            <slot name="actions" />
            <DropdownMenu v-if="menu?.length">
              <DropdownMenuTrigger as-child>
                <Button size="icon-lg" variant="outline" :aria-label="`${title}: more`"><Ellipsis /></Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" class="min-w-52">
                <template v-for="item in menu" :key="item.action">
                  <DropdownMenuSeparator v-if="item.apart" />
                  <DropdownMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
                    <component :is="item.icon" />{{ item.label }}
                  </DropdownMenuItem>
                </template>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </div>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="menu?.length" class="min-w-52">
      <template v-for="item in menu" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
          <component :is="item.icon" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
