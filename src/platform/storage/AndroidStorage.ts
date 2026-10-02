import { showOpenDirPicker } from 'tauri-plugin-android-fs-api'
import type { MusicStorage } from './MusicStorage'

/** Android: the system folder picker, which also offers SD cards. */
export class AndroidStorage implements MusicStorage {
  async pickMusicFolder() {
    const picked = await showOpenDirPicker()
    return picked ? picked.uri : null
  }
}
