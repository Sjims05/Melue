<script setup lang="ts">
/**
 * Shortcuts that work anywhere (desktop: src-tauri shortcuts.rs): Melue's volume, play / pause, next and previous from
 * any app, also with Melue in the tray. This computer's own; on by default (Ctrl+Alt with the arrows and Space). Keys
 * another app has: said under the card, the rest still work. A shortcut's row: right-click resets its keys.
 */
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { Switch } from '@/components/ui/switch'
import { Reveal, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { onPageLoad } from '@/components/page/pageLoading'
import ShortcutKeys from './ShortcutKeys.vue'

type Shortcuts = { on: boolean; keys: Record<string, string> }
type Now = { shortcuts: Shortcuts; taken: string[]; defaults: Record<string, string> }

const ACTIONS = [
  ['volumeUp', 'Volume up'],
  ['volumeDown', 'Volume down'],
  ['playPause', 'Play / pause'],
  ['next', 'Next song'],
  ['previous', 'Previous song'],
] as const

const now = ref<Now | null>(null)
const error = ref('')

onPageLoad(async () => {
  now.value = await invoke<Now>('shortcuts_get').catch(() => null)
})

async function save(shortcuts: Shortcuts) {
  error.value = ''
  try {
    now.value = await invoke<Now>('shortcuts_set', { shortcuts })
  } catch (e) {
    error.value = String(e)
  }
}
const setOn = (on: boolean) => now.value && save({ ...now.value.shortcuts, on })
const setKeys = (action: string, keys: string) => now.value && save({ ...now.value.shortcuts, keys: { ...now.value.shortcuts.keys, [action]: keys } })
const takenNames = () => ACTIONS.filter(([a]) => now.value?.taken.includes(a)).map(([, name]) => name)
</script>

<template>
  <template v-if="now">
    <SettingsLabel mark="This device">Shortcuts anywhere</SettingsLabel>
    <SettingsCard>
      <SettingsRow
        title="Shortcuts anywhere"
        description="Melue’s volume and player from any app, also with Melue in the tray"
        setting="shortcuts.on"
        :reset="() => setOn(true)"
        :is-default="now.shortcuts.on"
      >
        <Switch :model-value="now.shortcuts.on" @update:model-value="setOn" />
      </SettingsRow>
      <Reveal :show="now.shortcuts.on">
        <SettingsRow
          v-for="[action, name] in ACTIONS"
          :key="action"
          :title="name"
          :setting="`shortcuts.${action}`"
          :reset="() => setKeys(action, now!.defaults[action] ?? '')"
          :is-default="now.shortcuts.keys[action] === now.defaults[action]"
        >
          <ShortcutKeys :keys="now.shortcuts.keys[action] ?? ''" :label="name" @change="(keys) => setKeys(action, keys)" />
        </SettingsRow>
      </Reveal>
    </SettingsCard>
    <Transition name="drop">
      <SettingsTip v-if="now.shortcuts.on && now.taken.length" kind="bad" class="mt-3">
        Another app has the keys for {{ takenNames().join(', ') }}: choose others for {{ now.taken.length === 1 ? 'it' : 'them' }}
      </SettingsTip>
    </Transition>
    <Transition name="drop">
      <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
    </Transition>
  </template>
</template>
