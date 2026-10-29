/**
 * Playlists as the app shows them (their cards and pages): the cover, the grey line, the corner badge, where they
 * live (for their menu), and the auto playlists' covers, words and pages.
 */
import { AudioLines, CircleAlert, Cloud, Link, Lock, Unlink } from '@lucide/vue'
import type { Component } from 'vue'
import { AUTO_COVERS, type CoverKind } from '@/components/covers'
import { coverUrl, library, type AutoPlaylist, type Playlist } from '@/library'
import { count, totalLength } from '@/lib/format'

/**
 * Its picture: its own cover, or a Spotify one's picture from Spotify (unless its songs' are chosen), or else its
 * songs': one, or a 2×2 collage of up to four (repeated to fill it); none: the note. `size`: 'large' for a page's big
 * cover.
 */
export function playlistCover(p: Playlist, size: 'small' | 'large' = 'small'): { image?: string; collage?: string[] } {
  if (p.customCover) return { image: coverUrl(p.customCover, size) }
  if (p.spotify?.cover && !p.spotify.songsCover) return { image: coverUrl(p.spotify.cover, size) }
  const covers = p.covers.map((c) => coverUrl(c))
  if (covers.length === 0) return {}
  if (covers.length === 1) return { image: covers[0] }
  return { collage: [0, 1, 2, 3].map((i) => covers[i % covers.length]) }
}

/** "12 songs · 1 unavailable · 3 not on this device yet · 48 min"; an empty one: "No songs yet". */
export function playlistLine(p: Playlist): string {
  if (p.entries === 0) return 'No songs yet'
  const missing = p.entries - p.available - p.notHere
  return [
    count(p.entries, 'song'),
    ...(missing > 0 ? [`${missing} unavailable`] : []),
    ...(p.notHere > 0 ? [`${p.notHere} not on this device yet`] : []),
    totalLength(p.durationMs),
  ].join(' · ')
}

type Badge = { icon: Component; label: string }

/**
 * Where it lives, with a share: Synced or This device (a Spotify one too). A linked one by where its file is: in the
 * synced folder (every device has it), or a folder only here.
 */
export function playlistWhere(p: Playlist, shareSetUp: boolean): Badge | null {
  if (p.kind === 'linked' && !p.spotify) return shareSetUp ? (p.fileSynced ? { icon: Cloud, label: 'Synced' } : { icon: Lock, label: 'This device' }) : null
  if (p.synced && !p.localOnly) return { icon: Cloud, label: 'Synced' }
  if (p.localOnly) return { icon: Lock, label: 'This device' }
  return null
}

/** A linked one's link: Linked, or Unfollowed; a Spotify one's: Spotify (changed there: the "!"). */
export function playlistLink(p: Playlist): Badge | null {
  if (p.kind !== 'linked') return null
  if (p.spotify) return p.spotify.changed ? { icon: CircleAlert, label: 'Changed on Spotify' } : { icon: AudioLines, label: 'Spotify' }
  return p.following ? { icon: Link, label: 'Linked' } : { icon: Unlink, label: 'Unfollowed' }
}

/** The corner badge: where it lives, and a linked one's link next to it ("Synced · Linked"). */
export function playlistBadge(p: Playlist, shareSetUp: boolean): (Badge & { also?: Component }) | null {
  const where = playlistWhere(p, shareSetUp)
  const link = playlistLink(p)
  if (where && link) return { icon: where.icon, also: link.icon, label: `${where.label} · ${link.label}` }
  return where ?? link
}

/** Where it lives, for its menu: linked, synced or kept here (with a share), or nothing to choose (no share). */
export function playlistKind(p: Playlist, shareSetUp: boolean): CoverKind {
  const customCover = !!p.customCover
  if (p.spotify) {
    // Without its own cover: Spotify's picture or its songs' (the other one offered).
    const spotifyCover = customCover || !p.spotify.cover ? undefined : p.spotify.songsCover ? 'songs' : 'spotify'
    return { kind: 'playlist', lives: 'spotify', customCover, synced: shareSetUp ? !p.localOnly : undefined, spotifyCover }
  }
  if (p.kind === 'linked') return { kind: 'playlist', lives: p.following ? 'linked' : 'unfollowed', customCover }
  if (!shareSetUp) return { kind: 'playlist', lives: 'plain', customCover }
  return { kind: 'playlist', lives: p.localOnly ? 'here' : 'synced', customCover }
}

/** An auto playlist's card: its cover, title and line ("Top songs" / "Last month · 50 songs"). */
export function autoCard(a: AutoPlaylist) {
  const [kind, id] = a.source.split(':')
  const style =
    kind === 'top'
      ? { month: AUTO_COVERS.topMonth, 'half-year': AUTO_COVERS.topHalfYear, year: AUTO_COVERS.topYear, 'all-time': AUTO_COVERS.topAllTime }[id] ??
        AUTO_COVERS.topAllTime
      : kind === 'liked'
        ? AUTO_COVERS.liked
        : kind === 'folder'
          ? AUTO_COVERS.folder
          : AUTO_COVERS.songs
  const [title, period] = a.name.split(': ')
  const what = kind === 'folder' ? 'Folder' : period ? period.charAt(0).toUpperCase() + period.slice(1) : ''
  return {
    ...style,
    title,
    subtitle: [what, count(a.songs, 'song')].filter(Boolean).join(' · '),
  }
}

/** The page an auto playlist opens: its own (a folder's is the folder). */
export function autoPage(a: AutoPlaylist): string {
  const [kind, id] = a.source.split(':')
  if (kind === 'songs') return '/playlist/songs'
  if (kind === 'folder') return `/folder/${id}`
  if (kind === 'top') return `/playlist/top-${id}`
  return '/playlist/liked'
}

// ---- Spotify playlists: read again

/** A Spotify playlist opened again within this long of its last read here isn't read again. */
const READ_AGAIN_MS = 5 * 60_000
const lastRead = new Map<number, number>()
/**
 * Reads Spotify playlist `id` again (its page opened: unless it was read here in the last few minutes; `now`: Update
 * from Spotify). Whether it changed here (its pages reload by themselves, from the library's change).
 */
export async function readSpotify(id: number, now = false): Promise<boolean> {
  if (!now && Date.now() - (lastRead.get(id) ?? 0) < READ_AGAIN_MS) return false
  lastRead.set(id, Date.now())
  return library.updateSpotifyPlaylists(id)
}
