import * as audio from 'tauri-plugin-native-audio-api'
import type { TrackedListChange, TrackedListConfig, TrackedListEntry, TrackedLists } from './TrackedLists'

/** Android / iOS: the native-audio plugin keeps the lists, also while the app isn't running. */
export class NativeTrackedLists implements TrackedLists {
  configure = (lists: TrackedListConfig[]) => audio.setTrackedLists(lists)
  get = (listId: string) => audio.getTrackedList(listId)
  set = (listId: string, entries: TrackedListEntry[], options?: { merge?: boolean }) => audio.setTrackedList(listId, entries, options)
  onChange = (listener: (change: TrackedListChange) => void | Promise<void>) =>
    audio.onTrackedListChanged(({ listId, entries, changedAtMs }) => listener({ listId, entries, changedAtMs }))
}
