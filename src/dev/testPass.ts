/**
 * The test pass on the dev page (DevPage.vue): what to try before a release, each marked untested, passed, failed or
 * someone else's to try ("You": what needs a person, a car or ears), with a note. Kept on this device (a library
 * setting), so a pass can be followed over days.
 */
import { reactive } from 'vue'
import { library } from '@/library'

export type TestState = 'untested' | 'passed' | 'failed' | 'you'
export type TestItem = { id: string; what: string; how?: string; you?: boolean }

const SETTING = 'dev_test_pass'

export const TEST_PASS: { group: string; items: TestItem[] }[] = [
  {
    group: 'Across devices',
    items: [
      { id: 'devices.mode', what: 'Playback across devices: Ask, Follow, Off', how: 'One device plays; the other offers it, follows it paused, or leaves it' },
      { id: 'devices.within', what: 'Continue without asking if played within', how: 'Played recently: continued at once, paused' },
      { id: 'devices.name', what: 'This device’s name', how: 'The other device says it' },
    ],
  },
  {
    group: 'Syncing',
    items: [
      { id: 'sync.shareChanges', what: 'Sync when the share changes', how: 'A file added on the share: synced soon after' },
      { id: 'sync.interval', what: 'Sync every …', how: 'A short interval: a sync on time' },
      { id: 'sync.removeGone', what: 'Remove songs gone from the share, and Ask first', how: 'On the test share only' },
      { id: 'sync.moveNew', what: 'Move new songs to the share', how: 'On the test share only' },
      { id: 'sync.layout', what: 'The share’s layout', how: 'Reorganise on the test share' },
      { id: 'sync.mobileData', what: 'On mobile data: Small things, Everything', how: 'Phone with Wi-Fi off' },
      { id: 'sync.reports', what: 'Bug reports sent later, through the share', how: 'Made on the phone, sent by the computer' },
    ],
  },
  {
    group: 'Playback',
    items: [
      { id: 'play.queueFollows', what: 'The queue follows its list (the group)', how: 'Changes to the list reach the queue' },
      { id: 'play.fromStart', what: 'Play from the start' },
      { id: 'play.fromAllSongs', what: 'A song from All songs' },
      { id: 'play.lastSong', what: 'Next on the last song' },
      { id: 'play.keepAdded', what: 'Keep added songs' },
      { id: 'play.prevNext', what: 'What previous and next do' },
      { id: 'play.mediaKeysPc', what: 'Media keys on the computer' },
      { id: 'play.mediaKeysPhone', what: 'Media buttons on the phone (headset, lock screen)' },
      { id: 'play.atStart', what: 'When Melue starts', how: 'Restart: as set' },
      { id: 'play.keepReady', what: 'Keep the player ready' },
      { id: 'play.volumeCurve', what: 'Volume curve', how: 'Listen: the steps feel even', you: true },
      { id: 'play.output', what: 'Output device', how: 'Listen: it plays where chosen', you: true },
      { id: 'play.car', what: 'Android Auto', how: 'In the car', you: true },
    ],
  },
  {
    group: 'Display',
    items: [
      { id: 'display.topSongs', what: 'Top songs size' },
      { id: 'display.adaptive', what: 'Adaptive theme: dark and light times', how: 'Times near now: it switches' },
      { id: 'display.upNextCard', what: 'Up next on Now playing: Primary, Like the cards' },
      { id: 'display.homeCards', what: 'Home’s Playing and Continue cards' },
      { id: 'display.closing', what: 'Closing the window: to the tray, or quit' },
      { id: 'display.windowsStart', what: 'Start Melue with Windows, and how', how: 'The installed app, after signing in to Windows', you: true },
    ],
  },
]

export const testPass = reactive<Record<string, { state: TestState; note: string }>>({})

export async function loadTestPass() {
  const saved = await library.setting(SETTING).catch(() => null)
  const parsed = saved ? (JSON.parse(saved) as Record<string, { state: TestState; note: string }>) : {}
  for (const group of TEST_PASS)
    for (const item of group.items) testPass[item.id] = parsed[item.id] ?? { state: item.you ? 'you' : 'untested', note: '' }
}

export async function setTest(id: string, state: TestState, note?: string) {
  // Not loaded yet (the dev page not opened): loaded first, or this one result would be saved as the whole pass.
  if (!Object.keys(testPass).length) await loadTestPass()
  testPass[id] = { state, note: note ?? testPass[id]?.note ?? '' }
  await library.setSetting(SETTING, JSON.stringify(testPass)).catch(() => {})
}

/** Starts the pass again: every item untested (or yours), notes gone. */
export async function resetTestPass() {
  await library.setSetting(SETTING, '').catch(() => {})
  await loadTestPass()
}
