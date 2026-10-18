/** Words for numbers in the app's lines ("12 songs", "48 min"). */

/** "1 song", "12 songs" (thousands grouped). */
export const count = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`

/** A total length: "48 min", "1 h 12 min" (under a minute: "1 min"). */
export function totalLength(ms: number): string {
  const minutes = Math.max(1, Math.round(ms / 60_000))
  const hours = Math.floor(minutes / 60)
  return hours ? `${hours} h ${minutes % 60} min` : `${minutes} min`
}
