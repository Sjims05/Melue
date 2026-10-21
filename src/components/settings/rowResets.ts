/**
 * The setting rows that can be reset to default, while they're on screen: a heading's "Reset these to default"
 * (SettingsHeading.vue) resets the changed ones under it, up to the next heading.
 */
export interface Resettable {
  el: () => HTMLElement | null | undefined
  /** The setting's id: rows of one setting (the output devices) count once. */
  setting: () => string | undefined
  reset: () => unknown
  isDefault: () => boolean
}

const rows = new Set<Resettable>()

/** A row's reset, until the row goes (the returned function). */
export function keepReset(row: Resettable): () => void {
  rows.add(row)
  return () => rows.delete(row)
}

/** The changed settings from `heading` to the next heading (one row each). */
export function changedUnder(heading: HTMLElement): Resettable[] {
  const headings = [...document.querySelectorAll<HTMLElement>('[data-settings-heading]')]
  const next = headings.find((h) => heading.compareDocumentPosition(h) & Node.DOCUMENT_POSITION_FOLLOWING)
  const seen = new Set<string>()
  return [...rows].filter((row) => {
    const el = row.el()
    if (!el?.isConnected || row.isDefault()) return false
    const after = heading.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING
    const before = !next || next.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_PRECEDING
    const id = row.setting()
    if (!after || !before || (id && seen.has(id))) return false
    if (id) seen.add(id)
    return true
  })
}
