// npm run clean: clears what Melue's builds leave behind, for disk space. Rust's builds (src-tauri/target, by far the
// most: debug and release builds for the computer and the phone, cached as they go), the Android app's build folders,
// the web build and Vite's cache. The next build makes them again (slower, the first time). Says how much came free.
import { execSync } from 'node:child_process'
import { existsSync, rmSync, statfsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const free = () => {
  const disk = statfsSync(root)
  return disk.bavail * disk.bsize
}
const gb = (bytes) => `${(bytes / 1024 ** 3).toFixed(1)} GB`

const before = free()
console.log('Rust builds (src-tauri/target)…')
execSync('cargo clean', { cwd: join(root, 'src-tauri'), stdio: 'inherit' })
for (const folder of ['src-tauri/gen/android/app/build', 'src-tauri/gen/android/build', 'src-tauri/gen/android/.gradle', 'dist', 'node_modules/.vite']) {
  const path = join(root, folder)
  if (!existsSync(path)) continue
  console.log(`${folder}…`)
  rmSync(path, { recursive: true, force: true })
}
console.log(`Done: ${gb(free() - before)} free again (${gb(free())} free on the disk).`)
