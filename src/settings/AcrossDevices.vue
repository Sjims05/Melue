<script setup lang="ts">
/**
 * Across devices: continuing what another device was playing (Ask / Follow / Off), when Ask may continue without
 * asking, and this device's name (what the others call it: "Continue from TROLLGANG"). Saves as you change it. Then
 * your devices (library/remote.ts): which are online now (an old one can be forgotten), whether the others may play on this one, and the volume's
 * steps when playing on another (a phone's; on a phone, its buttons' on a computer).
 *
 * Melue's playback sync settings (library/sync.ts: playbackSync). An empty name goes back to the system's name. Only
 * with a share: it all goes through the share.
 */
import { Ellipsis, Monitor, Smartphone } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { ref } from 'vue'
import { Chevron, ConfirmDialog, DropValue, EditRow, IconTile, resetTo, Reveal, Segmented, SettingsBadge, SettingsCard, SettingsLabel, SettingsRow, SettingsTip, StatusDot, Stepper } from '@/components/settings'
import { usePhone } from '@/lib/phone'
import { playingOn } from '@/player/playingOn'
import { remote, type Allow, type Device, type RemoteSettings } from '@/library/remote'
import { ago } from './format'
import { playbackSync, sync, type PlaybackMode, type PlaybackSettings } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { PLAYBACK_DEFAULTS } from './defaults'
import { everyWhileShown } from '@/lib/whileShown'

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

// Your devices, looked at again every 15 s while this is shown (a device that went away without saying so shows offline within a few minutes).
const devices = ref<Device[]>([])
const remoteSettings = ref<RemoteSettings | null>(null)
const loadDevices = async () => (devices.value = await remote.devices().catch(() => devices.value))
everyWhileShown(loadDevices, 15_000)
const ALLOWS = [['on', 'On'], ['ask', 'Ask first'], ['off', 'Off']] as const
const ALLOW_LINE: Record<Allow, string> = {
  on: 'Your other devices can play their music on this one',
  ask: 'Someone here says yes first',
  off: 'Your other devices can’t play on this one',
}
// A device that never said where it can be reached: an older Melue (or one removed since), known from its library data.
const older = (d: Device) => !d.this && d.port == null && d.addresses.length === 0
const deviceLine = (d: Device) =>
  [
    d.online ? 'Online now' : `Last seen ${ago(d.lastSeenMs)}`,
    older(d) ? 'an older Melue' : d.allow === 'off' ? 'doesn’t let others play on it' : d.allow === 'ask' ? 'asks first' : '',
  ]
    .filter(Boolean)
    .join(' · ')

const phone = usePhone()
const PHONE_STEPS = [1, 2, 5, 10, 20] as const
const BUTTON_STEPS = [1, 2, 5, 10] as const

async function setRemote(change: Partial<RemoteSettings>) {
  if (!remoteSettings.value) return
  try {
    remoteSettings.value = await remote.setSettings({ ...remoteSettings.value, ...change })
    playingOn.phoneStep = remoteSettings.value.phoneStep
    if (change.allow) await loadDevices()
  } catch (e) {
    error.value = `Not saved: ${e}`
  }
}
const setAllow = (allow: Allow) => setRemote({ allow })

// An old device (a Melue removed, one not used any more): forgotten, it isn't counted or offered anywhere.
const forgetting = ref<Device | null>(null)
async function forget() {
  const device = forgetting.value
  if (!device) return
  try {
    await remote.forget(device.id)
    devices.value = devices.value.filter((d) => d.id !== device.id)
  } catch (e) {
    error.value = `Can’t forget ${device.name}: ${e}`
  }
}

onPageLoad(async () => {
  try {
    ;[settings.value, remoteSettings.value] = await Promise.all([playbackSync.settings(), remote.settings()])
  } catch (e) {
    error.value = `Can’t read these settings: ${e}`
  }
  await loadDevices()
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
  setting: `playback.${key}`,
  reset: resetTo(() => settings.value?.[key] ?? PLAYBACK_DEFAULTS[key], (value) => save({ [key]: value }), PLAYBACK_DEFAULTS[key]),
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
      <Reveal :show="settings.mode !== 'off'">
        <SettingsRow
          title="Songs this device doesn’t have"
          :description="settings.songsNotHere ? 'Continued anyway: play starts the next song here' : 'Not continued'"
          detail
          v-bind="resetPlayback('songsNotHere')"
        >
          <Switch :model-value="settings.songsNotHere" @update:model-value="(songsNotHere) => save({ songsNotHere })" />
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
      <SettingsRow
        v-if="remoteSettings"
        title="Let other devices play here"
        :description="ALLOW_LINE[remoteSettings.allow]"
        wide
        setting="remote.allow"
        :reset="() => setAllow('on')"
        :is-default="remoteSettings.allow === 'on'"
      >
        <Segmented :model-value="remoteSettings.allow" :options="ALLOWS" @update:model-value="setAllow" />
      </SettingsRow>
      <SettingsRow
        v-if="remoteSettings"
        title="Volume steps on a phone"
        :description="`Playing on a phone, each step moves its volume ${remoteSettings.phoneStep === 1 ? '1 level' : `${remoteSettings.phoneStep} levels`}`"
        setting="remote.phoneStep"
        :reset="() => setRemote({ phoneStep: 10 })"
        :is-default="remoteSettings.phoneStep === 10"
      >
        <Stepper :model-value="remoteSettings.phoneStep" :steps="[...PHONE_STEPS]" @update:model-value="(phoneStep) => setRemote({ phoneStep })" />
      </SettingsRow>
      <SettingsRow
        v-if="remoteSettings && phone"
        title="Volume buttons on another device"
        :description="`Playing on a computer, each press moves its volume ${remoteSettings.buttonStep}%`"
        setting="remote.buttonStep"
        :reset="() => setRemote({ buttonStep: 5 })"
        :is-default="remoteSettings.buttonStep === 5"
      >
        <Stepper :model-value="remoteSettings.buttonStep" :steps="[...BUTTON_STEPS]" @update:model-value="(buttonStep) => setRemote({ buttonStep })" />
      </SettingsRow>
    </SettingsCard>
    <!-- A device that turns up drops in; one coming online moves up to the others. -->
    <Transition name="drop">
      <!-- The label's space above it (it has none as the first in here). -->
      <div v-if="devices.length" class="mt-5">
        <SettingsLabel>Your devices</SettingsLabel>
        <SettingsCard>
          <!-- Each row in a plain box the list can animate (a row's own root is its right-click menu), separated like
               rows (settings-reveal). -->
          <TransitionGroup name="drop">
            <div v-for="d in devices" :key="d.id" class="settings-reveal">
              <SettingsRow :description="deviceLine(d)">
                <template #lead><IconTile :icon="d.kind === 'phone' ? Smartphone : Monitor" /></template>
                <template #title>{{ d.name }} <SettingsBadge v-if="d.this" class="ml-1.5">This device</SettingsBadge></template>
                <StatusDot :kind="d.online ? 'ok' : 'off'" />
                <DropdownMenu v-if="!d.this && !d.online">
                  <DropdownMenuTrigger as-child>
                    <Button variant="ghost" size="icon-sm" :aria-label="`${d.name}: more`"><Ellipsis /></Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem variant="destructive" @select="forgetting = d">Forget…</DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </SettingsRow>
            </div>
          </TransitionGroup>
        </SettingsCard>
      </div>
    </Transition>
    <Transition name="drop">
      <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
    </Transition>
    <ConfirmDialog
      :open="!!forgetting"
      :title="`Forget ${forgetting?.name ?? 'this device'}?`"
      text="Its files on your share go: it isn’t counted among your devices any more (for songs on all your devices, mobile data, playing on it). The likes and plays it already shared stay. A device still in use comes back the next time it syncs."
      action="Forget"
      @update:open="(open) => !open && (forgetting = null)"
      @confirm="forget"
    />
  </template>
</template>
