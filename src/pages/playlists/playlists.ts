/**
 * Playlists as the app shows them (their cards and pages): the cover, the grey line, the corner badge, where they
 * live (for their menu), and the auto playlists' covers, words and pages.
 */
import { Cloud, Link, Lock, Unlink } from '@lucide/vue'
import type { Component } from 'vue'
import { AUTO_COVERS, type CoverKind } from '@/components/covers'
import { coverUrl, type AutoPlaylist, type Playlist } from '@/library'
import { count, totalLength } from '@/lib/format'

/** Its picture: one cover, or a 2×2 collage of up to four (repeated to fill it); none: the note. */
export function playlistCover(p: Playlist): { image?: string; collage?: string[] } {
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

/** The corner badge: Synced, This device, Linked or Unfollowed (none without a share, for an app playlist). */
export function playlistBadge(p: Playlist): { icon: Component; label: string } | null {
  if (p.kind === 'linked') return p.following ? { icon: Link, label: 'Linked' } : { icon: Unlink, label: 'Unfollowed' }
  if (p.synced && !p.localOnly) return { icon: Cloud, label: 'Synced' }
  if (p.localOnly) return { icon: Lock, label: 'This device' }
  return null
}

/** Where it lives, for its menu: linked, synced or kept here (with a share), or nothing to choose (no share). */
export function playlistKind(p: Playlist, shareSetUp: boolean): CoverKind {
  if (p.kind === 'linked') return { kind: 'playlist', lives: p.following ? 'linked' : 'unfollowed' }
  if (!shareSetUp) return { kind: 'playlist', lives: 'plain' }
  return { kind: 'playlist', lives: p.localOnly ? 'here' : 'synced' }
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
