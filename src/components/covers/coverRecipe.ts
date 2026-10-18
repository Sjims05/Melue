/**
 * Melue's covers when there's no picture (the playground's Cover maker, phoenix-ux-playground/cover-maker/cover.js):
 * a 135° gradient between two colours, under a soft white light at 30% / 25% (0.32 strong, gone by 45% of the way to
 * the far corner). Auto playlists add a white icon in the middle; playlists are a 2×2 collage of their songs' covers.
 */
import { Folder, Heart, Library, TrendingUp } from '@lucide/vue'
import type { Component } from 'vue'

export type CoverColors = readonly [string, string]

/** The recipe as CSS (a `background`). */
export const coverCss = ([from, to]: CoverColors) =>
  `radial-gradient(circle at 30% 25%, rgba(255,255,255,.32), transparent 45%), linear-gradient(135deg, ${from}, ${to})`

/**
 * The auto playlists' covers (Made for you): their colours and icon. Like the other covers, lively: the bright colour
 * at the bottom (the gradient's end), the two not too far apart. The Top songs lists are one
 * scale from cool to warm: all time blue, then violet, magenta, and last month coral (the newest the warmest).
 */
export const AUTO_COVERS = {
  songs: { colors: ['#5b6170', '#a7adb9'], icon: Library },
  liked: { colors: ['#e8487a', '#ffb3c7'], icon: Heart },
  topAllTime: { colors: ['#4f6ef0', '#a8c4ff'], icon: TrendingUp },
  topYear: { colors: ['#8a5cf0', '#cdb6ff'], icon: TrendingUp },
  topHalfYear: { colors: ['#cf4fd6', '#f6b0ee'], icon: TrendingUp },
  topMonth: { colors: ['#f0644d', '#ffb86b'], icon: TrendingUp },
  folder: { colors: ['#2b86c5', '#7fd8e8'], icon: Folder },
} satisfies Record<string, { colors: CoverColors; icon: Component }>
