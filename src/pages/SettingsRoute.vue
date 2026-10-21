<script setup lang="ts">
/**
 * Settings as a main page (settings/SettingsPage.vue). Loaded on its own, not with the first screen (it's the biggest
 * page): router.ts's preloadPages has it ready shortly after start.
 */
import { defineAsyncComponent } from 'vue'

const SettingsPage = defineAsyncComponent(() => import('@/settings/SettingsPage.vue'))
</script>

<template>
  <SettingsPage />
</template>
