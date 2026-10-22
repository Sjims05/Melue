/**
 * Two colours from a cover, for the lyrics (Now playing): the line being sung and the word being sung. Picked in OKLCH
 * (lightness, chroma, hue: how bright, how vivid, which colour), where those are separate:
 *
 * - the cover's pixels are grouped by hue, each counted by how vivid it is (greys hardly count);
 * - the line gets the strongest hue; the word a second clearly different one, or else a stronger version of the first;
 * - both are then made lighter or darker until they read well on the card (4.5:1 contrast at least, as for text),
 *   and kept within the colours a screen can show;
 * - the word must also stand clearly apart from its line (a big enough colour difference): if not, it's pushed further
 *   from the card's lightness and made more vivid, then turned to a neighbouring hue, until it does.
 *
 * A cover that's grey, black and white, or missing gives null: the theme's own colours are used.
 */

export interface LyricColors {
  line: string
  word: string
}

type Lch = { l: number; c: number; h: number }

// ---- colour maths (OKLab: Björn Ottosson's)

const toLinear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)

function rgbToLch(r: number, g: number, b: number): Lch {
  const [R, G, B] = [r / 255, g / 255, b / 255].map(toLinear)
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B)
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B)
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  return { l: L, c: Math.hypot(a, bb), h: ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360 }
}

/** Linear sRGB (0 to 1 when the screen can show it). */
function lchToLinear({ l, c, h }: Lch): [number, number, number] {
  const a = c * Math.cos((h * Math.PI) / 180)
  const b = c * Math.sin((h * Math.PI) / 180)
  const L = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const M = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const S = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  return [
    4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
    -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
    -0.0041960863 * L - 0.7034186147 * M + 1.707631046 * S,
  ]
}

const inGamut = (lch: Lch) => lchToLinear(lch).every((v) => v >= -0.0001 && v <= 1.0001)

/** Within the screen's colours: less vivid until it fits (the hue and lightness kept). */
function fit(lch: Lch): Lch {
  if (inGamut(lch)) return lch
  let [lo, hi] = [0, lch.c]
  for (let i = 0; i < 20; i++) {
    const mid = (lo + hi) / 2
    if (inGamut({ ...lch, c: mid })) lo = mid
    else hi = mid
  }
  return { ...lch, c: lo }
}

const luminance = (lch: Lch) => {
  const [r, g, b] = lchToLinear(lch).map((v) => Math.min(1, Math.max(0, v)))
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}
const contrast = (a: Lch, b: Lch) => {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p)
  return (x + 0.05) / (y + 0.05)
}

/** Lighter (on a dark card) or darker (on a light one) until it reads well: `ratio` contrast at least. */
function readable(lch: Lch, bg: Lch, ratio = 4.5): Lch {
  const step = bg.l < 0.5 ? 0.02 : -0.02
  let c = fit(lch)
  for (let i = 0; i < 60 && contrast(c, bg) < ratio; i++) c = fit({ ...c, l: Math.min(0.98, Math.max(0.05, c.l + step)) })
  return c
}

/** How different two colours look (the distance in OKLab; about 0.02 is barely noticeable). */
function difference(a: Lch, b: Lch): number {
  const ab = (x: Lch) => [x.l, x.c * Math.cos((x.h * Math.PI) / 180), x.c * Math.sin((x.h * Math.PI) / 180)]
  const [p, q] = [ab(a), ab(b)]
  return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2])
}
/** The word and its line: at least this different, to tell them apart at a glance. */
const APART = 0.13

/** The word colour, moved until it stands apart from the line (and still reads well on the card). */
function apartFrom(word: Lch, line: Lch, bg: Lch): Lch {
  const away = bg.l < 0.5 ? 1 : -1
  let w = word
  for (let i = 0; i < 12 && difference(w, line) < APART; i++) w = readable({ ...w, l: w.l + 0.025 * away, c: w.c * 1.08 }, bg)
  // Lightness and vividness weren't enough (both near their limits): a neighbouring hue, further each try.
  for (let turn = 30; turn <= 120 && difference(w, line) < APART; turn += 15) w = readable({ ...w, h: (line.h + turn) % 360 }, bg)
  return w
}

const css = ({ l, c, h }: Lch) => `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)})`
const hueGap = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b))

// ---- picking

/** The hues in a cover's pixels (RGBA), strongest first: the average colour of each 15° band, and its weight. */
function hues(pixels: Uint8ClampedArray): { lch: Lch; weight: number }[] {
  const bands = Array.from({ length: 24 }, () => ({ l: 0, c: 0, x: 0, y: 0, weight: 0 }))
  for (let i = 0; i < pixels.length; i += 4) {
    if (pixels[i + 3] < 128) continue
    const p = rgbToLch(pixels[i], pixels[i + 1], pixels[i + 2])
    // Greys, near-black and near-white say little about a cover's colour.
    if (p.c < 0.02 || p.l < 0.15 || p.l > 0.97) continue
    // The most vivid pixels count most (cubed): a muted cover's few livelier pixels set its colour, not its grey.
    const w = p.c * p.c * p.c
    const band = bands[Math.floor(p.h / 15) % 24]
    band.l += p.l * w
    band.c += p.c * w
    band.x += Math.cos((p.h * Math.PI) / 180) * w
    band.y += Math.sin((p.h * Math.PI) / 180) * w
    band.weight += w
  }
  return bands
    .filter((b) => b.weight > 0)
    .map((b) => ({
      lch: { l: b.l / b.weight, c: b.c / b.weight, h: ((Math.atan2(b.y, b.x) * 180) / Math.PI + 360) % 360 },
      weight: b.weight,
    }))
    .sort((a, b) => b.weight - a.weight)
}

/** The two colours for pixels of a cover, on a card of colour `bg` (an OKLCH colour). Null: too grey. */
export function lyricColorsFrom(pixels: Uint8ClampedArray, bg: Lch): LyricColors | null {
  const found = hues(pixels)
  // Hardly any colour (a black and white cover: under 2% of it with a tint): the theme's.
  let tinted = 0
  for (let i = 0; i < pixels.length; i += 4) if (rgbToLch(pixels[i], pixels[i + 1], pixels[i + 2]).c >= 0.035) tinted++
  if (!found.length || tinted / (pixels.length / 4) < 0.02) return null
  const first = found[0]
  const second = found.find((h) => hueGap(h.lch.h, first.lch.h) >= 30 && h.weight >= first.weight * 0.12)
  // Vivid enough to be seen as a colour, even when the cover's is soft: a muted cover gives its hue, livelier.
  const vivid = (lch: Lch) => ({ ...lch, c: Math.max(lch.c * 1.25, 0.16) })
  const line = readable(vivid(first.lch), bg)
  const word = apartFrom(
    second
      ? readable(vivid(second.lch), bg)
      : // One colour on the cover: the word stronger than its line (more vivid, and further from the card).
        readable({ ...line, c: line.c * 1.4, l: line.l + (bg.l < 0.5 ? 0.06 : -0.06) }, bg),
    line,
    bg,
  )
  return { line: css(line), word: css(word) }
}

/** An OKLCH colour as CSS writes it ("oklch(0.205 0 0)"), for the card's background. */
export function parseOklch(text: string): Lch | null {
  const m = /oklch\(\s*([\d.]+)(%?)\s+([\d.]+)\s+([\d.]+)/.exec(text)
  if (!m) return null
  return { l: Number(m[1]) / (m[2] ? 100 : 1), c: Number(m[3]), h: Number(m[4]) }
}

// A cover's pixels, once read (the colours for each card colour are quick to work out from them).
const pixelsOf = new Map<string, Promise<Uint8ClampedArray | null>>()

/**
 * A cover's pixels (48 × 48), or null when they can't be read. Loaded from its own address (`?for=colours`): the page
 * shows the same picture as a plain image, and the browser would hand that copy over, which a page may not read.
 * A failure isn't kept: the next try reads it again.
 */
function readPixels(src: string): Promise<Uint8ClampedArray | null> {
  let read = pixelsOf.get(src)
  if (!read) {
    read = new Promise<Uint8ClampedArray | null>((done) => {
      const img = new Image()
      img.crossOrigin = 'anonymous'
      img.onload = () => {
        try {
          const size = 48
          const canvas = document.createElement('canvas')
          canvas.width = canvas.height = size
          const ctx = canvas.getContext('2d', { willReadFrequently: true })!
          ctx.drawImage(img, 0, 0, size, size)
          done(ctx.getImageData(0, 0, size, size).data)
        } catch {
          done(null)
        }
      }
      img.onerror = () => done(null)
      img.src = `${src}${src.includes('?') ? '&' : '?'}for=colours`
    })
    pixelsOf.set(src, read)
    read.then((pixels) => pixels || pixelsOf.delete(src))
  }
  return read
}

/** The lyrics' colours for a cover image (its address), on a card of colour `bg`. */
export async function lyricColors(src: string, bg: Lch): Promise<LyricColors | null> {
  const pixels = await readPixels(src)
  return pixels ? lyricColorsFrom(pixels, bg) : null
}
