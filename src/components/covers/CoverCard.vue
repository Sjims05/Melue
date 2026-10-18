<script setup lang="ts">
/**
 * A playlist, album, auto playlist or song as a card (Home, Playlists, an artist's albums, search): its cover, its
 * name, and a grey line ("8 songs · 31 min"). The cover's props go to Cover; the `badge` slot holds a CoverBadge.
 * The whole card is a button (it opens what it is); `menu` is its right-click menu (long-press on a phone: coverMenu,
 * or a song's songMenu), and the card shows its highlight while the menu is open. `editing`: its name is a text field
 * with the name in it (Rename): Enter or leaving it saves (`rename`), Escape doesn't (`cancel`). Its spacing and how it
 * shows it's under the pointer: covers.css (the highlight behind the whole card). Put cards in a `.cover-grid`
 * (`--cover-min`: the narrowest a card may get).
 */
import { nextTick, ref, watch, type Component } from 'vue'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from '@/components/ui/context-menu'
import Cover from './Cover.vue'
import type { MenuItem } from './coverMenu'
import type { CoverColors } from './coverRecipe'

const props = defineProps<{
  title: string
  subtitle?: string
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
  menu?: MenuItem[]
  editing?: boolean
}>()
const emit = defineEmits<{ open: []; action: [action: string]; rename: [name: string]; cancel: [] }>()

// Renaming: the field starts with the name, all of it selected.
const field = ref<HTMLInputElement>()
const name = ref(props.title)
let done = false
watch(
  () => props.editing,
  async (editing) => {
    if (!editing) return
    name.value = props.title
    done = false
    await nextTick()
    field.value?.focus()
    field.value?.select()
  },
  { immediate: true },
)
function finish(save: boolean) {
  if (done) return
  done = true
  if (save) emit('rename', name.value.trim() || props.title)
  else emit('cancel')
}
</script>

<template>
  <ContextMenu :modal="false">
    <ContextMenuTrigger as-child :disabled="!menu?.length || editing">
      <component
        :is="editing ? 'div' : 'button'"
        :type="editing ? undefined : 'button'"
        class="group/cover cover-card min-w-0 text-left outline-none"
        @click="!editing && emit('open')"
      >
        <Cover
          :image="image"
          :colors="colors"
          :icon="icon"
          :collage="collage"
          class="group-focus-visible/cover:ring-2 group-focus-visible/cover:ring-ring/50"
        >
          <slot name="badge" />
        </Cover>
        <span class="cover-card-text min-w-0">
          <input
            v-if="editing"
            ref="field"
            v-model="name"
            aria-label="Name"
            class="-mx-1.5 -my-0.5 rounded-md bg-background px-1.5 py-0.5 text-[15px] font-medium outline-none ring-2 ring-ring/50"
            @keydown.enter.prevent="finish(true)"
            @keydown.escape.stop.prevent="finish(false)"
            @blur="finish(true)"
          />
          <span v-else class="truncate text-[15px] font-medium">{{ title }}</span>
          <span v-if="subtitle" class="truncate text-sm text-muted-foreground">{{ subtitle }}</span>
        </span>
      </component>
    </ContextMenuTrigger>
    <ContextMenuContent v-if="menu?.length" class="min-w-52">
      <template v-for="item in menu" :key="item.action">
        <ContextMenuSeparator v-if="item.apart" />
        <ContextMenuItem :variant="item.danger ? 'destructive' : 'default'" @select="emit('action', item.action)">
          <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
        </ContextMenuItem>
      </template>
    </ContextMenuContent>
  </ContextMenu>
</template>
