/**
 * A question before something that's hard to undo, from anywhere (a song's menu: Move to sync?): `ask` shows it
 * (ConfirmHost.vue, in the app's frame) and resolves true when the action is chosen, false when cancelled.
 */
import { shallowRef } from 'vue'

export type Question = { title: string; text: string; action: string; danger?: boolean }

export const asked = shallowRef<(Question & { answer: (yes: boolean) => void }) | null>(null)

export function ask(question: Question): Promise<boolean> {
  asked.value?.answer(false)
  return new Promise((answer) => (asked.value = { ...question, answer }))
}
