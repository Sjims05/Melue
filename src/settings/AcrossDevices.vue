<script setup lang="ts">
/**
 * Across devices: continuing what another device was playing (Ask / Follow / Off), when Ask may continue without
 * asking, and this device's name (what the others call it: "Continue from TROLLGANG"). Saves as you change it.
 *
 * Melue's playback sync settings (library/sync.ts: playbackSync). An empty name goes back to the system's name. Only
 * with a share: it all goes through the share.
 */
import { onMounted, ref } from 'vue'
import { Chevron, DropValue, EditRow, Reveal, Segmented, SettingsBadge, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { playbackSync, sync, type PlaybackMode, type PlaybackSettings } from '@/library/sync'
import { PLAYBACK_DEFAULTS } from './defaults'

const MODES = [['ask', 'Ask'], ['follow', 'Follow'], ['off', 'Off']] as const
const MODE_LINE: Record<PlaybackMode, string> = {
  ask: 'Asks before continuing what another device was playing',
  follow: 'Continues by itself, paused: no questions',
  off: 'Doesn’t continue from other devices',
}
// 0 is "Never" here; Melue keeps it as no limit (always ask).
const WITHIN = [[0, 'Never'], [5, '5 min'], [10, '10 min'], [30, '30 min'], [60, '1 hour']] as const

const settings = ref<PlaybackSettings | null>(null)
const editingName = ref(false)
const error = ref('')
const saving = ref(false)

onMounted(async () => {
  try {
    settings.value = await playbackSync.settings()
  } catch (e) {
    error.value = `Can’t read these settings: ${e}`
  }
})

async function save(change: Partial<PlaybackSettings>) {
  if (!settings.value) return
  const before = settings.value
  settings.value = { ...before, ...change }
  saving.value = true
  error.value = ''
  try {
    settings.value = await playbackSync.setSettings(settings.value)
    editingName.value = false
  } catch (e) {
    settings.value = before
    error.value = `Not saved: ${e}`
  } finally {
    saving.value = false
  }
}

// "Reset to default" for one setting. The name's default is the system's, which only Melue knows: always offered.
const resetPlayback = <K extends keyof PlaybackSettings>(key: K) => ({
  reset: () => save({ [key]: PLAYBACK_DEFAULTS[key] }),
  isDefault: key !== 'deviceName' && settings.value?.[key] === PLAYBACK_DEFAULTS[key],
})
</script>

<!-- No wrapper: the label keeps its space from what's above it. -->
<template>
  <template v-if="sync.available">
    <SettingsLabel>Across devices</SettingsLabel>
    <SettingsCard v-if="settings">
      <SettingsRow title="Playback across devices" :description="MODE_LINE[settings.mode]" wide v-bind="resetPlayback('mode')">
        <Segmented :model-value="settings.mode" :options="MODES" @update:model-value="(mode) => save({ mode })" />
      </SettingsRow>
      <Reveal :show="settings.mode === 'ask'">
        <SettingsRow title="Continue without asking if played within" description="Then it just continues, paused" detail v-bind="resetPlayback('continueWithinMinutes')">
          <DropValue
            :model-value="settings.continueWithinMinutes ?? 0"
            :options="WITHIN"
            @update:model-value="(minutes) => save({ continueWithinMinutes: minutes || null })"
          />
        </SettingsRow>
      </Reveal>
      <!-- The name's row stays: its line changes, and the field with Cancel and Save unfolds under it. -->
      <SettingsRow as="button" :aria-expanded="editingName" v-bind="resetPlayback('deviceName')" @click="editingName = !editingName">
        <template #title>This device’s name <SettingsBadge class="ml-1.5">This device</SettingsBadge></template>
        <template #description>
          <Transition name="swap" mode="out-in">
            <span :key="String(editingName)">{{ editingName ? 'How other devices see this one' : settings.deviceName }}</span>
          </Transition>
        </template>
        <template #end><Chevron :open="editingName" points="right" /></template>
      </SettingsRow>
      <Reveal :show="editingName" :line="false">
        <EditRow
          class="pt-0"
          :value="settings.deviceName ?? ''"
          :saving="saving"
          @save="(deviceName) => save({ deviceName })"
          @cancel="editingName = false"
        />
      </Reveal>
    </SettingsCard>
    <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  </template>
</template>
