/**
 * Every setting the settings page has, by id ("sync.atStart"): its default in words (the right-click menu's "Reset
 * to default · On"), what it does ("About this setting"), and whether it's always this device's own (those don't
 * offer "This device only"). Ids are also the settings' links: melue://settings/sync.atStart.
 */
export interface SettingInfo {
  title: string
  /** The default, as the control shows it. */
  default?: string
  /** What it does and when to change it (About this setting). */
  help: string
  /** Always this device's own (its name, folders, output, the share, On this PC, storage). */
  perDevice?: boolean
}

export const SETTINGS: Record<string, SettingInfo> = {
  // ---- General
  'playback.mode': {
    title: 'Playback across devices',
    default: 'Ask',
    help: 'When another device was playing something, Melue can offer to continue it here (Ask), continue it by itself, paused (Follow), or leave it (Off). It goes through your share.',
  },
  'playback.continueWithinMinutes': {
    title: 'Continue without asking if played within',
    default: '10 min',
    help: 'With Ask: if the other device played this recently, Melue continues right away (paused) instead of asking. Never: it always asks.',
  },
  'playback.deviceName': {
    title: 'This device’s name',
    default: 'the system’s name',
    help: 'What your other devices call this one, like “Continue from TROLLGANG”. Empty goes back to the system’s own name.',
    perDevice: true,
  },

  // ---- Syncing
  'sync.atStart': { title: 'Sync when Melue starts', default: 'On', help: 'Looks for new music on your share each time you open Melue.' },
  'sync.onChange': {
    title: 'Sync on share changes',
    default: 'On',
    help: 'Syncs when the songs on the share change, so a song you add on one device reaches the others without waiting.',
  },
  'sync.onReturnAfterMinutes': {
    title: 'Sync when coming back after…',
    default: '15 min',
    help: 'Coming back to Melue after this long (the window, or the app on your phone) syncs first. Never: only the other triggers sync.',
  },
  'sync.everyMinutes': {
    title: 'Sync share interval',
    default: 'Never',
    help: 'Syncs every so often while Melue is open, on top of the other triggers. Useful when the share’s changes aren’t noticed by themselves.',
  },
  'sync.removeGone': {
    title: 'Remove songs gone from the share',
    default: 'Off',
    help: 'When a song is deleted from the share, its synced copy here goes too (to the Recycle Bin, or Android’s trash). Off: it stays here.',
  },
  'sync.askBeforeRemoving': {
    title: 'Ask first when it’s more than',
    default: '5',
    help: 'If more songs than this are gone from the share at once, Melue asks before removing them here: a whole album gone at once is usually a mistake.',
  },
  'sync.moveNewSongs': {
    title: 'Move new songs to sync automatically',
    default: 'Off',
    help: 'Songs that turn up in your music folders here are moved to the share by themselves, so every device gets them.',
  },
  'sync.shareLayout': {
    title: 'Folders on the share',
    default: 'Artist / Album',
    help: 'How Melue files songs on the share: in Artist / Album folders, or all in one folder. It’s the share’s own setting, for every device; Reorganise moves what’s there.',
  },
  'mobile.level': {
    title: 'On mobile data, sync',
    default: 'Small things',
    help: 'What syncs while your phone is on mobile data. Small things: playback, likes and playlists (tiny); songs wait for Wi-Fi. Everything: songs too, within the limits.',
  },
  'mobile.limit': { title: 'Limit for songs', default: 'No limit', help: 'With Everything: how much song data Melue may use on mobile data in a month. After that, songs wait for Wi-Fi.' },
  'mobile.monthStarts': { title: 'Month starts on', default: 'The 1st', help: 'The day your mobile plan’s month starts, so the limit counts the same month as your plan.' },
  'mobile.askOver': {
    title: 'Ask before a sync over',
    default: '100 MB',
    help: 'A sync that starts by itself asks first when its songs are bigger than this on mobile data. Sync now never asks.',
  },

  // ---- Playback
  'queue.enabled': {
    title: 'The queue follows its list',
    default: 'On',
    help: 'When the album or playlist that’s playing changes (a song added, one removed, the order changed), the queue changes with it. Customize says how.',
  },
  'queue.removedCurrent': {
    title: 'When the playing song leaves the list',
    default: 'Finish it',
    help: 'If the song playing now is taken out of the list: let it finish, or skip to the next one right away.',
  },
  'queue.removedItems': {
    title: 'Other songs that leave it',
    default: 'Remove them',
    help: 'Songs taken out of the list: removed from the queue too, or kept in it.',
  },
  'queue.newItems': {
    title: 'New songs go',
    default: 'Near their place',
    help: 'Where songs added to the list go in the queue: near their place in the list, at the end, or next.',
  },
  'queue.order': {
    title: 'Order',
    default: 'The list’s',
    help: 'When the list is sorted or moved around: the queue follows the list’s order, or keeps its own.',
  },
  'player.nextAtEnd': {
    title: 'Next on the last song',
    default: 'Nothing',
    help: 'With repeat off, Next on the last song does nothing, or goes back to the first song, paused.',
  },
  'player.repeatAddedTracks': {
    title: 'Keep added songs when it repeats',
    default: 'Off',
    help: 'Songs you added with “Add to queue” play once and leave. On: they stay and play every time the queue starts over.',
  },
  'player.volumeCurve': {
    title: 'Volume curve',
    default: 'Quadratic',
    help: 'How the volume slider turns into loudness. Quadratic and Cubic give finer control when it’s quiet; Linear makes even steps, so the quiet part goes by fast.',
  },
  'player.mediaControls': {
    title: 'Media keys and the media panel',
    default: 'On',
    help: 'Play, pause and skip from your keyboard’s media keys, headphones, and the system’s media panel.',
  },
  'player.skipSeconds': {
    title: 'Previous and next do',
    default: 'Skip songs',
    help: 'What previous and next do on media keys, headphones, the notification and the car: skip songs, or jump back and forward (handy for audiobooks and podcasts).',
  },
  'player.resumeLastQueue': {
    title: 'When Melue starts',
    default: 'Load the last queue, paused',
    help: 'What the queue does when you open Melue: the last one ready to play, the last one playing, or nothing.',
  },
  'player.previousRestartsAfterSeconds': {
    title: 'Previous restarts the song after',
    default: '3 seconds',
    help: 'Previous starts the song over once it has played this long; before that, it goes to the song before. Never: always the song before.',
  },
  'player.pausedKeepAliveMinutes': {
    title: 'Keep the player ready after pausing',
    default: '30 min',
    help: 'How long the notification and the car’s player stay after you pause, so you can pick up where you were.',
  },
  'player.keepAliveWhileCarConnected': {
    title: 'Keep it ready while Android Auto is connected',
    default: 'On',
    help: 'While your phone is connected to Android Auto, the player stays ready however long you pause.',
  },
  'output.device': {
    title: 'Output device',
    default: 'System default',
    help: 'Where this computer plays. If the chosen device goes away (headphones unplugged), Melue plays on the system default until it’s back.',
    perDevice: true,
  },

  // ---- Library
  'playlists.topSize': {
    title: 'Top songs lists hold',
    default: '50',
    help: 'How many songs the Top songs lists (this month, this year…) hold.',
  },

  // ---- Display
  'display.theme': {
    title: 'Theme',
    default: 'System',
    help: 'Light or dark. System follows your system; Adaptive switches by the clock, at the times you pick.',
  },
  'display.darkFrom': { title: 'Dark from', default: '21:00', help: 'With Adaptive: when Melue turns dark.' },
  'display.lightFrom': { title: 'Light from', default: '07:00', help: 'With Adaptive: when Melue turns light again.' },
  'display.startOn': { title: 'Start on', default: 'Home', help: 'Which page Melue opens on.' },
  'display.upNextPrimary': {
    title: 'Up next on Now playing',
    default: 'Primary',
    help: 'The queue under the player on Now playing: a panel in the primary colour (white in dark mode, black in light mode, so it stands out), or a card like the others.',
  },
  'display.homeCardsPrimary': {
    title: 'Home’s Playing and Continue cards',
    default: 'Primary',
    help: 'The cards at the top of Home (what’s playing, and continuing from another device): in the primary colour (white in dark mode, black in light mode), or like the other cards.',
  },
  'display.autostart': {
    title: 'Start Melue with Windows',
    default: 'On',
    help: 'Melue starts with Windows, in the tray (no window), so the media keys work right away.',
    perDevice: true,
  },
  'display.closeWindow': {
    title: 'Closing the window',
    default: 'Keep playing',
    help: 'Closing the window hides Melue in the tray and the music keeps playing (quit from the tray’s menu), or quits Melue.',
    perDevice: true,
  },
}
