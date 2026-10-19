<script setup lang="ts">
/**
 * A small neutral badge ("This device", "Synced", "Phones"). Its own colour, so it shows on the page and on cards. A
 * badge of words only has them centred on the letters themselves (trimmed to the capitals and the baseline), not on a
 * line that keeps room below for letters like g and j, which badges don't use. The trim works on the words' own box,
 * not on the badge (a flex box), so they get one. A badge with an icon stays as it is (icon and words side by side).
 */
import { Text, useSlots } from 'vue'

const slots = useSlots()
const wordsOnly = () => slots.default?.().every((node) => node.type === Text) ?? false
</script>

<template>
  <span class="inline-flex h-5 flex-none items-center rounded-md bg-badge px-1.5 align-middle text-[11.5px] font-medium text-soft">
    <span v-if="wordsOnly()" class="[text-box:trim-both_cap_alphabetic]"><slot /></span>
    <slot v-else />
  </span>
</template>
