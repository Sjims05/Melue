/** Music folders in the Library: where one is, in short words, and its icon. */
import { Folder, Smartphone } from '@lucide/vue'
import type { MusicFolder } from '@/library'

/**
 * Where a folder is, short: on Windows without "C:\Users\<you>\" ("Music\Melue"); a folder chosen on Android
 * by its name in the storage ("Music"); all music on the phone as that.
 */
export function folderPlace(folder: MusicFolder): string {
  if (folder.kind === 'android_media') return 'All music on this phone'
  if (folder.kind === 'android_tree') {
    const tree = decodeURIComponent(folder.location.split('/tree/')[1] ?? folder.location)
    return tree.replace(/^[^:]*:/, '') || folder.name
  }
  return folder.location.replace(/^[A-Za-z]:\\Users\\[^\\]+\\/, '').replace(/^\/(home|Users)\/[^/]+\//, '')
}

export const folderIcon = (folder: MusicFolder) => (folder.kind === 'android_media' ? Smartphone : Folder)
