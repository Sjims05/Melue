/**
 * Where the synced music lives on this device.
 * Android: any folder through Android's picker, including an SD card. Desktop: a normal folder.
 */
export interface MusicStorage {
  /**
   * Lets the user pick the folder for the music. Resolves to the picked folder
   * (a content URI on Android, a path on desktop), or null when cancelled.
   */
  pickMusicFolder(): Promise<string | null>
}
