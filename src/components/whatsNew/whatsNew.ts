/**
 * New in Melue: what's new since the last version this device ran, shown once after an update instead of the "Updated"
 * note (an update with nothing here: just the note), and from About and the Dev page. A big feature shows itself, live
 * (its real component with demo data, on a stage: no pictures to keep up to date or ship); new settings that come down
 * to taste (tastes.ts, with a `since`) are asked on a page after them, as the first-start guide asks them; everything
 * else just gets its default.
 */
import { Sparkles } from '@lucide/vue'
import { defineAsyncComponent, reactive, ref, shallowRef, type Component } from 'vue'
import { readTastes, type Taste } from '@/components/firstStart/tastes'
import { toast } from '@/lib/toast'
import { newer } from '@/lib/version'
import { isDesktop } from '@/platform'
import { router } from '@/router'

export type Feature = {
  id: string
  /** The version it came in: the release's own number (shown when an update brings features of several versions). */
  since: string
  title: string
  /** What it is, in a line. */
  text: string
  /** Where to find it. */
  where: string
  /** It, live on the stage. */
  demo?: Component
  /** The demo's size on the stage (1: as in the app): all of it scaled alike, the layout the same, to fit the stage. */
  scale?: number
  /** Only on a computer, or only on a phone. */
  only?: 'desktop' | 'mobile'
}

/** The features, oldest first (shown newest first). */
export const FEATURES: Feature[] = [
  {
    id: 'miniPlayer',
    since: '1.1.0',
    title: 'Mini player',
    text: 'Your music from the tray: what plays, the controls and the volume, without opening Melue.',
    where: 'Right-click Melue’s icon in the tray',
    demo: defineAsyncComponent(() => import('./MiniPlayerDemo.vue')),
    scale: 0.8,
    only: 'desktop',
  },
]

/** `version`: the one it's about (All changes in it: the changelog). */
export const whatsNew = reactive({ open: false, features: [] as Feature[], version: '' })
/** Its questions (shallow: each its own ref, as the cards want it). */
export const newTastes = shallowRef<Taste[]>([])

const here = (f: Feature) => !f.only || f.only === (isDesktop ? 'desktop' : 'mobile')
/** Any feature to show on this kind of device. */
export const anyFeatures = FEATURES.some(here)

/** A made-up feature from a version an update skipped over (the Dev page): how several versions' features look. */
const skipped = (since: string): Feature => ({
  id: 'example',
  since,
  title: 'An example feature',
  text: 'From a version this update skipped over: shown too, once, marked with the version it came in.',
  where: 'Where it is in Melue',
})

/** A made-up question (the Dev page): how the questions' page looks; nothing saved. */
const example = (): Taste => ({
  id: 'example',
  since: '0.0.1',
  title: 'An example question',
  options: [
    { value: 'one', label: 'One way', hint: 'What it does, in a line' },
    { value: 'other', label: 'The other', hint: 'And what this does' },
  ],
  value: ref('one'),
  save: async () => {},
})

/**
 * Shows the features (for this kind of device) newer than version `since` (none: all of them, and no questions), newest
 * first, and the questions newer than it, about `version`. `withExample` (the Dev page): every feature, those for the
 * other kind of device too, a made-up one from a skipped version (`skippedVersion`), and a made-up question.
 */
export async function showWhatsNew(version: string, since?: string, withExample = false, skippedVersion = '') {
  const tastes = since ? (await readTastes().catch(() => [])).filter((t) => t.since && newer(t.since, since)) : []
  if (withExample) tastes.push(example())
  const features = FEATURES.filter((f) => (here(f) || withExample) && (!since || newer(f.since, since)))
  if (withExample) features.unshift(skipped(skippedVersion || '0.0.1'))
  whatsNew.features = features.reverse()
  newTastes.value = tastes
  whatsNew.version = version
  whatsNew.open = whatsNew.features.length + tastes.length > 0
}

/** The changelog (About's What's new). */
function showChangelog() {
  router.push({ path: '/settings', query: { whatsNew: '1' } })
}

/**
 * Updated from version `from` to `version`: New in Melue with what's new since, or (nothing there) the note.
 * `withExample`: a made-up feature from a skipped version and a made-up question too (Melue Dev's pretend update).
 */
export async function afterUpdate(version: string, from: string, withExample = false, skippedVersion = '') {
  await showWhatsNew(version, from, withExample, skippedVersion)
  if (!whatsNew.open) toast(`Updated to Melue ${version}`, Sparkles, { label: "What's new", run: showChangelog })
}
