/**
 * Whether a component is in the phone's frame (MobileShell provides it): for what styles can't say alone (the song
 * lists' row height, a tap on a song playing it). Styles use the `phone:` variant (style.css).
 */
import { inject, type InjectionKey } from 'vue'

export const PHONE: InjectionKey<boolean> = Symbol('phone')
export const usePhone = () => inject(PHONE, false)
