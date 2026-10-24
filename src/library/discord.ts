/**
 * Discord (src-tauri discord.rs; computers): "Listening to Melue" in your Discord status, with the song, its artist,
 * a progress bar and its cover; hovering it, this week's listening when that's on too. Shared across devices like
 * the other settings; phones keep it for your computers.
 */
import { invoke } from '@tauri-apps/api/core'

export interface DiscordSettings {
  /** Show what plays in your Discord status (default off). */
  enabled: boolean
  /** Hovering the cover: this week's listening ("This week: 312 songs · most played ABBA"). */
  weekly: boolean
}

export const discord = {
  settings: () => invoke<DiscordSettings>('discord_settings'),
  setSettings: (settings: DiscordSettings) => invoke<DiscordSettings>('discord_set_settings', { settings }),
  /** "off" (the setting), "showing", or "noDiscord" (Discord isn't running on this computer). */
  status: () => invoke<'off' | 'showing' | 'noDiscord'>('discord_status'),
}
