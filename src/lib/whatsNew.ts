/**
 * What's new: CHANGELOG.md (each version's section, newest first; the release notes too), read into versions for
 * About's What's new (the note after an update: lib/updatedNote.ts).
 */
import changelog from '../../CHANGELOG.md?raw'

export interface Version {
  version: string
  /** "2026-10-22" */
  date: string
  /** The version in a few words ("Lyrics, and …"). */
  summary: string
  /** Its points (the "- " items, their continued lines joined). */
  points: string[]
}

/** The versions in CHANGELOG.md, newest first. */
export const versions: Version[] = changelog
  .split(/^## /m)
  .slice(1)
  .map((section) => {
    const [heading, ...rest] = section.split('\n')
    const [, version = '', date = ''] = /^([\d.]+)\s*\(([^)]*)\)/.exec(heading) ?? []
    const body = rest.join('\n').trim()
    const [summary = '', ...more] = body.split(/\n\s*\n/)
    const points: string[] = []
    for (const line of more.join('\n').split('\n')) {
      if (line.startsWith('- ')) points.push(line.slice(2).trim())
      else if (line.trim() && points.length) points[points.length - 1] += ` ${line.trim()}`
    }
    return { version, date, summary: summary.replace(/\s*\n\s*/g, ' ').trim(), points }
  })
  .filter((v) => v.version)
