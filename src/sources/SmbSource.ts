import { invoke } from '@tauri-apps/api/core'
import type { MusicSource, RemoteEntry, SourceConfig } from './MusicSource'

/**
 * Matches the SmbEntry struct in src-tauri/src/lib.rs exactly.
 * Tauri auto-converts Rust snake_case field names to camelCase in the
 * JSON sent to JS, so is_directory -> isDirectory here.
 */
interface SmbEntryDto {
  name: string
  isDirectory: boolean
  size: number
}

export class SmbSource implements MusicSource {
  private share: string | null = null

  async connect(config: SourceConfig): Promise<void> {
    await invoke('smb_connect', {
      host: config.host,
      user: config.user,
      pass: config.pass,
    })
    this.share = config.share
  }

  async listFiles(path: string): Promise<RemoteEntry[]> {
    if (!this.share) {
      throw new Error('SmbSource: call connect() before listFiles()')
    }
    const entries = await invoke<SmbEntryDto[]>('smb_list_dir', {
      share: this.share,
      path,
    })
    return entries.map((e) => ({
      name: e.name,
      isDirectory: e.isDirectory,
      size: e.size,
    }))
  }

  async copyFile(remotePath: string, localPath: string): Promise<void> {
    if (!this.share) {
      throw new Error('SmbSource: call connect() before copyFile()')
    }
    await invoke('smb_copy_file', {
      share: this.share,
      remotePath,
      localPath,
    })
  }
}
