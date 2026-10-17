/**
 * The like button's heart: it fills in and out softly (the `heart` class, components/songs/songs.css), and pops (grows
 * a little and settles) as a song is liked.
 */
export function popHeart(button: EventTarget | null, liking: boolean) {
  const heart = (button as HTMLElement | null)?.querySelector('svg')
  if (!liking || !heart || matchMedia('(prefers-reduced-motion: reduce)').matches) return
  heart.animate([{ scale: 1 }, { scale: 1.3, offset: 0.4 }, { scale: 1 }], { duration: 340, easing: 'cubic-bezier(.2,.8,.2,1)' })
}
