<script setup lang="ts">
/**
 * Display: the theme (System, Dark, Light, or Adaptive: dark and light by the clock) and where Melue opens; on desktop (this device's own), starting with the system (in the tray)
 * and what closing the window does (src-tauri: desktop.rs). The theme is kept on this device for now (lib/theme.ts);
 * it syncs across devices once settings sync.
 *
 */
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { computed, ref } from 'vue'
import { Switch } from '@/components/ui/switch'
import { DropValue, resetTo, Reveal, Segmented, SettingsCard, SettingsHeading, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library } from '@/library'
import { APPEARANCE_DEFAULTS, appearance } from '@/lib/appearance'
import { THEME_TIMES_DEFAULT, themeMode, themeTimes, type ThemeMode } from '@/lib/theme'
import { isDesktop, isMobile } from '@/platform'
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
const PRIMARY = [['primary', 'Primary'], ['card', 'Like the cards']] as const
// Up next and Home's cards: in the primary colour (white in dark mode, black in light) or like the other cards.
const upNext = computed({ get: () => (appearance.upNextPrimary ? 'primary' : 'card'), set: (v) => (appearance.upNextPrimary = v === 'primary') })
const homeCards = computed({ get: () => (appearance.homeCardsPrimary ? 'primary' : 'card'), set: (v) => (appearance.homeCardsPrimary = v === 'primary') })

type Close = 'tray' | 'quit'
const CLOSES = [['tray', 'Keep playing'], ['quit', 'Quit']] as const
// Started by Windows: in the tray, or with its window (desktop.rs).
type AutostartWindow = 'tray' | 'window'
const AUTOSTART_WINDOW_SETTING = 'autostart_window'
const AUTOSTART_WINDOWS = [['tray', 'In the tray'], ['window', 'With its window']] as const
const CLOSE_LINE: Record<Close, string> = {
  tray: 'Melue keeps playing in the tray; quit from its menu there',
  quit: 'Closing the window quits Melue: the music stops',
}
// Read by desktop.rs when the window closes.
const CLOSE_SETTING = 'close_window'

const autostart = ref<boolean | null>(null)
// A development build can't be started with Windows (it needs the dev server for its page, and opens a console): the
// switch is locked there (the UI says nothing about it; it's the installed app's setting).
const devBuild = import.meta.env.DEV
const close = ref<Close>('tray')
const autostartWindow = ref<AutostartWindow>('tray')
const error = ref('')

onPageLoad(async () => {
  if (!isDesktop) return
  try {
    autostart.value = await isEnabled()
    close.value = (await library.setting(CLOSE_SETTING)) === 'quit' ? 'quit' : 'tray'
    autostartWindow.value = (await library.setting(AUTOSTART_WINDOW_SETTING)) === 'window' ? 'window' : 'tray'
  } catch (e) {
    error.value = `Can’t read these settings: ${e}`
  }
})

async function setAutostart(on: boolean) {
  // Never a development build (it would register the debug build to start with Windows).
  if (devBuild && on) return
  error.value = ''
  try {
    await (on ? enable() : disable())
    autostart.value = await isEnabled()
  } catch (e) {
    error.value = `Not changed: ${e}`
  }
}

async function setAutostartWindow(mode: AutostartWindow) {
  const before = autostartWindow.value
  autostartWindow.value = mode
  error.value = ''
  try {
    await library.setSetting(AUTOSTART_WINDOW_SETTING, mode)
  } catch (e) {
    autostartWindow.value = before
    error.value = `Not saved: ${e}`
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
      setting="display.upNextPrimary"
      :reset="() => (appearance.upNextPrimary = APPEARANCE_DEFAULTS.upNextPrimary)"
      :is-default="appearance.upNextPrimary === APPEARANCE_DEFAULTS.upNextPrimary"
    >
      <Segmented v-model="upNext" :options="PRIMARY" />
    </SettingsRow>
    <SettingsRow
      title="Home’s Playing and Continue cards"
      wide
      setting="display.homeCardsPrimary"
      :reset="() => (appearance.homeCardsPrimary = APPEARANCE_DEFAULTS.homeCardsPrimary)"
      :is-default="appearance.homeCardsPrimary === APPEARANCE_DEFAULTS.homeCardsPrimary"
    >
      <Segmented v-model="homeCards" :options="PRIMARY" />
    </SettingsRow>
    <SettingsRow
      title="Start on"
      description="Where Melue opens"
      setting="display.startOn"
      :reset="() => (appearance.startOn = APPEARANCE_DEFAULTS.startOn)"
      :is-default="appearance.startOn === APPEARANCE_DEFAULTS.startOn"
    >
      <DropValue v-model="appearance.startOn" :options="STARTS" />
    </SettingsRow>
    <SettingsRow
      v-if="isMobile"
      title="Hide the player when nothing is loaded"
      description="The mini player slides away until something plays"
      setting="display.hideEmptyMiniPlayer"
      :reset="() => (appearance.hideEmptyMiniPlayer = APPEARANCE_DEFAULTS.hideEmptyMiniPlayer)"
      :is-default="appearance.hideEmptyMiniPlayer === APPEARANCE_DEFAULTS.hideEmptyMiniPlayer"
    >
      <Switch v-model="appearance.hideEmptyMiniPlayer" />
    </SettingsRow>
  </SettingsCard>

  <template v-if="isDesktop">
    <SettingsLabel mark="This device">On this PC</SettingsLabel>
    <SettingsCard>
      <SettingsRow
        v-if="autostart !== null"
        title="Start Melue with Windows"
        setting="display.autostart"
        :description="autostartWindow === 'window' ? 'With its window open' : 'In the tray, ready for the media keys'"
        :reset="devBuild ? undefined : resetTo(() => !!autostart, setAutostart, true)"
        :is-default="autostart"
      >
        <Switch :model-value="autostart" :disabled="devBuild" @update:model-value="setAutostart" />
      </SettingsRow>
      <Reveal :show="!!autostart">
        <SettingsRow
          title="When Windows starts it"
          wide
          setting="display.autostartWindow"
          :reset="resetTo(() => autostartWindow, setAutostartWindow, 'tray')"
          :is-default="autostartWindow === 'tray'"
        >
          <Segmented :model-value="autostartWindow" :options="AUTOSTART_WINDOWS" @update:model-value="setAutostartWindow" />
        </SettingsRow>
      </Reveal>
      <SettingsRow title="Closing the window" wide setting="display.closeWindow" :reset="resetTo(() => close, setClose, 'tray')" :is-default="close === 'tray'">
        <template #description>
          <Transition name="swap" mode="out-in"><span :key="close">{{ CLOSE_LINE[close] }}</span></Transition>
        </template>
        <Segmented :model-value="close" :options="CLOSES" @update:model-value="setClose" />
      </SettingsRow>
    </SettingsCard>
  </template>
  <Transition name="drop">
    <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  </Transition>
</template>
