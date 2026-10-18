/**
 * A name's letter for A–Z headings: its first letter, "The" and accents ignored (as the library sorts), "#" for
 * anything that isn't a letter.
 */
export function letterOf(name: string): string {
  const first = name.trim().replace(/^the\s+/i, '').normalize('NFD').charAt(0).toUpperCase()
  return /[A-Z]/.test(first) ? first : '#'
}
