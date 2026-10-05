package com.simon.melue.android

import android.Manifest
import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
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
/** The Android Keystore key Melue's secrets are encrypted with. */
private const val SECRET_KEY_ALIAS = "melue-secrets"

class MelueAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    /** The permission this Android version uses for reading music. */
    private fun musicAlias() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) "audio" else "storage"

    private fun musicState(): PermissionState = getPermissionState(musicAlias()) ?: PermissionState.DENIED

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

    private fun photosState(): PermissionState = getPermissionState(photosAlias()) ?: PermissionState.DENIED

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

    // ---- secrets (the share's password)
    //
    // Encrypted with an AES key that lives in the Android Keystore (it can't be read out of the phone's
    // secure hardware), and kept in the app's private preferences as "iv:ciphertext" (Base64).

    private val secrets by lazy { activity.getSharedPreferences("melue_secrets", Context.MODE_PRIVATE) }

    private fun secretKey(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(SECRET_KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(SECRET_KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.getArgs()
        val key = args.getString("key") ?: return invoke.reject("a secret needs a key")
        val value = args.getString("value") ?: return invoke.reject("a secret needs a value")
        runCatching {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
            val encrypted = cipher.doFinal(value.toByteArray(Charsets.UTF_8))
            val stored = Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(encrypted, Base64.NO_WRAP)
            secrets.edit().putString(key, stored).apply()
        }.onSuccess { invoke.resolve() }.onFailure { invoke.reject("can't store the secret: ${it.message}") }
    }

    @Command
    fun secretGet(invoke: Invoke) {
        val key = invoke.getArgs().getString("key") ?: return invoke.reject("a secret needs a key")
        val stored = secrets.getString(key, null)
        if (stored == null) {
            invoke.resolve(JSObject()) // no "value": none stored
            return
        }
        runCatching {
            val (iv, encrypted) = stored.split(":", limit = 2).map { Base64.decode(it, Base64.NO_WRAP) }
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
            String(cipher.doFinal(encrypted), Charsets.UTF_8)
        }.onSuccess { invoke.resolve(JSObject().put("value", it)) }.onFailure { invoke.reject("can't read the secret: ${it.message}") }
    }

    @Command
    fun secretDelete(invoke: Invoke) {
        val key = invoke.getArgs().getString("key") ?: return invoke.reject("a secret needs a key")
        secrets.edit().remove(key).apply()
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
