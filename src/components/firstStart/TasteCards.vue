<script setup lang="ts">
/**
 * The settings that come down to taste (tastes.ts): each a heading and a card per choice, saved as picked (the
 * first-start guide's Make it yours; New in Melue's new ones). `pretend`: as a new device, nothing saved but the look.
 */
import { DropValue, Reveal, SettingsCard, SettingsRow } from '@/components/settings'
import { THEME_TIME_OPTIONS, themeTimes } from '@/lib/theme'
import { isMobile } from '@/platform'
import MobileLimits from '@/settings/MobileLimits.vue'
import { CHOICE, picked } from './choice'
import type { Taste } from './tastes'

const props = defineProps<{ tastes: Taste[]; pretend?: boolean }>()

const columns = (taste: Taste) => (taste.options.length > 3 ? 2 : isMobile && taste.options.length > 2 ? 1 : taste.options.length)
function pick(taste: Taste, value: string) {
  taste.value.value = value
  if (taste.id === 'look' || !props.pretend) taste.save(value).catch(() => {})
}
</script>

<template>
  <div v-for="taste in tastes" :key="taste.title" class="mt-7">
    <h2 class="mb-2 text-sm font-semibold text-muted-foreground">{{ taste.title }}</h2>
    <div class="grid gap-2" :style="{ gridTemplateColumns: `repeat(${columns(taste)}, minmax(0, 1fr))` }">
      <button
        v-for="option in taste.options"
        :key="option.value"
        type="button"
        :class="[CHOICE, picked(taste.value.value === option.value)]"
        :aria-pressed="taste.value.value === option.value"
        @click="pick(taste, option.value)"
      >
        <span class="block text-[15px] font-semibold">{{ option.label }}</span>
        <span class="block text-[13px] text-muted-foreground">{{ option.hint }}</span>
      </button>
    </div>
    <!-- Adaptive: when it turns dark, and light (as in Settings' Display). -->
    <Reveal v-if="taste.id === 'look'" :show="taste.value.value === 'adaptive'" :line="false">
      <SettingsCard class="mt-2">
        <SettingsRow title="Dark from"><DropValue v-model="themeTimes.dark" :options="THEME_TIME_OPTIONS" /></SettingsRow>
        <SettingsRow title="Light from"><DropValue v-model="themeTimes.light" :options="THEME_TIME_OPTIONS" /></SettingsRow>
      </SettingsCard>
    </Reveal>
    <!-- Everything: the limits for songs (as in Settings' Mobile data). -->
    <Reveal v-if="taste.id === 'mobile'" :show="taste.value.value === 'all'" :line="false">
      <SettingsCard class="mt-2"><MobileLimits :pretend="pretend" /></SettingsCard>
    </Reveal>
  </div>
</template>
