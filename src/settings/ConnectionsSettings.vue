<script setup lang="ts">
/**
 * Connections: accounts Melue can use (src-tauri github.rs). GitHub: bug reports go to Melue's issues from the
 * user's own account. Signing in shows a code (copied, and GitHub's page opens) to enter there. A sign-in (and a
 * sign-out) is every device's: it goes through the share, encrypted (src-tauri connections.rs).
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { onPageLoad } from '@/components/page/pageLoading'

const account = ref<string | null>(null)
/** Signing in: the code to enter on GitHub, and where. */
const signIn = ref<{ code: string; url: string } | null>(null)
const error = ref('')

const line = computed(() => {
  if (signIn.value) return `Enter ${signIn.value.code} on GitHub (copied), then approve`
  if (account.value) return `Signed in as ${account.value} · bug reports are sent from this account`
  return 'To send bug reports from your own account'
})

const loadAccount = async () => (account.value = await invoke<string | null>('github_account').catch(() => null))
onPageLoad(loadAccount)
// Signed in or out on another device: the same here.
const stopListening = listen('connections-changed', loadAccount)
onBeforeUnmount(() => void stopListening.then((stop) => stop()))

async function start() {
  error.value = ''
  try {
    signIn.value = await invoke<{ code: string; url: string }>('github_sign_in_start')
    await navigator.clipboard.writeText(signIn.value.code).catch(() => {})
    await openGitHub()
    account.value = await invoke<string>('github_sign_in_wait')
  } catch (e) {
    const why = String(e)
    if (why === 'expired') error.value = 'The code ran out before it was entered: sign in again'
    else if (why === 'denied') error.value = 'GitHub said no (the request was cancelled there)'
    else if (why !== 'cancelled') error.value = `Couldn’t sign in: ${why}`
  } finally {
    signIn.value = null
  }
}

const openGitHub = () => (signIn.value ? openUrl(signIn.value.url).catch(() => {}) : undefined)

function cancel() {
  invoke('github_sign_in_cancel')
}

async function signOut() {
  error.value = ''
  try {
    await invoke('github_sign_out')
    account.value = null
  } catch (e) {
    error.value = `Couldn’t sign out: ${e}`
  }
}
</script>

<template>
  <SettingsHeading title="Connections" description="Accounts Melue can use" />
  <SettingsCard>
    <SettingsRow title="GitHub" :description="line" setting="connections.github">
      <template v-if="signIn">
        <Button variant="outline" size="sm" @click="openGitHub">Open GitHub</Button>
        <Button variant="ghost" size="sm" @click="cancel">Cancel</Button>
      </template>
      <Button v-else-if="account" variant="outline" size="sm" @click="signOut">Sign out</Button>
      <Button v-else variant="outline" size="sm" @click="start">Sign in</Button>
    </SettingsRow>
  </SettingsCard>
  <Transition name="drop">
    <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  </Transition>
</template>
