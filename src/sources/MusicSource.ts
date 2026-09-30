/**
 * The contract every music source implements: SMB today, potentially a
 * local-folder source or an Android SAF source later. The sync engine and
 * UI only ever talk to this interface, never to a concrete source class.
 */

export interface RemoteEntry {
  name: string
  isDirectory: boolean
  size: number
}

/**
 * Connection details. Kept protocol-agnostic at this level so adding a
 * second protocol later (FTP, WebDAV, etc.) is a new implementation of
 * MusicSource, not a rework of this type.
 */
export interface SourceConfig {
  host: string
  user: string
  pass: string
  share: string
}

export interface MusicSource {
  connect(config: SourceConfig): Promise<void>

  /** List files/folders at `path` (relative to the share root). */
  listFiles(path: string): Promise<RemoteEntry[]>

  /**
   * Copy one remote file to a local path.
   * Implementations are expected to write safely (temp file + rename)
   * where the underlying transport supports it.
   */
  copyFile(remotePath: string, localPath: string): Promise<void>
}
