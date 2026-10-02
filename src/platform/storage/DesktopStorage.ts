import { open } from '@tauri-apps/plugin-dialog'
import type { MusicStorage } from './MusicStorage'

/** Desktop: the system's folder dialog. */
export class DesktopStorage implements MusicStorage {
  async pickMusicFolder() {
    const picked = await open({ directory: true, title: 'Pick the folder for your music' })
    return typeof picked === 'string' ? picked : null
  }
}
