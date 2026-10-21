/**
 * Home as it was last shown, kept on this device: at start Home draws it at once, before the library has answered
 * (then the fresh one replaces it). Covers' icons are kept by name (a component can't be stored).
 */
import { User } from '@lucide/vue'
import { AUTO_COVERS } from '@/components/covers'
import type { Track } from '@/library'
import type { ListCard } from './listCards'

const KEY = 'melue-home'
type Icon = keyof typeof AUTO_COVERS
type KeptCard = Omit<ListCard, 'icon' | 'placeholder'> & { icon?: Icon; person?: boolean }

export function rememberHome(lists: ListCard[], recent: Track[]) {
  const name = (icon: ListCard['icon']) => (Object.keys(AUTO_COVERS) as Icon[]).find((k) => AUTO_COVERS[k].icon === icon)
  const kept: KeptCard[] = lists.map(({ icon, placeholder, ...card }) => ({ ...card, icon: name(icon), person: placeholder === User }))
  try {
    localStorage.setItem(KEY, JSON.stringify({ lists: kept, recent }))
  } catch {
    // Not kept: Home just waits for the library next time.
  }
}

export function rememberedHome(): { lists: ListCard[]; recent: Track[] } | null {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? 'null') as { lists: KeptCard[]; recent: Track[] } | null
    if (!kept) return null
    const lists = kept.lists.map(({ icon, person, ...card }) => ({
      ...card,
      icon: icon && AUTO_COVERS[icon] ? AUTO_COVERS[icon].icon : undefined,
      placeholder: person ? User : undefined,
    }))
    return { lists, recent: kept.recent }
  } catch {
    return null
  }
}
