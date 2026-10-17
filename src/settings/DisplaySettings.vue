<script setup lang="ts">
/**
 * Display: the theme (System, Dark, Light, or Adaptive: dark and light by the clock) and where Melue opens; on desktop (this device's own), starting with the system (in the tray)
 * and what closing the window does (src-tauri: desktop.rs). The theme is kept on this device for now (lib/theme.ts);
 * it syncs across devices once settings sync.
 *
 * Not wired yet: Start on (Melue's pages aren't there yet).
 */
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { computed, ref } from 'vue'
import { Switch } from '@/components/ui/switch'
import { DropValue, Reveal, Segmented, SettingsBadge, SettingsCard, SettingsHeading, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library } from '@/library'
import { APPEARANCE_DEFAULTS, appearance } from '@/lib/appearance'
import { THEME_TIMES_DEFAULT, themeMode, themeTimes, type ThemeMode } from '@/lib/theme'
import { isDesktop } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'

const THEMES = [['system', 'System'], ['dark', 'Dark'], ['light', 'Light'], ['adaptive', 'Adaptive']] as const
const themeLine = (mode: ThemeMode) =>
  ({
    system: 'Light or dark, as your system is set',
    dark: 'Always dark',
    light: 'Always light',
    adaptive: `Dark from ${themeTimes.dark}, light from ${themeTimes.light}`,
  })[mode]
// Adaptive's times: every half hour.
const TIMES = Array.from({ length: 48 }, (_, i) => {
  const time = `${String(Math.floor(i / 2)).padStart(2, '0')}:${i % 2 ? '30' : '00'}`
  return [time, time] as const
})
const STARTS = [['home', 'Home'], ['library', 'Library'], ['last', 'Where I left off']] as const
const WHITE = [['white', 'White'], ['card', 'Like the cards']] as const
// Up next and Home's cards: white (in dark mode too) or like the other cards.
const upNext = computed({ get: () => (appearance.upNextWhite ? 'white' : 'card'), set: (v) => (appearance.upNextWhite = v === 'white') })
const homeCards = computed({ get: () => (appearance.homeCardsWhite ? 'white' : 'card'), set: (v) => (appearance.homeCardsWhite = v === 'white') })

type Close = 'tray' | 'quit'
const CLOSES = [['tray', 'Keep playing'], ['quit', 'Quit']] as const
const CLOSE_LINE: Record<Close, string> = {
  tray: 'Melue keeps playing in the tray; quit from its menu there',
  quit: 'Closing the window quits Melue: the music stops',
}
// Read by desktop.rs when the window closes.
const CLOSE_SETTING = 'close_window'

// Not wired yet: kept only here.
const startOn = ref<(typeof STARTS)[number][0]>('home')
const autostart = ref<boolean | null>(null)
const close = ref<Close>('tray')
const error = ref('')

onPageLoad(async () => {
  if (!isDesktop) return
  try {
    autostart.value = await isEnabled()
    close.value = (await library.setting(CLOSE_SETTING)) === 'quit' ? 'quit' : 'tray'
  } catch (e) {
    error.value = `Can’t read these settings: ${e}`
  }
})

async function setAutostart(on: boolean) {
  error.value = ''
  try {
    await (on ? enable() : disable())
    autostart.value = await isEnabled()
  } catch (e) {
    error.value = `Not changed: ${e}`
  }
}

async function setClose(mode: Close) {
  const before = close.value
  close.value = mode
  error.value = ''
  try {
    await library.setSetting(CLOSE_SETTING, mode)
  } catch (e) {
    close.value = before
    error.value = `Not saved: ${e}`
  }
}
</script>

<template>
  <SettingsHeading title="Display" description="Settings controlling general display and behavior" />
  <SettingsCard>
    <SettingsRow title="Theme" wide setting="display.theme" :reset="() => (themeMode = 'system')" :is-default="themeMode === 'system'">
      <template #description>
        <Transition name="swap" mode="out-in"><span :key="themeMode">{{ themeLine(themeMode) }}</span></Transition>
      </template>
      <Segmented v-model="themeMode" :options="THEMES" />
    </SettingsRow>
    <Reveal :show="themeMode === 'adaptive'">
      <SettingsRow
        title="Dark from"
        detail
        setting="display.darkFrom"
        :reset="() => (themeTimes.dark = THEME_TIMES_DEFAULT.dark)"
        :is-default="themeTimes.dark === THEME_TIMES_DEFAULT.dark"
      >
        <DropValue v-model="themeTimes.dark" :options="TIMES" />
      </SettingsRow>
      <SettingsRow
        title="Light from"
        detail
        setting="display.lightFrom"
        :reset="() => (themeTimes.light = THEME_TIMES_DEFAULT.light)"
        :is-default="themeTimes.light === THEME_TIMES_DEFAULT.light"
      >
        <DropValue v-model="themeTimes.light" :options="TIMES" />
      </SettingsRow>
    </Reveal>
    <SettingsRow
      title="Up next on Now playing"
      description="The queue under the player"
      wide
      setting="display.upNextWhite"
      :reset="() => (appearance.upNextWhite = APPEARANCE_DEFAULTS.upNextWhite)"
      :is-default="appearance.upNextWhite === APPEARANCE_DEFAULTS.upNextWhite"
    >
      <Segmented v-model="upNext" :options="WHITE" />
    </SettingsRow>
    <SettingsRow
      title="Home’s Playing and Continue cards"
      wide
      setting="display.homeCardsWhite"
      :reset="() => (appearance.homeCardsWhite = APPEARANCE_DEFAULTS.homeCardsWhite)"
      :is-default="appearance.homeCardsWhite === APPEARANCE_DEFAULTS.homeCardsWhite"
    >
      <Segmented v-model="homeCards" :options="WHITE" />
    </SettingsRow>
    <SettingsRow description="Where Melue opens" setting="display.startOn" :reset="() => (startOn = 'home')" :is-default="startOn === 'home'">
      <template #title>Start on <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></template>
      <DropValue v-model="startOn" :options="STARTS" />
    </SettingsRow>
  </SettingsCard>

  <template v-if="isDesktop">
    <SettingsLabel mark="This device">On this PC</SettingsLabel>
    <SettingsCard>
      <SettingsRow
        v-if="autostart !== null"
        title="Start Melue with Windows"
        setting="display.autostart"
        description="In the tray, ready for the media keys"
        :reset="() => setAutostart(true)"
        :is-default="autostart"
      >
        <Switch :model-value="autostart" @update:model-value="setAutostart" />
      </SettingsRow>
      <SettingsRow title="Closing the window" wide setting="display.closeWindow" :reset="() => setClose('tray')" :is-default="close === 'tray'">
        <template #description>
          <Transition name="swap" mode="out-in"><span :key="close">{{ CLOSE_LINE[close] }}</span></Transition>
        </template>
        <Segmented :model-value="close" :options="CLOSES" @update:model-value="setClose" />
      </SettingsRow>
    </SettingsCard>
  </template>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
