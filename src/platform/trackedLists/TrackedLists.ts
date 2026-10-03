/**
 * Lists kept up to date as things play, e.g. "the last 5 playlists played": most recent first,
 * no duplicates, capped at a limit. On Android / iOS the native-audio plugin keeps them, also while
 * the app isn't running (Android Auto); the desktop version comes with the desktop player.
 *
 * Every entry has the time it was last played, so copies of a list from different devices can be
 * merged (newest wins), for example through the SMB share.
 */

export interface TrackedListConfig {
  /** Unique; also usable as an Android Auto library folder's `trackedList`. */
  id: string
  /**
   * `folder`: the playlist / album a queue was started from (the player's `sourceId`, or picked in
   * Android Auto). `item`: the tracks themselves.
   */
  track: 'folder' | 'item'
  /** How many to keep, most recent first. Default 10. */
  limit?: number
  /** How long something has to play before it counts. Default 0: as soon as it starts. Finishing always counts. */
  countAfterSeconds?: number
}

export interface TrackedListEntry {
  /** A folder id (string) or a track id (number). */
  id: string | number
  /** When it was last played; used to merge copies of a list (newest wins). */
  playedAtMs: number
}

export interface TrackedListChange {
  listId: string
  /** The whole list after the change, newest first. */
  entries: TrackedListEntry[]
  changedAtMs: number
}

export interface TrackedLists {
  /** Declares the lists to keep. Lists no longer declared are dropped. Saved on the device. */
  configure(lists: TrackedListConfig[]): Promise<void>
  /** A list's current entries, newest first. */
  get(listId: string): Promise<TrackedListEntry[]>
  /**
   * Sets a list, e.g. from the database or another device's copy. Replaces it, or with `merge`
   * combines it with the current one (by id, newest `playedAtMs` wins). Resolves with the result.
   */
  set(listId: string, entries: TrackedListEntry[], options?: { merge?: boolean }): Promise<TrackedListEntry[]>
  /**
   * Calls `listener` for every change: first those made while the app wasn't running, then live
   * ones. Resolves to a function that stops listening.
   */
  onChange(listener: (change: TrackedListChange) => void | Promise<void>): Promise<() => void>
}
