<script setup lang="ts">
/**
 * A strip with an icon and a line of text. `note` (a tip; Simon's primary colour), `ok` (green: it worked), `bad`
 * (red: it didn't). The `action` slot holds a link or button at its end.
 */
import { CircleCheck, CircleX, Info } from '@lucide/vue'
import type { Component } from 'vue'
import { computed } from 'vue'

const props = withDefaults(defineProps<{ kind?: 'note' | 'ok' | 'bad'; icon?: Component }>(), { kind: 'note' })

const icon = computed(() => props.icon ?? { note: Info, ok: CircleCheck, bad: CircleX }[props.kind])
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
    <span class="flex-1"><slot /></span>
    <slot name="action" />
  </div>
</template>
