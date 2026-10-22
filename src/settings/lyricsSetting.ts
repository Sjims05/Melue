/**
 * "Find lyrics online" (Playback settings; src-tauri library/online_lyrics.rs): on by default, kept in Melue's settings
 * ("lyrics_online": "0" is off), shared across devices like the others (sharedSettings.ts).
 */
import { reactive } from 'vue'
import { resetTo } from '@/components/settings'
import { library } from '@/library'

const KEY = 'lyrics_online'

export const lyricsOnline = reactive({
  on: true,
  async load() {
    lyricsOnline.on = (await library.setting(KEY).catch(() => null)) !== '0'
  },
})

export async function setLyricsOnline(on: boolean) {
  lyricsOnline.on = on
  await library.setSetting(KEY, on ? '1' : '0')
}

export const resetLyricsOnline = () => ({
  reset: resetTo(() => lyricsOnline.on, setLyricsOnline, true),
  isDefault: lyricsOnline.on,
})

/** For settings sync: read and set (sharedSettings.ts). */
export const readLyricsOnline = async () => (await lyricsOnline.load(), lyricsOnline.on)
