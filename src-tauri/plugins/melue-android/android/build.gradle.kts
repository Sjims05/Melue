plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.simon.melue.android"
    compileSdk = 34

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    implementation(project(":tauri-android"))
    // The car with Melue closed: CarQueue is the player's queue provider.
    implementation(project(":tauri-plugin-native-audio"))
    // Sharing the log with another app (FileProvider).
    implementation("androidx.core:core:1.13.1")
}
