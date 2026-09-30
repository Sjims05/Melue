buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:9.3.1")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.2.10")
    }
}

subprojects {
    afterEvaluate {
        if (project.plugins.hasPlugin("com.android.library")) {
            configure<com.android.build.gradle.LibraryExtension> {
                compileSdk = 37
            }
        }
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

tasks.register("clean").configure {
    delete("build")
}

