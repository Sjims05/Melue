/**
 * Lists to play as cards (Home's Recently played): an album, a playlist, an artist, a folder or a Made for you list,
 * from its source ('album:3'), with its picture, words and page. Lists that are gone are left out.
 */
import { AUTO_COVERS, type CoverColors } from '@/components/covers'
import { coverUrl, library, type AutoPlaylist, type MusicFolder, type Source } from '@/library'
import { count } from '@/lib/format'
import { autoCard, autoPage, playlistCover, playlistLine } from '../playlists/playlists'
import type { Component } from 'vue'

export interface ListCard {
  source: Source
  title: string
  subtitle: string
  image?: string
  collage?: string[]
  colors?: CoverColors
  icon?: Component
  page: string
}

export async function listCards(sources: string[]): Promise<ListCard[]> {
  let autos: AutoPlaylist[] | null = null
  let folders: MusicFolder[] | null = null
  const card = async (source: string): Promise<ListCard | null> => {
    const [kind, raw] = source.split(':')
    const id = Number(raw)
    const at = source as Source
    if (kind === 'album') {
      const album = (await library.albumPage(id))?.album
      return album
        ? { source: at, title: album.title, subtitle: ['Album', album.artist ?? 'Various artists'].join(' · '), image: album.cover ? coverUrl(album.cover) : undefined, page: `/album/${id}` }
        : null
    }
    if (kind === 'playlist') {
      const p = await library.playlist(id)
      return { source: at, title: p.name, subtitle: playlistLine(p), ...playlistCover(p), page: `/playlist/${id}` }
    }
    if (kind === 'artist') {
      const artist = (await library.artistPage(id))?.artist
      return artist ? { source: at, title: artist.name, subtitle: `Artist · ${count(artist.tracks + artist.featured, 'song')}`, page: `/artist/${id}` } : null
    }
    if (kind === 'folder') {
      folders ??= await library.folders()
      const folder = folders.find((f) => f.id === id)
      return folder ? { source: at, title: folder.name, subtitle: `Folder · ${count(folder.tracks, 'song')}`, ...AUTO_COVERS.folder, page: `/folder/${id}` } : null
    }
    autos ??= await library.autoPlaylists()
    const auto = autos.find((a) => a.source === source)
    return auto ? { source: at, ...autoCard(auto), page: autoPage(auto) } : null
  }
  const cards = await Promise.all(sources.map((s) => card(s).catch(() => null)))
  return cards.filter((c): c is ListCard => !!c)
}
