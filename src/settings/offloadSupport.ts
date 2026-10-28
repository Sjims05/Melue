/**
 * What this phone can play on its own audio chip (offload: saves battery with the screen off), by format, as the phone
 * says (the player plugin's getOffloadSupport): the Audio settings show "Save battery on … songs too" (OffloadSetting.vue)
 * only when some format can be offloaded but not gaplessly (a limit of the phone). The Dev page previews it for pretend
 * phones.
 */
import { computed, reactive } from 'vue'
import { player } from '@/platform'
import type { OffloadFormat } from '@/platform/player/Player'

export const offloadSupport = reactive({
  /** As the phone says (empty on a computer, or not read yet). */
  formats: [] as OffloadFormat[],
})

let loaded = false
/** Reads what the phone says, once. */
export async function loadOffloadSupport() {
  if (loaded) return
  loaded = true
  offloadSupport.formats = (await player.getOffloadSupport().catch(() => ({ formats: [] }))).formats
}

/** The formats a phone offloads gaplessly and only with gaps, as words ("MP3 and AAC"; gapless: '' for none). */
export type OffloadChoice = { gapless: string; withGaps: string }

const names = (formats: OffloadFormat[]) => {
  const list = formats.map((f) => f.format)
  return list.length > 1 ? `${list.slice(0, -1).join(', ')} and ${list[list.length - 1]}` : (list[0] ?? '')
}

/** What there is to choose on a phone with these formats; null: nothing (no format offloaded only with gaps). */
export function offloadChoiceFor(formats: OffloadFormat[]): OffloadChoice | null {
  const withGaps = formats.filter((f) => f.offload && !f.gapless)
  if (!withGaps.length) return null
  return { gapless: names(formats.filter((f) => f.offload && f.gapless)), withGaps: names(withGaps) }
}

/** This phone's choice (the Audio settings). */
export const offloadChoice = computed(() => offloadChoiceFor(offloadSupport.formats))
