package com.simon.melue.android

import android.content.Context
import android.provider.Settings
import android.os.Build
import android.util.Log
import app.tauri.nativeaudio.ProvidedEntry
import app.tauri.nativeaudio.ProvidedPlayback
import app.tauri.nativeaudio.ProvidedQueue
import app.tauri.nativeaudio.QueueProvider
import app.tauri.nativeaudio.SetSourceArgs
import org.json.JSONArray
import org.json.JSONObject

/**
 * Synced playback in the car with Melue closed: the player's queue provider (named in this plugin's manifest,
 * see native-audio's QueueProvider). Melue's Rust code does the work (src/sync/car.rs, through JNI), with the
 * library in the app's data folder and the share's password from Android's secure store.
 *
 * While Melue's app is running it does all this itself, so the start-up queue and saving are left to it here.
 */
class CarQueue : QueueProvider {
    companion object {
        private const val TAG = "MelueCar"
        private const val PREFS = "melue_car"
        private const val DATA_DIR = "dataDir"
        /** The share's password in MelueSecrets (the same key as share/setup.rs's PASSWORD_KEY). */
        private const val PASSWORD_KEY = "share-password"

        /** Melue's app is running (its plugin is loaded). */
        @Volatile
        @JvmStatic
        var appRunning = false

        private val native: Boolean by lazy {
            runCatching { System.loadLibrary("melue_lib") }.onFailure { Log.w(TAG, "can't load Melue's library: $it") }.isSuccess
        }

        /** Melue's data folder (where the library is), remembered by the app when it runs. */
        fun rememberDataDir(context: Context, dir: String) {
            context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(DATA_DIR, dir).apply()
        }

        @JvmStatic external fun nativeQueueAtStart(dataDir: String, password: String?): String?
        @JvmStatic external fun nativeEntries(dataDir: String, password: String?, folderId: String): String?
        @JvmStatic external fun nativeQueueForEntry(dataDir: String, password: String?, entryId: String): String?
        @JvmStatic external fun nativePlayback(dataDir: String, password: String?, name: String, playback: String): String?
    }

    private fun dataDir(context: Context): String? =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(DATA_DIR, null)

    private fun password(context: Context): String? = runCatching { MelueSecrets.get(context, PASSWORD_KEY) }.getOrNull()

    /** Runs `call` with the data folder and password, when Melue's library can be reached. */
    private fun <T> withMelue(context: Context, call: (String, String?) -> T?): T? {
        val dir = dataDir(context) ?: return null
        if (!native) return null
        return runCatching { call(dir, password(context)) }.onFailure { Log.w(TAG, "the car's synced playback failed: $it") }.getOrNull()
    }

    private fun queueFrom(json: String?): ProvidedQueue? {
        val q = JSONObject(json ?: return null)
        val items = q.getJSONArray("items")
        return ProvidedQueue(
            items = (0 until items.length()).map { i ->
                val item = items.getJSONObject(i)
                SetSourceArgs().apply {
                    src = item.getString("src")
                    id = if (item.has("id") && !item.isNull("id")) item.getLong("id") else null
                    title = item.optString("title").takeIf { it.isNotEmpty() }
                    artist = item.optString("artist").takeIf { it.isNotEmpty() }
                    artworkUrl = item.optString("artworkUrl").takeIf { it.isNotEmpty() }
                }
            },
            index = q.getInt("index"),
            positionMs = q.getLong("positionMs"),
            shuffle = q.getBoolean("shuffle"),
            repeatMode = q.getString("repeat"),
        )
    }

    override fun queueAtStart(context: Context): ProvidedQueue? {
        if (appRunning) return null
        return queueFrom(withMelue(context) { dir, password -> nativeQueueAtStart(dir, password) })
    }

    override fun entries(context: Context, folderId: String): List<ProvidedEntry> {
        val json = withMelue(context) { dir, password -> nativeEntries(dir, password, folderId) } ?: return emptyList()
        val entries = JSONArray(json)
        return (0 until entries.length()).map { i ->
            val e = entries.getJSONObject(i)
            ProvidedEntry(e.getString("id"), e.getString("title"), e.optString("subtitle").takeIf { it.isNotEmpty() })
        }
    }

    override fun queueForEntry(context: Context, entryId: String): ProvidedQueue? =
        queueFrom(withMelue(context) { dir, password -> nativeQueueForEntry(dir, password, entryId) })

    override fun onPlayback(context: Context, playback: ProvidedPlayback) {
        if (appRunning) return
        val json = JSONObject()
            .put("itemIds", JSONArray(playback.itemIds.map { it ?: JSONObject.NULL }))
            .put("index", playback.index)
            .put("positionMs", playback.positionMs)
            .put("playing", playback.playing)
            .put("shuffle", playback.shuffle)
            .put("repeatMode", playback.repeatMode)
            .put("title", playback.title ?: JSONObject.NULL)
            .put("artist", playback.artist ?: JSONObject.NULL)
        withMelue(context) { dir, password -> nativePlayback(dir, password, phoneName(context), json.toString()) }
    }

    /** The phone's name, as set in its settings, or its model (the app's own name setting wins, in Rust). */
    private fun phoneName(context: Context): String =
        Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() } ?: Build.MODEL
}
