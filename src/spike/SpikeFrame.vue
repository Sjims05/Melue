<script setup lang="ts">
/**
 * The spike's pages: the tests (each platform's own, in the slot), the settings page (Phase 3's finished UI, as it
 * will be in the app), and the components (the building blocks, and design options being tried). The last one picked
 * is opened again.
 */
import { ref, watch } from 'vue'
import { TooltipProvider } from '@/components/ui/tooltip'
import SettingsPage from '@/settings/SettingsPage.vue'
import ComponentsPage from './ComponentsPage.vue'

type Page = 'tests' | 'settings' | 'components'
const PAGES = ['tests', 'settings', 'components'] as const
const KEY = 'melue-spike-page'

const isPage = (value: string | null): value is Page => PAGES.includes(value as Page)

// The address can pick it too (?page=settings), e.g. in a browser while working on a component.
function saved(): Page {
  const asked = new URLSearchParams(location.search).get('page')
  if (isPage(asked)) return asked
  try {
    const kept = localStorage.getItem(KEY)
    return isPage(kept) ? kept : 'tests'
  } catch {
    return 'tests'
  }
}

const page = ref<Page>(saved())
watch(page, (value) => {
  try {
    localStorage.setItem(KEY, value)
  } catch {
    // Not kept: the spike opens on Tests next time.
  }
})
</script>

<template>
  <TooltipProvider>
    <nav class="sticky top-0 z-40 flex justify-center border-b bg-background/90 p-2 backdrop-blur">
      <div class="inline-flex rounded-[9px] bg-secondary p-[3px]">
        <button
          v-for="[value, label] in [['tests', 'Tests'], ['settings', 'Settings'], ['components', 'Components']] as const"
          :key="value"
          type="button"
          class="h-[30px] rounded-[7px] px-4 text-[13px] font-medium"
          :class="page === value ? 'bg-primary text-primary-foreground' : 'text-muted-foreground'"
          @click="page = value"
        >
          {{ label }}
        </button>
      </div>
    </nav>
    <slot v-if="page === 'tests'" />
    <SettingsPage v-else-if="page === 'settings'" />
    <ComponentsPage v-else />
  </TooltipProvider>
</template>
