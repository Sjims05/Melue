<script setup lang="ts">
/**
 * The spike's two pages: the tests (each platform's own, in the slot) and the components (Phase 3's finished UI, as
 * it will be in the app). The last one picked is opened again.
 */
import { ref, watch } from 'vue'
import { TooltipProvider } from '@/components/ui/tooltip'
import ComponentsPage from './ComponentsPage.vue'

type Page = 'tests' | 'components'
const KEY = 'melue-spike-page'

// The address can pick it too (?page=components), e.g. in a browser while working on a component.
function saved(): Page {
  const asked = new URLSearchParams(location.search).get('page')
  if (asked === 'components' || asked === 'tests') return asked
  try {
    return localStorage.getItem(KEY) === 'components' ? 'components' : 'tests'
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
          v-for="[value, label] in [['tests', 'Tests'], ['components', 'Components']] as const"
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
    <ComponentsPage v-else />
  </TooltipProvider>
</template>
