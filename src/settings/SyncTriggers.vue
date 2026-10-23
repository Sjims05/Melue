<script setup lang="ts">
/**
 * When Melue syncs by itself: at start, when the share's music changes, coming back after a while, and every so
 * often (the design's rows, texts and order). The tip shows only without a share.
 */
import { Switch } from '@/components/ui/switch'
import { Chips, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { sync } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { loadSyncSettings, resetSync, saveSyncSettings, syncSettings } from './syncSettings'

const emit = defineEmits<{ setUp: [] }>()

// 0 is "Never" here; Melue keeps it as none.
const RETURN = [[0, 'Never'], [5, '5 min'], [15, '15 min'], [60, '1 hour']] as const
const EVERY = [[0, 'Never'], [15, '15 min'], [30, '30 min'], [60, '1 hour']] as const

onPageLoad(loadSyncSettings)
</script>

<template>
  <Transition name="drop">
    <SettingsTip v-if="!sync.available">
      No share set up yet: these settings start working once you set one up.
      <template #action><button type="button" class="text-[13px] font-medium underline underline-offset-3" @click="emit('setUp')">Set one up</button></template>
    </SettingsTip>
  </Transition>
  <SettingsLabel>When it syncs</SettingsLabel>
  <SettingsCard v-if="syncSettings.value">
    <SettingsRow title="Sync when Melue starts" description="Gets new music as soon as you open it" v-bind="resetSync('atStart')">
      <Switch :model-value="syncSettings.value.atStart" @update:model-value="(atStart) => saveSyncSettings({ atStart })" />
    </SettingsRow>
    <SettingsRow title="Sync on share changes" description="When the songs on the share change" v-bind="resetSync('onChange')">
      <Switch :model-value="syncSettings.value.onChange" @update:model-value="(onChange) => saveSyncSettings({ onChange })" />
    </SettingsRow>
    <SettingsRow title="Sync when coming back after…" description="When you reopen an inactive instance" wide v-bind="resetSync('onReturnAfterMinutes')">
      <Chips
        :model-value="syncSettings.value.onReturnAfterMinutes ?? 0"
        :options="RETURN"
        @update:model-value="(minutes) => saveSyncSettings({ onReturnAfterMinutes: minutes || null })"
      />
    </SettingsRow>
    <SettingsRow title="Sync share interval" description="The interval between app syncs from the share" wide v-bind="resetSync('everyMinutes')">
      <Chips
        :model-value="syncSettings.value.everyMinutes ?? 0"
        :options="EVERY"
        @update:model-value="(minutes) => saveSyncSettings({ everyMinutes: minutes || null })"
      />
    </SettingsRow>
  </SettingsCard>
</template>
