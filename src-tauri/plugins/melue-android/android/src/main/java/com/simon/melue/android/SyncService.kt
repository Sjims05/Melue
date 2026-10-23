package com.simon.melue.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * The "Syncing music" notification while Melue syncs with the share: progress (a bar, "12 of 120") and a
 * Cancel button. A foreground service, so Android keeps the sync going while Melue is in the background.
 * Started and updated by the plugin's syncNotification command, stopped by syncNotificationDone.
 */
class SyncService : Service() {
    companion object {
        private const val CHANNEL = "melue_sync"
        private const val ID = 7451
        private const val ACTION_CANCEL = "com.simon.melue.android.SYNC_CANCEL"
        private const val EXTRA_DONE = "done"
        private const val EXTRA_TOTAL = "total"
        private const val EXTRA_TEXT = "text"

        /** Called when Cancel is pressed (the plugin passes it on to Melue's Rust code). */
        @Volatile
        var onCancel: (() -> Unit)? = null

        @Volatile
        private var running = false

        /** Asked to start, not started yet (Android starts it a moment later). */
        @Volatile
        private var starting = false

        /**
         * Over before it started (a sync that had nothing to do): stopped once it has started. Stopping it earlier
         * makes Android end Melue (a service started this way has to show its notification first).
         */
        @Volatile
        private var stopWhenStarted = false

        private fun channel(context: Context) {
            val manager = context.getSystemService(NotificationManager::class.java)
            if (manager.getNotificationChannel(CHANNEL) == null) {
                manager.createNotificationChannel(
                    NotificationChannel(CHANNEL, "Syncing music", NotificationManager.IMPORTANCE_LOW).apply {
                        description = "While Melue copies music from your share"
                    },
                )
            }
        }

        private fun build(context: Context, done: Int, total: Int, text: String): Notification {
            channel(context)
            val cancel = PendingIntent.getService(
                context,
                0,
                Intent(context, SyncService::class.java).setAction(ACTION_CANCEL),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            val open = context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
                PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            }
            return Notification.Builder(context, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle(if (total > 0) "Syncing music: ${done + 1} of $total" else "Syncing music")
                .setContentText(text)
                .setProgress(total, done, total == 0)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setContentIntent(open)
                .addAction(Notification.Action.Builder(null, "Cancel", cancel).build())
                .build()
        }

        /** Shows or updates the notification (starting the service the first time). */
        fun show(context: Context, done: Int, total: Int, text: String) {
            stopWhenStarted = false
            if (!running && !starting) {
                val start = Intent(context, SyncService::class.java)
                    .putExtra(EXTRA_DONE, done)
                    .putExtra(EXTRA_TOTAL, total)
                    .putExtra(EXTRA_TEXT, text)
                try {
                    context.startForegroundService(start)
                    starting = true
                    return
                } catch (e: Exception) {
                    // Android may not allow starting it now (the app in the background): a plain notification.
                }
            }
            context.getSystemService(NotificationManager::class.java).notify(ID, build(context, done, total, text))
        }

        /** The sync is over: the notification goes. */
        fun stop(context: Context) {
            if (starting && !running) {
                stopWhenStarted = true
                return
            }
            context.stopService(Intent(context, SyncService::class.java))
            context.getSystemService(NotificationManager::class.java).cancel(ID)
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_CANCEL) {
            onCancel?.invoke()
            return START_NOT_STICKY
        }
        val notification = build(
            this,
            intent?.getIntExtra(EXTRA_DONE, 0) ?: 0,
            intent?.getIntExtra(EXTRA_TOTAL, 0) ?: 0,
            intent?.getStringExtra(EXTRA_TEXT) ?: "",
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(ID, notification)
        }
        running = true
        starting = false
        if (stopWhenStarted) {
            stopWhenStarted = false
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        running = false
        starting = false
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
