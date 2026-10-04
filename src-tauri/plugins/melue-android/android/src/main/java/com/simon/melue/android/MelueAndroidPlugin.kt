package com.simon.melue.android

import android.Manifest
import android.app.Activity
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Melue's own Android code: the permission to read the music on the phone, and Android's music index
 * (MediaStore), which knows every song on the phone and its SD cards. Called from Melue's Rust code.
 */
@TauriPlugin(
    permissions = [
        // Android 13 and newer.
        Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
        // Android 12 and older.
        Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
    ]
)
class MelueAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    /** The permission this Android version uses for reading music. */
    private fun musicAlias() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) "audio" else "storage"

    private fun musicState(): PermissionState = getPermissionState(musicAlias()) ?: PermissionState.DENIED

    /** Whether Melue may read the music on the phone: granted, denied, prompt or prompt-with-rationale. */
    @Command
    fun musicPermission(invoke: Invoke) {
        invoke.resolve(JSObject().put("state", musicState().toString()))
    }

    /** Asks for that permission (Android shows its dialog unless it was granted or permanently denied). */
    @Command
    fun requestMusicPermission(invoke: Invoke) {
        if (musicState() == PermissionState.GRANTED) {
            musicPermission(invoke)
            return
        }
        requestPermissionForAlias(musicAlias(), invoke, "musicPermissionResult")
    }

    @PermissionCallback
    private fun musicPermissionResult(invoke: Invoke) {
        musicPermission(invoke)
    }

    /** Every song in the music index: on the phone and on SD cards (all external volumes). */
    private fun audioCollection(): Uri =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL)
        } else {
            MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
        }

    /** How many songs the music index has (music only, not ringtones or recordings). */
    @Command
    fun countMusic(invoke: Invoke) {
        if (musicState() != PermissionState.GRANTED) {
            invoke.reject("Melue isn't allowed to read the music on this phone")
            return
        }
        val count = activity.contentResolver.query(
            audioCollection(),
            arrayOf(MediaStore.Audio.Media._ID),
            "${MediaStore.Audio.Media.IS_MUSIC} != 0",
            null,
            null,
        )?.use { it.count } ?: 0
        invoke.resolve(JSObject().put("count", count))
    }
}
