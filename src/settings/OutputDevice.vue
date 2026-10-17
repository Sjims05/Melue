<script setup lang="ts">
/**
 * Where this computer plays (desktop): System default first (it's the default, and what plays while the chosen device
 * is gone), then the devices. A chosen device that's unplugged stays in the list, chosen but grey, with a tip that it
 * plays on the default until it's back (the player goes back to it by itself).
 *
 * The player saves the choice. It only lists the devices that are there, so this remembers the chosen one's name (for
 * when it's unplugged). Devices come and go without an event: the list is read again every few seconds while shown.
 */
import { Unplug } from '@lucide/vue'
import { useDocumentVisibility, useIntervalFn } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'
import { RadioMark, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { player, type OutputDevices } from '@/platform'

const NAMES = 'melue-output-names'

const outputs = ref<OutputDevices | null>(null)
const error = ref('')

function remembered(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(NAMES) ?? '{}')
  } catch {
    return {}
  }
}
function remember(id: string, name: string) {
  try {
    localStorage.setItem(NAMES, JSON.stringify({ ...remembered(), [id]: name }))
  } catch {
    // Without storage, an unplugged device shows as "Your chosen device".
  }
}

function show(next: OutputDevices) {
  outputs.value = next
  const chosen = next.devices.find((d) => d.id === next.selected)
  if (chosen) remember(chosen.id, chosen.name)
}

const read = () => player.getOutputDevices().then(show).catch((e) => (error.value = `Can’t list the output devices: ${e}`))
onMounted(read)
const visible = useDocumentVisibility()
const { pause, resume } = useIntervalFn(read, 3000)
watch(visible, (now) => {
  if (now !== 'visible') return pause()
  read()
  resume()
})

async function choose(id: string | null) {
  if (!outputs.value || outputs.value.selected === id) return
  error.value = ''
  try {
    show(await player.setOutputDevice(id))
  } catch (e) {
    error.value = `Can’t play there: ${e}`
  }
}

const systemDefault = computed(() => outputs.value?.devices.find((d) => d.isDefault))
/** The chosen device, when it isn't there (unplugged). */
const missing = computed(() => {
  const selected = outputs.value?.selected
  if (!selected || outputs.value?.devices.some((d) => d.id === selected)) return null
  return { id: selected, name: remembered()[selected] ?? 'Your chosen device' }
})
// Reset to default: back to System default.
const reset = computed(() => ({ setting: 'output.device', reset: () => choose(null), isDefault: outputs.value?.selected === null }))
</script>

<template>
  <template v-if="outputs && outputs.devices.length > 0">
    <SettingsLabel mark="This device">Output device</SettingsLabel>
    <SettingsTip v-if="missing" :icon="Unplug" class="mb-3">
      {{ missing.name }} not connected: <b>playing on System default</b><template v-if="systemDefault"> ({{ systemDefault.name }})</template> until it’s back
    </SettingsTip>
    <SettingsCard>
      <SettingsRow
        as="button"
        title="System default"
        :description="`Follows the system’s choice${systemDefault ? ` · now ${systemDefault.name}` : ''}`"
        v-bind="reset"
        @click="choose(null)"
      >
        <template #end><RadioMark :on="outputs.selected === null" /></template>
      </SettingsRow>
      <SettingsRow v-if="missing" :title="missing.name" description="Not connected · Melue goes back to it when it’s plugged in" dim v-bind="reset">
        <template #end><RadioMark on /></template>
      </SettingsRow>
      <SettingsRow v-for="device in outputs.devices" :key="device.id" as="button" :title="device.name" v-bind="reset" @click="choose(device.id)">
        <template #end><RadioMark :on="outputs.selected === device.id" /></template>
      </SettingsRow>
    </SettingsCard>
  </template>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
