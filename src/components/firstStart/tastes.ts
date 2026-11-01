/**
 * The first-start guide's "Make it yours": the settings that come down to taste (about half of people would pick each),
 * each its current value and how it's saved, as Settings saves it. A computer: the look, what Melue does when it starts,
 * closing the window, starting with Windows, Discord. A phone: the look, when it starts, mobile data.
 */
import { ref, type Ref } from 'vue'
import { library } from '@/library'
import { discord } from '@/library/discord'
import { themeMode, themeTimes, type ThemeMode } from '@/lib/theme'
import { isDesktop } from '@/platform'
import { loadPlayerSettings, playerSettings, savePlayerSettings } from '@/settings/playerSettings'
import { loadSyncSettings, saveSyncSettings, syncSettings } from '@/settings/syncSettings'

export type Taste = {
  id: 'look' | 'starts' | 'close' | 'autostart' | 'discord' | 'mobile' | 'example'
  /** A question added after the first release: the version it came in (New in Melue asks it after that update). */
  since?: string
  title: string
  options: { value: string; label: string; hint: string }[]
  value: Ref<string>
  save: (value: string) => Promise<unknown>
}

const CLOSE_SETTING = 'close_window'

/** The questions, with their current answers read. */
export async function readTastes(): Promise<Taste[]> {
  await Promise.all([loadPlayerSettings().catch(() => {}), isDesktop ? null : loadSyncSettings().catch(() => {})])
  const tastes: Taste[] = [
    {
      id: 'look',
      title: 'Look',
      options: [
        { value: 'light', label: 'Light', hint: 'Always' },
        { value: 'dark', label: 'Dark', hint: 'Always' },
        { value: 'system', label: 'Like the system', hint: `Light or dark as ${isDesktop ? 'Windows' : 'your phone'} is` },
        // Its line follows the times (chosen under the cards).
        {
          value: 'adaptive',
          label: 'Adaptive',
          get hint() {
            return `Dark from ${themeTimes.dark}, light from ${themeTimes.light}`
          },
        },
      ],
      value: ref<string>(themeMode.value),
      save: async (value) => void (themeMode.value = value as ThemeMode),
    },
    {
      id: 'starts',
      title: 'When Melue starts',
      options: [
        { value: 'paused', label: 'Continue where I left off', hint: 'The last songs, ready to play' },
        { value: 'off', label: 'Start fresh', hint: 'An empty player' },
      ],
      value: ref(playerSettings.value?.resumeLastQueue === 'off' ? 'off' : 'paused'),
      save: (value) => savePlayerSettings({ resumeLastQueue: value as 'paused' | 'off' }),
    },
  ]
  if (isDesktop) {
    const [close, autostart, discordNow] = await Promise.all([
      library.setting(CLOSE_SETTING).catch(() => null),
      import('@tauri-apps/plugin-autostart').then((a) => a.isEnabled()).catch(() => true),
      discord.settings().catch(() => null),
    ])
    tastes.push(
      {
        id: 'close',
        title: 'Closing the window',
        options: [
          { value: 'tray', label: 'Keep playing', hint: 'Melue goes to the tray' },
          { value: 'quit', label: 'Quit', hint: 'The music stops' },
        ],
        value: ref(close === 'quit' ? 'quit' : 'tray'),
        save: (value) => library.setSetting(CLOSE_SETTING, value),
      },
      {
        id: 'autostart',
        title: 'Start with Windows',
        options: [
          { value: 'on', label: 'Yes', hint: 'In the tray, ready to play' },
          { value: 'off', label: 'No', hint: 'Only when you open it' },
        ],
        value: ref(autostart ? 'on' : 'off'),
        save: async (value) => {
          const a = await import('@tauri-apps/plugin-autostart')
          await (value === 'on' ? a.enable() : a.disable())
        },
      },
      {
        id: 'discord',
        title: 'Discord',
        options: [
          { value: 'on', label: 'Show what I play', hint: 'In your Discord status' },
          { value: 'off', label: 'Don’t', hint: 'Nothing is shown' },
        ],
        value: ref(discordNow?.enabled ? 'on' : 'off'),
        save: async (value) => discordNow && discord.setSettings({ ...discordNow, enabled: value === 'on' }),
      },
    )
  } else {
    const s = syncSettings.value
    tastes.push({
      id: 'mobile',
      title: 'On mobile data, sync',
      options: [
        { value: 'nothing', label: 'Nothing', hint: 'Only on Wi-Fi' },
        { value: 'small', label: 'Small things', hint: 'Likes, plays, playlists' },
        { value: 'all', label: 'Everything', hint: 'Songs too' },
      ],
      value: ref(!s?.syncOnMetered ? 'nothing' : s.copyOnMetered ? 'all' : 'small'),
      save: (value) => saveSyncSettings({ syncOnMetered: value !== 'nothing', copyOnMetered: value === 'all' }),
    })
  }
  return tastes
}
