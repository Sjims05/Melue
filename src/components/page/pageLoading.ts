/**
 * A page that waits for its parts' first data before showing (Sync & settings: a skeleton, then the page all at once,
 * not section after section popping in). A part loads with `onPageLoad(fn)` (like onMounted); the page that provides
 * `providePageLoading()` knows when every part's first load is done (or after `LONGEST`, whatever is still loading).
 * Without such a page, onPageLoad is just onMounted.
 */
import { inject, onMounted, provide, ref, type InjectionKey } from 'vue'

const LONGEST = 1500
const LOADING: InjectionKey<(load: Promise<unknown>) => void> = Symbol('page loading')

/** Like onMounted, and the page waits for it (its first data). */
export function onPageLoad(load: () => unknown) {
  const track = inject(LOADING, null)
  onMounted(() => {
    const result = load()
    if (track && result instanceof Promise) track(result)
  })
}

/** The page: `ready` once its parts' first loads are done. */
export function providePageLoading() {
  const ready = ref(false)
  let pending = 0
  const settle = () => setTimeout(() => !pending && (ready.value = true))
  provide(LOADING, (load) => {
    pending++
    load.finally(() => {
      pending--
      settle()
    })
  })
  // Nothing to wait for (or all done at once), and never longer than LONGEST.
  onMounted(() => {
    settle()
    setTimeout(() => (ready.value = true), LONGEST)
  })
  return ready
}
