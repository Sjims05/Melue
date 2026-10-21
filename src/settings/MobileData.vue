<script setup lang="ts">
/**
 * Mobile data: what syncs on mobile data, the limits for songs (with Everything), and how much was used this month
 * (src-tauri sync/mobile.rs). Shown on every device (settings sync), marked as being about phones: a phone shows its
 * own use, a computer the phones' (each sends it with its library data).
 */
import { computed, ref } from 'vue'
import { DropValue, resetTo, Reveal, Segmented, SettingsCard, SettingsHeading, SettingsRow } from '@/components/settings'
import { syncing, type DeviceMobileUse, type SyncSettings } from '@/library/sync'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { SYNC_DEFAULTS } from './defaults'
import { size } from './format'
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
const AT_LIMIT = [['wait', 'Songs wait for Wi-Fi'], ['ask', 'Ask me']] as const

// The month's use: this phone's, or (a computer) the phones'.
const used = ref<DeviceMobileUse[]>([])
onPageLoad(async () => {
  await loadSyncSettings()
  used.value = await syncing.mobileUse().catch(() => [])
})

const s = computed(() => syncSettings.value)
const level = computed<Level>(() => (!s.value?.syncOnMetered ? 'nothing' : s.value.copyOnMetered ? 'all' : 'small'))
const pick = (next: Level) => saveSyncSettings({ syncOnMetered: next !== 'nothing', copyOnMetered: next === 'all' })
const atLimit = computed(() => (s.value?.mobileAskAtLimit ? 'ask' : 'wait'))
const pickAtLimit = (next: 'wait' | 'ask') => saveSyncSettings({ mobileAskAtLimit: next === 'ask' })

// "Reset to default" for a limit row (its id is the Mobile data setting's: mobile.limit).
const resetMobile = <K extends keyof SyncSettings>(id: string, key: K) => ({
  setting: `mobile.${id}`,
  reset: resetTo(() => s.value?.[key] ?? SYNC_DEFAULTS[key], (value) => saveSyncSettings({ [key]: value }), SYNC_DEFAULTS[key]),
  isDefault: s.value?.[key] === SYNC_DEFAULTS[key],
})
const field = <K extends keyof SyncSettings>(key: K) =>
  computed({ get: () => s.value?.[key] ?? SYNC_DEFAULTS[key], set: (value) => saveSyncSettings({ [key]: value }) })
const limit = field('mobileLimitMb')
const monthStarts = field('mobileMonthStarts')
const askOver = field('mobileAskOverMb')

const limitBytes = computed(() => (level.value === 'all' && limit.value ? limit.value * 1_000_000 : 0))
const share = (bytes: number) => (limitBytes.value ? Math.min(1, bytes / limitBytes.value) : 0)
</script>

<template>
  <SettingsHeading
    title="Mobile data"
    mark="Phones"
    description="Settings controlling when to sync on mobile data. They apply on your phones, and can be changed from any device."
  />
  <SettingsCard v-if="s">
    <SettingsRow title="On mobile data, sync" :description="LEVEL_LINE[level]" wide setting="mobile.level" :reset="resetTo(() => level, pick, 'small')" :is-default="level === 'small'">
      <Segmented :model-value="level" :options="LEVELS" @update:model-value="pick" />
    </SettingsRow>
    <Reveal :show="level === 'all'">
      <SettingsRow title="Limit for songs" description="Per month; then songs wait for Wi-Fi, or Melue asks" detail v-bind="resetMobile('limit', 'mobileLimitMb')">
        <DropValue v-model="limit" :options="LIMITS" />
      </SettingsRow>
      <Reveal :show="limit > 0">
        <SettingsRow title="When the limit is reached" detail wide v-bind="resetMobile('atLimit', 'mobileAskAtLimit')">
          <Segmented :model-value="atLimit" :options="AT_LIMIT" @update:model-value="pickAtLimit" />
        </SettingsRow>
      </Reveal>
      <SettingsRow title="Month starts on" description="Your plan’s billing day" detail v-bind="resetMobile('monthStarts', 'mobileMonthStarts')">
        <DropValue v-model="monthStarts" :options="DAYS" />
      </SettingsRow>
      <SettingsRow title="Ask before a sync over" description="One sync’s songs, when it starts by itself" detail v-bind="resetMobile('askOver', 'mobileAskOverMb')">
        <DropValue v-model="askOver" :options="ASKS" />
      </SettingsRow>
    </Reveal>
    <!-- The month's use: the phone's own; a computer shows what each phone sent. -->
    <div class="settings-row grid gap-2.5" data-row>
      <div class="flex items-baseline justify-between gap-3 text-sm">
        <span>{{ isMobile ? 'This month on mobile data' : 'Your phones, this month' }}</span>
        <span v-if="isMobile">
          <b class="text-[17px] font-bold">{{ size(used[0]?.bytes ?? 0) }}</b>
          <small v-if="limitBytes" class="text-muted-foreground"> of {{ size(limitBytes) }}</small>
        </span>
      </div>
      <template v-if="isMobile">
        <div class="h-1 overflow-hidden rounded-full bg-well shadow-[inset_0_0_0_1px_var(--well-line)]">
          <div class="h-full rounded-full bg-primary transition-[width] duration-300" :style="{ width: `${share(used[0]?.bytes ?? 0) * 100}%` }" />
        </div>
      </template>
      <template v-else>
        <div v-for="phone in used" :key="phone.device" class="grid gap-1.5">
          <div class="flex items-baseline justify-between gap-3 text-[13.5px]">
            <span class="truncate">{{ phone.device }}</span>
            <span><b class="font-semibold">{{ size(phone.bytes) }}</b><small v-if="limitBytes" class="text-muted-foreground"> of {{ size(limitBytes) }}</small></span>
          </div>
          <div class="h-1 overflow-hidden rounded-full bg-well shadow-[inset_0_0_0_1px_var(--well-line)]">
            <div class="h-full rounded-full bg-primary" :style="{ width: `${share(phone.bytes) * 100}%` }" />
          </div>
        </div>
        <span v-if="!used.length" class="text-[13.5px] text-muted-foreground">No phone has synced on mobile data this month</span>
      </template>
      <small class="text-[13px] text-muted-foreground">
        {{ isMobile ? 'Songs Melue copied on mobile data' : 'Each phone sends it when it syncs' }} · the month starts on the {{ ordinal(monthStarts) }}
      </small>
    </div>
  </SettingsCard>
</template>
