/**
 * Sample songs for the Components page's song lists (the design's), with covers drawn by the playground's Cover
 * maker recipe (phoenix-ux-playground/cover-maker/cover.js): a 135° gradient between two colours, and a soft white
 * light at 30% / 25% (0.32 strong, gone by 45% of the way to the far corner). The colours look lively: the bright one
 * at the bottom (the gradient's end), and the two not too far apart.
 */
import type { Song } from '@/components/songs'

// The light's radius: 45% of the distance from (30%, 25%) to the farthest corner (100%, 100%).
const LIGHT_R = 0.45 * Math.hypot(0.7, 0.75)

const cover = (from: string, to: string) =>
  `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><defs>` +
      `<linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient>` +
      `<radialGradient id="l" cx="0.3" cy="0.25" r="${LIGHT_R.toFixed(4)}"><stop offset="0" stop-color="#fff" stop-opacity="0.32"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>` +
      `</defs><rect width="96" height="96" fill="url(#g)"/><rect width="96" height="96" fill="url(#l)"/></svg>`,
  )}`

export const SAMPLE_SONGS: Song[] = [
  { id: 1, title: 'Golden hour', artist: 'Amara Sol', album: 'Soft focus', duration: 252, cover: cover('#f0644d', '#ffb86b') },
  { id: 2, title: 'Slow motion', artist: 'Elias Bloom', album: 'After the rain', duration: 226, cover: cover('#5b6cf0', '#a8c0ff'), liked: true },
  { id: 3, title: 'Stay a little longer', artist: 'June & the Satellites', album: 'In your orbit', duration: 212, cover: cover('#9b5de5', '#f3a6e8') },
  { id: 4, title: 'In your orbit', artist: 'Theo Banks', album: 'In your orbit', duration: 212, cover: cover('#9b5de5', '#f3a6e8') },
  { id: 5, title: 'Sunday kind of love', artist: 'Nia Rivers', album: 'Sunday kind of love', duration: 265, cover: cover('#ff5f6d', '#ffc371') },
  { id: 6, title: 'Coastline', artist: 'Oliver Hayes', album: 'Open water', duration: 198, cover: cover('#2b86c5', '#7fd8e8'), liked: true },
  { id: 7, title: 'All the way home', artist: 'Mira Okafor', album: 'The long way', duration: 221, cover: cover('#3fae7a', '#c6f08a') },
  { id: 8, title: 'Midnight City Lights', artist: 'Neon Dreams', album: 'Someway', duration: 225, cover: cover('#5b6170', '#a7adb9') },
]
