import { PickerInitialLocation, showOpenDirPicker } from 'tauri-plugin-android-fs-api'
import type { MusicStorage } from './MusicStorage'

/** Android: the system folder picker, which also offers SD cards. It opens in Music (not wherever it was last). */
export class AndroidStorage implements MusicStorage {
  async pickMusicFolder() {
    const picked = await showOpenDirPicker({ initialLocation: PickerInitialLocation.PublicDir('Music') })
    return picked ? picked.uri : null
  }
}
