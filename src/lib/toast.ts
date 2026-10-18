/**
 * A short note that something happened ("Added “Song” to the queue"): shown at the bottom of the window for a few
 * seconds (Toasts.vue, in the app's frame). One at a time; a new one replaces it. `action`: a button on it (Undo),
 * which also closes it; a note with one stays a little longer.
 */
import type { Component } from 'vue'
import { shallowRef } from 'vue'

export type ToastAction = { label: string; run: () => unknown }
export type Toast = { id: number; text: string; icon?: Component; action?: ToastAction }

const SHOWN_MS = 2800
const WITH_ACTION_MS = 5000

export const shownToast = shallowRef<Toast | null>(null)
let next = 0
let hiding: ReturnType<typeof setTimeout> | undefined

export function toast(text: string, icon?: Component, action?: ToastAction) {
  clearTimeout(hiding)
  shownToast.value = { id: ++next, text, icon, action }
  hiding = setTimeout(() => (shownToast.value = null), action ? WITH_ACTION_MS : SHOWN_MS)
}

/** The note's button: closes it, then does its action. */
export function runToastAction() {
  const action = shownToast.value?.action
  clearTimeout(hiding)
  shownToast.value = null
  action?.run()
}
