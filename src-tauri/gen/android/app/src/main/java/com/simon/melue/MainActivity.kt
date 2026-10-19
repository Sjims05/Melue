package com.simon.melue

import android.os.Bundle
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Back with nothing left to go back to in the app: Melue goes to the background and keeps playing, like music
    // apps, instead of closing its screen. A closed screen comes back blank (Tauri doesn't recreate its page well
    // while the app keeps running for the music), and closing it could crash. Added before Tauri's own back handling
    // (added once its page is made), so that goes first: it goes back in the app while it can, then hands back here.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        moveTaskToBack(true)
      }
    })
  }
}
