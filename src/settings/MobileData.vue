<script setup lang="ts">
/**
 * Mobile data: what syncs on mobile data, the limits for songs (with Everything), and how much was used
 * this month. Shown on every device (settings sync), marked as being about phones.
 *
 * Wired: Small things / Everything (Melue's "copy songs on metered connections"). Not wired yet (Melue doesn't have
 * them): Nothing, the limits and the month's use; they keep their values only here, for the design.
 */
import { computed, onMounted, reactive, ref } from 'vue'
import { DropValue, Reveal, Segmented, SettingsBadge, SettingsCard, SettingsHeading, SettingsRow } from '@/components/settings'
import { isMobile } from '@/platform'
import { loadSyncSettings, saveSyncSettings, syncSettings } from './syncSettings'

type Level = 'nothing' | 'small' | 'all'
const LEVELS = [['nothing', 'Nothing'], ['small', 'Small things'], ['all', 'Everything']] as const
const LEVEL_LINE: Record<Level, string> = {
  nothing: 'Wait for Wi-Fi for everything',
  small: 'Playback, likes and playlists; songs wait for Wi-Fi',
  all: 'Songs too, within the limits below',
}
const LIMITS = [[0, 'No limit'], [250, '250 MB'], [500, '500 MB'], [1000, '1 GB'], [2000, '2 GB']] as const
const ordinal = (n: number) => `${n}${[11, 12, 13].includes(n) ? 'th' : ({ 1: 'st', 2: 'nd', 3: 'rd' } as Record<number, string>)[n % 10] ?? 'th'}`
const DAYS = Array.from({ length: 28 }, (_, i) => [i + 1, `The ${ordinal(i + 1)}`] as const)
const ASKS = [[0, 'Never'], [50, '50 MB'], [100, '100 MB'], [250, '250 MB']] as const

onMounted(loadSyncSettings)

// Not wired yet: "Nothing" and the limits live only here.
const nothing = ref(false)
const LIMITS_DEFAULT = { limit: 0, monthStarts: 1, askOver: 100 }
const limits = reactive({ ...LIMITS_DEFAULT })
// "Reset to default" for a limit (local only, like the limits).
const resetLimit = (key: keyof typeof LIMITS_DEFAULT) => ({
  setting: `mobile.${key}`,
  reset: () => (limits[key] = LIMITS_DEFAULT[key]),
  isDefault: limits[key] === LIMITS_DEFAULT[key],
})

const level = computed<Level>(() => (nothing.value ? 'nothing' : syncSettings.value?.copyOnMetered ? 'all' : 'small'))
function pick(next: Level) {
  nothing.value = next === 'nothing'
  if (next !== 'nothing') saveSyncSettings({ copyOnMetered: next === 'all' })
}
</script>

<template>
  <SettingsHeading
    title="Mobile data"
    mark="Phones"
    description="Settings controlling when to sync on mobile data. They apply on your phones, and can be changed from any device."
  />
  <SettingsCard v-if="syncSettings.value">
    <SettingsRow :description="LEVEL_LINE[level]" wide setting="mobile.level" :reset="() => pick('small')" :is-default="level === 'small'">
      <template #title>On mobile data, sync <SettingsBadge v-if="level === 'nothing'" class="ml-1.5">Not wired yet</SettingsBadge></template>
      <Segmented :model-value="level" :options="LEVELS" @update:model-value="pick" />
    </SettingsRow>
    <Reveal :show="level === 'all'">
      <SettingsRow description="Per month; then songs wait for Wi-Fi" detail v-bind="resetLimit('limit')">
        <template #title>Limit for songs <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></template>
        <DropValue v-model="limits.limit" :options="LIMITS" />
      </SettingsRow>
      <SettingsRow description="Your plan’s billing day" detail v-bind="resetLimit('monthStarts')">
        <template #title>Month starts on <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></template>
        <DropValue v-model="limits.monthStarts" :options="DAYS" />
      </SettingsRow>
      <SettingsRow description="One sync’s songs, on mobile data" detail v-bind="resetLimit('askOver')">
        <template #title>Ask before a sync over <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></template>
        <DropValue v-model="limits.askOver" :options="ASKS" />
      </SettingsRow>
    </Reveal>
    <!-- The month's use: the phone's own; a desktop shows what the phones sent, by each phone's own name once wired. -->
    <div class="settings-row grid gap-2.5" data-row>
      <div class="flex items-baseline justify-between gap-3 text-sm">
        <span>{{ isMobile ? 'This month on mobile data' : 'Your phones, this month' }} <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></span>
        <span><b class="text-[17px] font-bold">0 MB</b><small v-if="limits.limit && level === 'all'" class="text-muted-foreground"> of {{ limits.limit >= 1000 ? `${limits.limit / 1000} GB` : `${limits.limit} MB` }}</small></span>
      </div>
      <div class="h-1 overflow-hidden rounded-full bg-well shadow-[inset_0_0_0_1px_var(--well-line)]"><div class="h-full w-0 rounded-full bg-primary" /></div>
      <small class="text-[13px] text-muted-foreground">
        {{ isMobile ? 'Counted by Melue for its own syncing' : 'From your phones (each sends it when it syncs, and when Melue closes there)' }} · the month starts on the {{ ordinal(limits.monthStarts) }}
      </small>
    </div>
  </SettingsCard>
</template>
