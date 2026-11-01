<script setup lang="ts">
/**
 * Mobile data's limits for songs (with Everything): the month's limit, what happens at it, the billing day, and when
 * to ask before a sync. Rows in the card it's put in (MobileData.vue; the first-start guide). `pretend`: as on a new
 * device (Melue Dev's guide): the choices are kept here only, nothing saved.
 */
import { computed, reactive } from 'vue'
import { DropValue, resetTo, Reveal, Segmented, SettingsRow } from '@/components/settings'
import type { SyncSettings } from '@/library/sync'
import { SYNC_DEFAULTS } from './defaults'
import { ordinal } from './format'
import { saveSyncSettings, syncSettings } from './syncSettings'

const props = defineProps<{ pretend?: boolean }>()

const LIMITS = [[0, 'No limit'], [250, '250 MB'], [500, '500 MB'], [1000, '1 GB'], [2000, '2 GB']] as const
const DAYS = Array.from({ length: 28 }, (_, i) => [i + 1, `The ${ordinal(i + 1)}`] as const)
const ASKS = [[0, 'Never'], [50, '50 MB'], [100, '100 MB'], [250, '250 MB']] as const
const AT_LIMIT = [['wait', 'Songs wait for Wi-Fi'], ['ask', 'Ask me']] as const

// As a new device: the defaults, changed here only.
const pretended = reactive<Partial<SyncSettings>>({})
const value = <K extends keyof SyncSettings>(key: K): SyncSettings[K] =>
  ((props.pretend ? pretended[key] : syncSettings.value?.[key]) ?? SYNC_DEFAULTS[key]) as SyncSettings[K]
const save = (patch: Partial<SyncSettings>) => (props.pretend ? Object.assign(pretended, patch) : saveSyncSettings(patch))

const atLimit = computed(() => (value('mobileAskAtLimit') ? 'ask' : 'wait'))
const pickAtLimit = (next: 'wait' | 'ask') => save({ mobileAskAtLimit: next === 'ask' })

// "Reset to default" for a limit row (its id is the Mobile data setting's: mobile.limit).
const resetMobile = <K extends keyof SyncSettings>(id: string, key: K) => ({
  setting: `mobile.${id}`,
  reset: resetTo(() => value(key), (v) => save({ [key]: v }), SYNC_DEFAULTS[key]),
  isDefault: value(key) === SYNC_DEFAULTS[key],
})
const field = <K extends keyof SyncSettings>(key: K) => computed({ get: () => value(key), set: (v) => save({ [key]: v }) })
const limit = field('mobileLimitMb')
const monthStarts = field('mobileMonthStarts')
const askOver = field('mobileAskOverMb')
</script>

<template>
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
</template>
