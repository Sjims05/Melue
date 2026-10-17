/**
 * The player's own icons (Melue's design: the concepts' narrower, sharper shapes, not Lucide's wide ones): play, pause,
 * previous, next. Filled, in a 24 × 24 box like Lucide's, so they size and colour the same way (class, currentColor).
 * Repeat one is Lucide's repeat with a "1" beside it (the player draws that).
 */
import { h, type FunctionalComponent, type SVGAttributes } from 'vue'

const icon = (name: string, shapes: () => ReturnType<typeof h>[]): FunctionalComponent<SVGAttributes> => {
  const component: FunctionalComponent<SVGAttributes> = (_, { attrs }) =>
    h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'currentColor', 'aria-hidden': 'true', class: 'size-4', ...attrs }, shapes())
  component.displayName = name
  return component
}

/** A narrow, sharp triangle (pointing right). */
export const PlayIcon = icon('PlayIcon', () => [h('path', { d: 'M7 4.5 19 12 7 19.5Z' })])
/** Two bars, their ends slightly rounded. */
export const PauseIcon = icon('PauseIcon', () => [
  h('rect', { x: 6.5, y: 4.5, width: 4, height: 15, rx: 1 }),
  h('rect', { x: 13.5, y: 4.5, width: 4, height: 15, rx: 1 }),
])
/** A bar, then a triangle pointing left. */
export const PreviousIcon = icon('PreviousIcon', () => [
  h('rect', { x: 5, y: 5, width: 2.5, height: 14, rx: 0.5 }),
  h('path', { d: 'M19 5 9 12 19 19Z' }),
])
/** A triangle pointing right, then a bar. */
export const NextIcon = icon('NextIcon', () => [
  h('path', { d: 'M5 5 15 12 5 19Z' }),
  h('rect', { x: 16.5, y: 5, width: 2.5, height: 14, rx: 0.5 }),
])
