/**
 * Melue Dev only (main.ts, with the dev page): the app's own parts on `window.melue`, so a test driving the app from
 * outside (Chrome's remote debugging) can reach the same player, router, library and settings the app uses.
 */
import { router } from '@/router'
import { library } from '@/library'
import * as sync from '@/library/sync'
import { player } from '@/platform'
import * as nowPlaying from '@/player/nowPlaying'
import * as playerSettings from '@/settings/playerSettings'
import { listPlay } from '@/lib/listPlay'
import * as songs from '@/player/songs'
import * as theme from '@/lib/theme'
import { appearance } from '@/lib/appearance'
import * as sharedSettings from '@/settings/sharedSettings'
import * as remotePlayback from '@/player/remotePlayback'
import * as testPass from './testPass'

export function installTestHooks() {
  Object.assign(window, { melue: { router, library, sync, player, nowPlaying, playerSettings, listPlay, songs, theme, appearance, sharedSettings, remotePlayback, testPass } })
}
