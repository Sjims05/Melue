/**
 * A bug report: what the person wrote, and parts that help find the problem (Melue and the device, the last steps,
 * the screen in words, errors, the sync and the library, the log's last lines). Everything personal is replaced by
 * placeholders (src-tauri report.rs), the same throughout; the review (ReportDialog.vue) shows the report as it will
 * be sent, what the placeholders stand for (only there), and lets each part be left out. Sent as a public issue on
 * Melue's GitHub, or copied; one that can't go yet is kept and sent later, by this device or another one signed in
 * (src-tauri report/saved.rs). With "Keep a screenshot with reports" on, the window is captured as the report opens
 * (kept on the device and the share, never sent).
 */
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { reactive } from 'vue'
import { library } from '@/library'
import { isAndroid, isMobile } from '@/platform'
import { troubleshootingDetails } from '@/settings/troubleshooting'
import { outline } from './outline'
import { pauseTrail, trail, type Noted } from './trail'

export type PartId = 'device' | 'steps' | 'screen' | 'errors' | 'state' | 'log'
export type Part = { id: PartId; title: string; text: string; include: boolean }
export type Replaced = { placeholder: string; original: string }

export type Report = {
  /** What the person wrote (always sent). */
  note: string
  parts: Part[]
  /** What each placeholder stands for: shown in the review, never sent. */
  replaced: Replaced[]
}

/** Recognised by the issue workflow (labels it). */
const MARK = '<!-- melue-report -->'
/** Between the parts while they're scrubbed together (so a name gets the same placeholder in all). */
const SPLIT = '\n@@melue-part@@\n'

const ago = (at: number, now: number) => {
  const s = Math.round((now - at) / 1000)
  return s < 60 ? `${s} s ago` : s < 3600 ? `${Math.round(s / 60)} min ago` : `${Math.round(s / 3600)} h ago`
}
const listed = (items: Noted[], now: number) => items.map((n) => `- ${n.what} (${ago(n.at, now)})`).join('\n')

/** What's there to report, taken now (before the report's own dialog covers the screen). */
export function snapshot() {
  return { at: Date.now(), screen: outline(), ...trail() }
}

/** The report for `note`, from a `snapshot()`. */
export async function makeReport(note: string, taken: ReturnType<typeof snapshot>): Promise<Report> {
  const [version, isDev, state, log] = await Promise.all([
    getVersion().catch(() => '?'),
    invoke<boolean>('app_is_dev').catch(() => false),
    troubleshootingDetails().catch(() => ''),
    invoke<string>('report_log').catch(() => ''),
  ])
  const device = [
    `Melue ${version}${isDev ? ' (Melue Dev: a development build)' : ''}`,
    `${isAndroid ? 'Android phone' : isMobile ? 'Phone' : 'Computer'} · ${navigator.userAgent}`,
    `Language: ${navigator.language}`,
  ].join('\n')
  // troubleshootingDetails' first lines are the version and the device, said above already.
  const stateLines = state.split('\n').filter((l) => !/^Melue |^Device: /.test(l)).join('\n')
  const parts: Part[] = [
    { id: 'device', title: 'Melue and the device', text: device, include: true },
    { id: 'steps', title: 'What was done just before', text: listed(taken.steps, taken.at) || 'Nothing yet', include: true },
    { id: 'screen', title: 'The screen, in words', text: taken.screen, include: true },
    { id: 'errors', title: 'Errors', text: listed(taken.errors, taken.at) || 'None', include: true },
    { id: 'state', title: 'The sync and the library', text: stateLines, include: true },
    { id: 'log', title: 'Melue’s log (its last lines)', text: log || 'No log here', include: true },
  ]
  const scrubbed = await invoke<{ text: string; replaced: Replaced[] }>('report_scrub', {
    text: [note, ...parts.map((p) => p.text)].join(SPLIT),
    extra: [],
  })
  const [cleanNote, ...texts] = scrubbed.text.split(SPLIT)
  parts.forEach((p, i) => (p.text = texts[i] ?? ''))
  return { note: cleanNote, parts, replaced: scrubbed.replaced }
}

const TITLE_MOST = 70

/** The issue's title: the note's first line, shortened. */
export function reportTitle(report: Report): string {
  const first = report.note.trim().split('\n')[0].trim()
  return first.length > TITLE_MOST ? `${first.slice(0, TITLE_MOST - 3)}…` : first || 'A bug report from Melue'
}

/** The note under the title: without its first line when that's the whole title (cut short, it stays). */
export function noteRest(report: Report): string {
  const [first, ...rest] = report.note.trim().split('\n')
  return first.trim().length > TITLE_MOST ? report.note.trim() : rest.join('\n').trim()
}

/** The issue's text (Markdown), with only the parts included. */
export function reportBody(report: Report, id: string): string {
  const fence = (text: string) => '```text\n' + text.replace(/```/g, '`‵`') + '\n```'
  const rest = noteRest(report)
  const sections = rest ? [`### What happened\n\n${rest}`] : []
  for (const part of report.parts) {
    if (!part.include) continue
    // The long ones fold away.
    if (part.id === 'log' || part.id === 'steps') sections.push(`<details><summary>${part.title}</summary>\n\n${fence(part.text)}\n\n</details>`)
    else sections.push(`### ${part.title}\n\n${fence(part.text)}`)
  }
  const left = report.parts.filter((p) => !p.include).map((p) => p.title.toLowerCase())
  if (left.length) sections.push(`_Left out by the reporter: ${left.join(', ')}._`)
  sections.push(`_Sent from Melue. Names, paths and addresses are replaced by placeholders like [song 1]._\n${MARK}\n<!-- report ${id} -->`)
  return sections.join('\n\n')
}

/** "Keep a screenshot with reports" (this device's own; off unless switched on). */
const SCREENSHOT_SETTING = 'report_screenshot'

/** The report dialog (ReportDialog.vue): open, the report's id, what was there when it opened, and its screenshot. */
export const reporting = reactive({
  open: false,
  id: '',
  taken: null as ReturnType<typeof snapshot> | null,
  /** A screenshot was taken for it. */
  screenshot: false,
  /** "Keep a screenshot with reports". */
  keepScreenshots: false,
})

export async function readScreenshotSetting() {
  reporting.keepScreenshots = (await library.setting(SCREENSHOT_SETTING).catch(() => null)) === '1'
}
export async function setKeepScreenshots(on: boolean) {
  reporting.keepScreenshots = on
  await library.setSetting(SCREENSHOT_SETTING, on ? '1' : '0').catch(() => {})
}

let opening = false
/** Opens Report a bug: the screen, the steps (and the screenshot) are taken first, before the dialog covers it. */
export async function openReport() {
  if (reporting.open || opening) return
  opening = true
  try {
    reporting.id = crypto.randomUUID()
    reporting.taken = snapshot()
    pauseTrail(true)
    await readScreenshotSetting()
    reporting.screenshot = reporting.keepScreenshots && (await invoke<boolean>('report_screenshot', { id: reporting.id }).catch(() => false))
    reporting.open = true
  } finally {
    opening = false
  }
}

/** A report kept (sent: `sentUrl`; else it's sent when it can, from here or another device signed in). */
export type SavedReport = { id: string; madeMs: number; title: string; body: string; sentUrl?: string | null; sentMs?: number | null; screenshot?: boolean }

export const saveReport = (report: SavedReport) => invoke('report_save', { report })
/** Given up (closed before it was kept): its screenshot goes. */
export const discardReport = (id: string) => invoke('report_discard', { id }).catch(() => {})
export const listReports = () => invoke<SavedReport[]>('reports_list').catch(() => [] as SavedReport[])
