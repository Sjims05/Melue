package com.simon.melue.android

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.database.ContentObserver
import android.net.ConnectivityManager
import android.net.Network
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.content.pm.PackageManager
import android.provider.DocumentsContract
import android.provider.Settings
import android.provider.MediaStore
import android.webkit.WebView
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
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
 * while Melue runs), "paused" when the app goes to the background, "resumed" when it comes back.
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
/** The Android Keystore key Melue's secrets are encrypted with. */
class MelueAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    /** The permission this Android version uses for reading music. */
    private fun musicAlias() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) "audio" else "storage"

    private fun musicState(): PermissionState = state(musicAlias(), musicPermissionName())

    private fun musicPermissionName() =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE

    /**
     * A permission's state: Android's own answer first (through the app, always current), then Tauri's
     * (which tells "not asked yet" from "denied"; it answers "denied" when its own bookkeeping isn't there).
     */
    private fun state(alias: String, permission: String): PermissionState {
        val granted = activity.applicationContext.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED
        val tauri = getPermissionState(alias)
        if (granted != (tauri == PermissionState.GRANTED)) {
            Log.w("MelueAndroid", "$permission: Android says ${if (granted) "granted" else "not granted"}, Tauri says $tauri")
        }
        return if (granted) PermissionState.GRANTED else tauri ?: PermissionState.PROMPT
    }

    /** Whether Melue may read the music on the phone: granted, denied, prompt or prompt-with-rationale. */
    @Command
    fun musicPermission(invoke: Invoke) {
        invoke.resolve(JSObject().put("state", musicState().toString()))
    }

    /**
     * Asks for that permission. Android shows its dialog unless it was granted, or denied for good ("don't ask
     * again"): then there's nothing to wait for, and the answer is "denied" (the user has to switch it on in
     * the system settings).
     */
    @Command
    fun requestMusicPermission(invoke: Invoke) {
        val state = musicState()
        if (state == PermissionState.GRANTED || state == PermissionState.DENIED) {
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

    private fun photosState(): PermissionState = state(
        photosAlias(),
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) Manifest.permission.READ_MEDIA_IMAGES else Manifest.permission.READ_EXTERNAL_STORAGE,
    )

    /** Whether Melue may read the photos on the phone (for cover images next to the songs). */
    @Command
    fun photosPermission(invoke: Invoke) {
        invoke.resolve(JSObject().put("state", photosState().toString()))
    }

    /** Like requestMusicPermission: denied for good answers "denied" straight away. */
    @Command
    fun requestPhotosPermission(invoke: Invoke) {
        val state = photosState()
        if (state == PermissionState.GRANTED || state == PermissionState.DENIED) {
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
        // Melue's app is running: it saves and offers synced playback itself (not the car's provider).
        CarQueue.appRunning = true
        // The folder for Melue's log file (Rust's logging.rs writes there); Android makes it on first ask.
        activity.getExternalFilesDir(null)
        // Android notifies about every song separately while copying: Rust waits for it to calm down.
        val watcher = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                trigger("musicChanged", JSObject())
            }
        }
        activity.contentResolver.registerContentObserver(audioCollection(), true, watcher)
        observer = watcher
        // Cancel on the "Syncing music" notification goes to Melue's Rust code.
        SyncService.onCancel = { trigger("syncCancel", JSObject()) }
        // The network the app uses changing (Wi-Fi, mobile data, a VPN coming up): Rust tries the share again.
        val connectivity = activity.getSystemService(ConnectivityManager::class.java)
        connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                trigger("networkChanged", JSObject())
            }
        })
    }

    override fun onPause() {
        super.onPause()
        trigger("paused", JSObject())
    }

    override fun onResume() {
        super.onResume()
        trigger("resumed", JSObject())
    }

    override fun onDestroy() {
        CarQueue.appRunning = false
        observer?.let { activity.contentResolver.unregisterContentObserver(it) }
        observer = null
        super.onDestroy()
    }

    @InvokeArg
    class ApkArgs {
        var path: String = ""
    }

    /**
     * Updating Melue: hands a downloaded APK (in the app's cache) to Android's installer, which asks the user and
     * installs it only when it's signed like this app. The first time, Android needs the user to allow Melue to install
     * apps: its settings screen for that opens, and this rejects with "allow" (the app asks to tap Update again).
     */
    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(ApkArgs::class.java)
        val file = java.io.File(args.path)
        if (!file.exists()) {
            invoke.reject("The update's file is gone")
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !activity.packageManager.canRequestPackageInstalls()) {
            activity.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            invoke.reject("allow")
            return
        }
        try {
            val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
            val install = Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(install)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Can't install it: ${e.message}")
        }
    }

    @InvokeArg
    class ShareArgs {
        var path: String = ""
        var title: String = ""
    }

    /**
     * Hands a file in Melue's own folder (its log) to Android's share sheet: email, a chat, Drive... Through the
     * app's file provider (Tauri's, `<package>.fileprovider`, which covers shared storage).
     */
    @Command
    fun shareFile(invoke: Invoke) {
        val args = invoke.parseArgs(ShareArgs::class.java)
        val file = java.io.File(args.path)
        if (!file.exists()) {
            invoke.reject("There's no ${file.name} yet")
            return
        }
        try {
            val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
            val send = Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_STREAM, uri)
                .putExtra(Intent.EXTRA_SUBJECT, args.title)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            activity.startActivity(Intent.createChooser(send, args.title).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Can't share it: ${e.message}")
        }
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

    // ---- secrets (the share's password; see MelueSecrets)

    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.getArgs()
        val key = args.getString("key") ?: return invoke.reject("a secret needs a key")
        val value = args.getString("value") ?: return invoke.reject("a secret needs a value")
        runCatching { MelueSecrets.set(activity, key, value) }
            .onSuccess { invoke.resolve() }
            .onFailure { invoke.reject("can't store the secret: ${it.message}") }
    }

    @Command
    fun secretGet(invoke: Invoke) {
        val key = invoke.getArgs().getString("key") ?: return invoke.reject("a secret needs a key")
        runCatching { MelueSecrets.get(activity, key) }
            .onSuccess { invoke.resolve(if (it == null) JSObject() else JSObject().put("value", it)) }
            .onFailure { invoke.reject("can't read the secret: ${it.message}") }
    }

    @Command
    fun secretDelete(invoke: Invoke) {
        val key = invoke.getArgs().getString("key") ?: return invoke.reject("a secret needs a key")
        MelueSecrets.delete(activity, key)
        invoke.resolve()
    }

    // ---- the car with Melue closed (CarQueue)

    /** Remembers the app's data folder (where the library is), for the car with Melue closed. */
    @Command
    fun rememberDataDir(invoke: Invoke) {
        val dir = invoke.getArgs().getString("dir") ?: return invoke.reject("rememberDataDir needs the folder")
        CarQueue.rememberDataDir(activity, dir)
        invoke.resolve()
    }

    /**
     * Moves a document (a file in a folder chosen with the picker) into another folder of it, keeping the file
     * itself (and its modified time). Answers the moved document's new address.
     */
    @Command
    fun moveDocument(invoke: Invoke) {
        val args = invoke.getArgs()
        val document = args.getString("document") ?: return invoke.reject("moveDocument needs the document")
        val from = args.getString("from") ?: return invoke.reject("moveDocument needs the folder it's in")
        val to = args.getString("to") ?: return invoke.reject("moveDocument needs the folder to move it to")
        runCatching {
            DocumentsContract.moveDocument(activity.contentResolver, Uri.parse(document), Uri.parse(from), Uri.parse(to))
                ?: throw IllegalStateException("the folder's provider can't move documents")
        }.onSuccess { invoke.resolve(JSObject().put("uri", it.toString())) }
            .onFailure { invoke.reject("can't move the file: ${it.message}") }
    }

    // ---- syncing

    /**
     * Moves songs from the music index (by file path) to Android's trash, after Android asks the user (its own
     * dialog; trashed files can be restored for 30 days in the Files app). Android 10 and older: deleted, as far
     * as Android lets Melue. Resolves with how many songs were found (the dialog decides the rest).
     */
    @Command
    fun trashMusic(invoke: Invoke) {
        val paths = invoke.getArgs().getJSONArray("paths")
        val uris = mutableListOf<Uri>()
        val collection = audioCollection()
        for (i in 0 until paths.length()) {
            val path = paths.getString(i)
            activity.contentResolver.query(
                collection, arrayOf(MediaStore.Audio.Media._ID), "${MediaStore.Audio.Media.DATA} = ?", arrayOf(path), null,
            )?.use { cursor ->
                if (cursor.moveToFirst()) uris.add(android.content.ContentUris.withAppendedId(collection, cursor.getLong(0)))
            }
        }
        if (uris.isNotEmpty()) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                val request = MediaStore.createTrashRequest(activity.contentResolver, uris, true)
                activity.startIntentSenderForResult(request.intentSender, 4711, null, 0, 0, 0)
            } else {
                for (uri in uris) runCatching { activity.contentResolver.delete(uri, null, null) }
            }
        }
        invoke.resolve(JSObject().put("found", uris.size))
    }

    /** The phone's name, as set in its settings (About phone), or its model. */
    @Command
    fun deviceName(invoke: Invoke) {
        val name = Settings.Global.getString(activity.contentResolver, Settings.Global.DEVICE_NAME)
        invoke.resolve(JSObject().put("name", if (name.isNullOrBlank()) Build.MODEL else name))
    }

    /** Whether the connection is metered (mobile data, a phone's hotspot): Android's own answer. */
    @Command
    fun networkMetered(invoke: Invoke) {
        val connectivity = activity.getSystemService(ConnectivityManager::class.java)
        invoke.resolve(JSObject().put("metered", connectivity.isActiveNetworkMetered))
    }

    /** Shows or updates the "Syncing music" notification: `done` of `total` files, `text` the current one. */
    @Command
    fun syncNotification(invoke: Invoke) {
        val args = invoke.getArgs()
        SyncService.show(activity.applicationContext, args.getInteger("done") ?: 0, args.getInteger("total") ?: 0, args.getString("text") ?: "")
        invoke.resolve()
    }

    @Command
    fun syncNotificationDone(invoke: Invoke) {
        SyncService.stop(activity.applicationContext)
        invoke.resolve()
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
