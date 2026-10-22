/**
 * Copying an error (Melue has no browser menu: main.ts): a right-click on anything marked `data-copy` (an error tip:
 * SettingsTip.vue) offers Copy (its words, or the part of them selected) and Copy with troubleshooting details (what
 * About's copy gives too, under the error), in a small menu in Melue's style (CopyMenu.vue, in the desktop frame).
 * On a phone a long press on it opens the menu drawer with the same.
 */
import { ClipboardList, Copy } from '@lucide/vue'
import { reactive } from 'vue'
import { troubleshootingDetails } from '@/settings/troubleshooting'
import { openMenu } from './menuDrawer'
import { toast } from './toast'

export const copyMenu = reactive({ open: false, x: 0, y: 0, text: '' })

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    toast('Copied', Copy)
  } catch {
    toast('Couldn’t copy')
  }
}

/** The menu's choices. */
export async function copyChosen(withDetails: boolean) {
  copyMenu.open = false
  await copy(withDetails ? `${copyMenu.text}\n\n${await troubleshootingDetails()}` : copyMenu.text)
}

/**
 * A right-click Melue's own menus didn't take: Copy for a marked element (its selected words, or all). True when it
 * offered it. `phone`: the drawer instead of the small menu.
 */
export function offerCopy(e: MouseEvent, phone: boolean): boolean {
  const marked = (e.target as Element | null)?.closest?.<HTMLElement>('[data-copy]')
  if (!marked) return false
  const selected = window.getSelection()
  const part = selected && marked.contains(selected.anchorNode) ? selected.toString().trim() : ''
  const text = part || marked.innerText.trim()
  if (!text) return false
  if (phone) {
    copyMenu.text = text
    openMenu({
      title: text.length > 60 ? `${text.slice(0, 60)}…` : text,
      items: [
        { action: 'copy', label: 'Copy', icon: Copy },
        { action: 'details', label: 'Copy with troubleshooting details', icon: ClipboardList },
      ],
      pick: (action) => copyChosen(action === 'details'),
    })
  } else {
    Object.assign(copyMenu, { open: true, x: e.clientX, y: e.clientY, text })
  }
  return true
}
