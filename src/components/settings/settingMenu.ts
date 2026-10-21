/**
 * What a setting row's right-click menu can offer besides Reset to default, given by the page that shows the rows
 * (the settings page provides it; rows elsewhere, like the Components page's, only reset). A row names its setting
 * with `setting="sync.atStart"`.
 */
import type { InjectionKey } from 'vue'

export interface SettingMenu {
  /** The setting's default in words, what it does, and whether it's always this device's own. */
  info(id: string): { title: string; default?: string; help: string; perDevice?: boolean } | undefined
  isDeviceOnly(id: string): boolean
  setDeviceOnly(id: string, on: boolean): void
  copyLink(id: string): void
  /** The setting whose link was just copied (the row says so for a moment). */
  copied: string | null
}

export const SETTING_MENU: InjectionKey<SettingMenu> = Symbol('setting menu')

/** The row's title, for what's in it (a DropValue's drawer on a phone says which setting it's choosing for). */
export const ROW_TITLE: InjectionKey<() => string | undefined> = Symbol('setting row title')
