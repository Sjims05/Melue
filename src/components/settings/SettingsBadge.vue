<script setup lang="ts">
/**
 * A small neutral badge ("This device", "Synced", "Phones"). Its own colour, so it shows on the page and on cards. Its
 * words are centred on the letters themselves (the text trimmed to the capitals and the baseline), not on a line that
 * keeps room below for letters like g and j, which badges don't use.
 */
</script>

<template>
  <span class="inline-flex h-5 flex-none items-center rounded-md bg-badge px-1.5 align-middle text-[11.5px] font-medium text-soft [text-box:trim-both_cap_alphabetic]">
    <slot />
  </span>
</template>
