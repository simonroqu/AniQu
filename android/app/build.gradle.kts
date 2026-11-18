plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.roqu.aniqu"
    compileSdk = 37
    defaultConfig {
        applicationId = "dev.roqu.aniqu"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        // Wie build-rust.sh: Handy (arm64) und Waydroid/Emulator (x86_64).
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    buildFeatures { compose = true }
}

// Rust-Teil (aniqu-ffi) vor jedem Build: .so in jniLibs, Kotlin-Anbindung in java/uniffi.
val buildRust by tasks.registering(Exec::class) {
    workingDir = rootDir
    commandLine("sh", "build-rust.sh")
}
tasks.named("preBuild") { dependsOn(buildRust) }

dependencies {
    val compose = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(compose)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.6.3")
    implementation("androidx.media3:media3-exoplayer:1.11.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.11.1")
    implementation("androidx.media3:media3-ui:1.11.1")
    // Für die UniFFI-Anbindung
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
}
