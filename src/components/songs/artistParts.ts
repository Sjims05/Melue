/**
 * A song's artist line in parts, each artist's name a part with their id, to link to their page ("A feat. B": A and B
 * each a link, " feat. " plain). The names are found in the line as the song shows it; ones that can't be are left
 * plain.
 */
export function artistParts(line: string, artists: { id: number; name: string }[] = []): { text: string; artist?: number }[] {
  const parts: { text: string; artist?: number }[] = []
  let at = 0
  for (const artist of artists) {
    const found = line.toLowerCase().indexOf(artist.name.toLowerCase(), at)
    if (found < 0 || !artist.name) continue
    if (found > at) parts.push({ text: line.slice(at, found) })
    parts.push({ text: line.slice(found, found + artist.name.length), artist: artist.id })
    at = found + artist.name.length
  }
  if (at < line.length) parts.push({ text: line.slice(at) })
  return parts
}
