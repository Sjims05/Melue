/** A choice card (the first-start guide, New in Melue's questions). */
export const CHOICE = 'rounded-xl border-2 p-4 text-left transition-colors'

/** A choice's look. Light: a card, picked with a border in the primary colour. Dark: picked, the songs' highlight
 * (songs.css); not, a card. */
export const picked = (on: boolean) =>
  on
    ? 'border-primary bg-card dark:border-transparent dark:bg-song-highlight dark:text-song-highlight-foreground'
    : 'border-transparent bg-card hover:bg-well'
