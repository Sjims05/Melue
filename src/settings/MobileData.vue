<script setup lang="ts">
/**
 * Mobile data: what syncs on mobile data, the limits for songs (with Everything), and how much was used this month
 * (src-tauri sync/mobile.rs). Shown on every device (settings sync), marked as being about phones: a phone shows its
 * own use, a computer the phones' (each sends it with its library data).
 */
import { computed, ref } from 'vue'
import { resetTo, Reveal, Segmented, SettingsCard, SettingsHeading, SettingsRow } from '@/components/settings'
import { syncing, type DeviceMobileUse } from '@/library/sync'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { SYNC_DEFAULTS } from './defaults'
import MobileLimits from './MobileLimits.vue'
import { ordinal, size } from './format'
import { loadSyncSettings, saveSyncSettings, syncSettings } from './syncSettings'

type Level = 'nothing' | 'small' | 'all'
const LEVELS = [['nothing', 'Nothing'], ['small', 'Small things'], ['all', 'Everything']] as const
const LEVEL_LINE: Record<Level, string> = {
  nothing: 'Wait for Wi-Fi for everything',
  small: 'Playback, likes and playlists; songs wait for Wi-Fi',
  all: 'Songs too, within the limits below',
}

// The month's use: this phone's, or (a computer) the phones'.
const used = ref<DeviceMobileUse[]>([])
onPageLoad(async () => {
  await loadSyncSettings()
  used.value = await syncing.mobileUse().catch(() => [])
})

const s = computed(() => syncSettings.value)
const level = computed<Level>(() => (!s.value?.syncOnMetered ? 'nothing' : s.value.copyOnMetered ? 'all' : 'small'))
const pick = (next: Level) => saveSyncSettings({ syncOnMetered: next !== 'nothing', copyOnMetered: next === 'all' })
const limit = computed(() => s.value?.mobileLimitMb ?? SYNC_DEFAULTS.mobileLimitMb)
const monthStarts = computed(() => s.value?.mobileMonthStarts ?? SYNC_DEFAULTS.mobileMonthStarts)

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
    <Reveal :show="level === 'all'"><MobileLimits /></Reveal>
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
