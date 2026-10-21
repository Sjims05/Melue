/**
 * What a setting row's right-click menu can offer besides Reset to default, given by the page that shows the rows
 * (the settings page provides it; rows elsewhere, like the Components page's, only reset). A row names its setting
 * with `setting="sync.atStart"`.
 */
import type { InjectionKey } from 'vue'

export interface SettingMenu {
  /**
   * Resets these rows' settings to default (a row's Reset to default, a heading's for the rows under it): the change
   * goes to the other devices like any other, and a note offers Undo. A reset that returns a function: its undo
   * (see [`resetTo`]); otherwise the setting's value from before is set back.
   */
  reset(rows: { setting?: string; reset: () => unknown }[]): Promise<void>
  /** The setting's default in words, what it does, and whether it's always this device's own. */
  info(id: string): { title: string; default?: string; help: string; perDevice?: boolean } | undefined
  isDeviceOnly(id: string): boolean
  setDeviceOnly(id: string, on: boolean): void
  copyLink(id: string): void
  /** The setting whose link was just copied (the row says so for a moment). */
  copied: string | null
}

export const SETTING_MENU: InjectionKey<SettingMenu> = Symbol('setting menu')

/** A reset that can be undone: sets `value` with `set`, and returns setting back what `get` gave before. */
export function resetTo<T>(get: () => T, set: (value: T) => unknown, value: T) {
  return () => {
    const before = get()
    set(value)
    return () => set(before)
  }
}

/** The row's title, for what's in it (a DropValue's drawer on a phone says which setting it's choosing for). */
export const ROW_TITLE: InjectionKey<() => string | undefined> = Symbol('setting row title')
