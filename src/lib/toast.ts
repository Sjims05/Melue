/**
 * A short note that something happened ("Added “Song” to the queue"): shown at the bottom of the window for a few
 * seconds (Toasts.vue, in the app's frame). One at a time; a new one replaces it. `action`: a button on it (Undo),
 * which also closes it; a note with one stays a little longer. `open`: tapping the note goes where it's about (the
 * playlist a song went into, Up next, Settings), which also closes it. `stay`: shown until closed (`closeToast`) or
 * replaced, for something that's waiting on you (a GitHub sign-in's code).
 */
import type { Component } from 'vue'
import { shallowRef } from 'vue'

export type ToastAction = { label: string; run: () => unknown }
export type Toast = { id: number; text: string; icon?: Component; action?: ToastAction; open?: () => unknown }

const SHOWN_MS = 2800
const WITH_ACTION_MS = 5000

export const shownToast = shallowRef<Toast | null>(null)
let next = 0
let hiding: ReturnType<typeof setTimeout> | undefined

export function toast(text: string, icon?: Component, action?: ToastAction, open?: () => unknown, stay = false): number {
  clearTimeout(hiding)
  shownToast.value = { id: ++next, text, icon, action, open }
  if (!stay) hiding = setTimeout(() => (shownToast.value = null), action ? WITH_ACTION_MS : SHOWN_MS)
  return next
}

/** Closes the note `id` (one shown with `stay`), if it's still the one shown. */
export function closeToast(id: number) {
  if (shownToast.value?.id !== id) return
  clearTimeout(hiding)
  shownToast.value = null
}

/** The note tapped: closes it, then goes where it's about. */
export function openToast() {
  const open = shownToast.value?.open
  clearTimeout(hiding)
  shownToast.value = null
  open?.()
}

/** The note's button: closes it, then does its action. */
export function runToastAction() {
  const action = shownToast.value?.action
  clearTimeout(hiding)
  shownToast.value = null
  action?.run()
}
