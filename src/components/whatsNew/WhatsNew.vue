<script setup lang="ts">
/**
 * New in Melue (whatsNew.ts), over the app like the first-start guide, a page at a time: the new features (each with a
 * line about it, live on its stage, and where to find it), then the new questions (the guide's cards, saved as picked).
 * All changes opens the changelog over it.
 */
import { ArrowLeft, Check } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import TasteCards from '@/components/firstStart/TasteCards.vue'
import { Button } from '@/components/ui/button'
import { isDesktop } from '@/platform'
import Changelog from '@/settings/WhatsNew.vue'
import WindowButtons from '@/shells/desktop/WindowButtons.vue'
import DemoStage from './DemoStage.vue'
import { newTastes, whatsNew } from './whatsNew'

const changelog = ref(false)
// Features of several versions (an update that skipped some): each marked with its own.
const versions = computed(() => new Set(whatsNew.features.map((f) => f.since)).size > 1)

// Its pages: the features, the questions (those there are).
const pages = computed(() => [...(whatsNew.features.length ? ['features' as const] : []), ...(newTastes.value.length ? ['tastes' as const] : [])])
const at = ref(0)
const back = ref(false)
watch(
  () => whatsNew.open,
  (open) => open && (at.value = 0),
)
const page = computed(() => pages.value[at.value])
const last = computed(() => at.value >= pages.value.length - 1)
function go(by: 1 | -1) {
  back.value = by < 0
  at.value += by
}
</script>

<template>
  <Transition name="fade">
    <div v-if="whatsNew.open" class="fixed inset-0 z-50 flex flex-col overflow-y-auto bg-background">
      <!-- A computer: the window dragged by the top (nothing shows), and its buttons. -->
      <div v-if="isDesktop" class="fixed inset-x-0 top-0 z-10 flex h-10 justify-end" data-tauri-drag-region>
        <WindowButtons />
      </div>
      <div class="mx-auto flex w-full max-w-[560px] flex-col px-5 pt-[max(env(safe-area-inset-top),2.5rem)] pb-10">
        <!-- Back, and All changes (the changelog, over this). -->
        <div class="mb-8 flex h-9 items-center justify-between">
          <Button v-if="at > 0" variant="ghost" size="sm" class="-ml-2" @click="go(-1)"><ArrowLeft />Back</Button>
          <span v-else />
          <Button variant="ghost" size="sm" class="-mr-2 text-muted-foreground" @click="changelog = true">All changes in {{ whatsNew.version }}</Button>
        </div>

        <Transition :name="back ? 'guide-back' : 'guide-on'" mode="out-in">
          <div :key="page" class="flex flex-col">
            <template v-if="page === 'features'">
              <h1 class="text-3xl font-bold tracking-tight phone:text-[28px]">New in Melue</h1>
              <section v-for="feature in whatsNew.features" :key="feature.id" class="mt-8 flex flex-col">
                <DemoStage v-if="feature.demo" :scale="feature.scale"><component :is="feature.demo" /></DemoStage>
                <span v-if="versions" class="mt-5 text-xs font-medium tracking-wide text-muted-foreground uppercase">In {{ feature.since }}</span>
                <h2 class="text-xl font-semibold" :class="versions ? 'mt-0.5' : 'mt-5'">{{ feature.title }}</h2>
                <p class="mt-1.5 text-[15px] text-muted-foreground">{{ feature.text }}</p>
                <p class="mt-2 text-[13.5px] text-muted-foreground">{{ feature.where }}</p>
              </section>
            </template>
            <template v-else>
              <h1 class="text-3xl font-bold tracking-tight phone:text-[28px]">{{ newTastes.length === 1 ? 'A new setting' : 'New settings' }}</h1>
              <p class="mt-3 text-[15px] text-muted-foreground">Pick what suits you. Any of them can be changed in Settings.</p>
              <TasteCards :tastes="newTastes" />
            </template>
            <Button v-if="!last" size="lg" class="mt-10 w-full" @click="go(1)">Next</Button>
            <Button v-else size="lg" class="mt-10 w-full" @click="whatsNew.open = false"><Check />Done</Button>
          </div>
        </Transition>
      </div>
    </div>
  </Transition>
  <Changelog v-model:open="changelog" :current="whatsNew.version" />
</template>
