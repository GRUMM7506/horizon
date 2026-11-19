plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// The game itself: built by cargo-ndk into src/main/jniLibs (docs/android-port.md).
val gameLibrary = file("src/main/jniLibs/arm64-v8a/libhorizon_android.so")

android {
    namespace = "dev.horizonport.android"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "dev.horizonport.android"
        // Android 8.0: AAudio, and what android-activity's GameActivity glue supports.
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += "arm64-v8a" }
    }

    signingConfigs {
        // A debug key kept in the repository on purpose: every build, local or CI, is signed
        // alike, so a new APK installs over the old one and the game's folder (the prepared
        // resources and the saves, removed with the app) stays. It protects nothing.
        getByName("debug") {
            storeFile = rootProject.file("debug.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }

    buildTypes {
        debug {
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    packaging {
        // Stored uncompressed and page-aligned: the system maps the library from the APK.
        jniLibs { useLegacyPackaging = false }
    }
    lint { abortOnError = false }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.core:core-ktx:1.13.1")
    // The Java half of android-activity 0.6's GameActivity glue, which the game's library links.
    implementation("androidx.games:games-activity:4.0.0")
}

val checkGameLibrary by tasks.registering {
    doLast {
        if (!gameLibrary.isFile) {
            throw GradleException(
                "The game's library is missing: $gameLibrary\n" +
                    "Build it first, from the repository root:\n" +
                    "  cargo ndk -t arm64-v8a --platform 26 -o android/app/src/main/jniLibs " +
                    "build -p horizon-android --release",
            )
        }
    }
}
tasks.named("preBuild") { dependsOn(checkGameLibrary) }
