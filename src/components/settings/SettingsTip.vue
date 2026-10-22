<script setup lang="ts">
/**
 * A strip with an icon and a line of text. `note` (a tip; the primary colour), `ok` (green: it worked), `bad`
 * (red: it didn't; its words can be copied with a right-click, and a bug report includes it: lib/report/trail.ts).
 * The `action` slot holds a link or button at its end.
 */
import { CircleCheck, CircleX, Info } from '@lucide/vue'
import type { Component } from 'vue'
import { computed, onMounted, ref } from 'vue'
import { noteError } from '@/lib/report/trail'

const props = withDefaults(defineProps<{ kind?: 'note' | 'ok' | 'bad'; icon?: Component }>(), { kind: 'note' })

const icon = computed(() => props.icon ?? { note: Info, ok: CircleCheck, bad: CircleX }[props.kind])
const words = ref<HTMLElement>()
onMounted(() => props.kind === 'bad' && words.value?.textContent && noteError(`Shown: ${words.value.textContent.trim()}`))
</script>

<template>
  <div
    class="flex items-center gap-2.5 rounded-[10px] px-3.5 py-2.5 text-[13px]"
    :class="{
      'bg-primary text-primary-foreground': kind === 'note',
      'bg-ok/12 text-ok': kind === 'ok',
      'bg-destructive/12 text-destructive': kind === 'bad',
    }"
  >
    <component :is="icon" class="size-[15px] flex-none" />
    <!-- An error's words can be copied (a right-click, or a long press on a phone: lib/copyMenu.ts). -->
    <span ref="words" class="flex-1" :class="kind === 'bad' && 'select-text'" :data-copy="kind === 'bad' ? '' : undefined"><slot /></span>
    <slot name="action" />
  </div>
</template>
