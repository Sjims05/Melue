package com.simon.melue.android

import android.Manifest
import android.app.Activity
import android.database.ContentObserver
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.webkit.WebView
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Melue's own Android code: the permission to read the music on the phone, and Android's music index
 * (MediaStore), which knows every song on the phone and its SD cards. Called from Melue's Rust code.
 * Also the permission to read photos, which the opt-in covers next to the songs (folder.jpg) need.
 *
 * Events (to Rust listeners): "musicChanged" when the music index changes (songs added, removed, edited
 * while Melue runs), "resumed" when the app comes back to the foreground.
 */
@TauriPlugin(
    permissions = [
        // Android 13 and newer.
        Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
        Permission(strings = [Manifest.permission.READ_MEDIA_IMAGES], alias = "photos"),
        // Android 12 and older (one permission for music and photos).
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

    /** The permission this Android version uses for reading photos. */
    private fun photosAlias() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) "photos" else "storage"

    private fun photosState(): PermissionState = getPermissionState(photosAlias()) ?: PermissionState.DENIED

    /** Whether Melue may read the photos on the phone (for cover images next to the songs). */
    @Command
    fun photosPermission(invoke: Invoke) {
        invoke.resolve(JSObject().put("state", photosState().toString()))
    }

    @Command
    fun requestPhotosPermission(invoke: Invoke) {
        if (photosState() == PermissionState.GRANTED) {
            photosPermission(invoke)
            return
        }
        requestPermissionForAlias(photosAlias(), invoke, "photosPermissionResult")
    }

    @PermissionCallback
    private fun photosPermissionResult(invoke: Invoke) {
        photosPermission(invoke)
    }

    /**
     * Which entries are music: those Android marked as music, and those it hasn't looked at yet (IS_MUSIC
     * still null, e.g. right after a copy) unless they're in Android's folders for ringtones,
     * notifications, alarms and recordings.
     */
    @Suppress("DEPRECATION") // DATA: the file's path, still filled in.
    private val musicSelection =
        "${MediaStore.Audio.Media.IS_MUSIC} != 0 OR (${MediaStore.Audio.Media.IS_MUSIC} IS NULL" +
            " AND ${MediaStore.Audio.Media.DATA} NOT LIKE ? AND ${MediaStore.Audio.Media.DATA} NOT LIKE ?" +
            " AND ${MediaStore.Audio.Media.DATA} NOT LIKE ? AND ${MediaStore.Audio.Media.DATA} NOT LIKE ?)"
    private val musicSelectionArgs = arrayOf("%/Ringtones/%", "%/Notifications/%", "%/Alarms/%", "%/Recordings/%")

    /** Every song in the music index: on the phone and on SD cards (all external volumes). */
    private fun audioCollection(): Uri =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL)
        } else {
            MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
        }

    private var observer: ContentObserver? = null

    override fun load(webView: WebView) {
        super.load(webView)
        // Android notifies about every song separately while copying: Rust waits for it to calm down.
        val watcher = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                trigger("musicChanged", JSObject())
            }
        }
        activity.contentResolver.registerContentObserver(audioCollection(), true, watcher)
        observer = watcher
    }

    override fun onResume() {
        super.onResume()
        trigger("resumed", JSObject())
    }

    override fun onDestroy() {
        observer?.let { activity.contentResolver.unregisterContentObserver(it) }
        observer = null
        super.onDestroy()
    }

    /**
     * Every song in the music index: its file path (readable with the permission), name, size and modified
     * time (ms). Music only, not ringtones or recordings.
     */
    @Command
    fun listMusic(invoke: Invoke) {
        if (musicState() != PermissionState.GRANTED) {
            invoke.reject("Melue isn't allowed to read the music on this phone")
            return
        }
        val files = JSArray()
        @Suppress("DEPRECATION") // DATA: still the file's path, and readable with READ_MEDIA_AUDIO.
        val columns = arrayOf(
            MediaStore.Audio.Media.DATA,
            MediaStore.Audio.Media.DISPLAY_NAME,
            MediaStore.Audio.Media.SIZE,
            MediaStore.Audio.Media.DATE_MODIFIED,
        )
        activity.contentResolver.query(audioCollection(), columns, musicSelection, musicSelectionArgs, null)?.use { c ->
            while (c.moveToNext()) {
                val path = c.getString(0) ?: continue
                files.put(
                    JSObject()
                        .put("path", path)
                        .put("name", c.getString(1) ?: path.substringAfterLast('/'))
                        .put("size", c.getLong(2))
                        .put("modifiedAt", c.getLong(3) * 1000),
                )
            }
        }
        invoke.resolve(JSObject().put("files", files))
    }

    /**
     * The playlist files (.m3u, .m3u8) Android's index knows, like listMusic. With the music permission
     * alone Android may show none.
     */
    @Command
    fun listPlaylists(invoke: Invoke) {
        if (musicState() != PermissionState.GRANTED) {
            invoke.reject("Melue isn't allowed to read the music on this phone")
            return
        }
        val collection =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                MediaStore.Files.getContentUri(MediaStore.VOLUME_EXTERNAL)
            } else {
                MediaStore.Files.getContentUri("external")
            }
        val files = JSArray()
        @Suppress("DEPRECATION") // DATA: the file's path.
        val columns = arrayOf(
            MediaStore.Files.FileColumns.DATA,
            MediaStore.Files.FileColumns.DISPLAY_NAME,
            MediaStore.Files.FileColumns.SIZE,
            MediaStore.Files.FileColumns.DATE_MODIFIED,
        )
        @Suppress("DEPRECATION")
        val selection = "${MediaStore.Files.FileColumns.DATA} LIKE ? OR ${MediaStore.Files.FileColumns.DATA} LIKE ?"
        activity.contentResolver.query(collection, columns, selection, arrayOf("%.m3u", "%.m3u8"), null)?.use { c ->
            while (c.moveToNext()) {
                val path = c.getString(0) ?: continue
                files.put(
                    JSObject()
                        .put("path", path)
                        .put("name", c.getString(1) ?: path.substringAfterLast('/'))
                        .put("size", c.getLong(2))
                        .put("modifiedAt", c.getLong(3) * 1000),
                )
            }
        }
        invoke.resolve(JSObject().put("files", files))
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
            musicSelection,
            musicSelectionArgs,
            null,
        )?.use { it.count } ?: 0
        invoke.resolve(JSObject().put("count", count))
    }
}
