<script setup lang="ts">
/** A small neutral badge ("This device", "Synced", "Phones"). Its own colour, so it shows on the page and on cards. */
</script>

<template>
  <span class="inline-flex h-5 flex-none items-center rounded-md bg-badge px-1.5 align-middle text-[11.5px] font-medium text-soft">
    <slot />
  </span>
</template>
