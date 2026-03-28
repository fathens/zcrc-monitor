plugins {
    id("com.android.application")
}

android {
    namespace = "com.example.zcrc_monitor"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.example.zcrc_monitor"
        minSdk = 28
        targetSdk = 34
        versionCode = 1
        versionName = "0.3.1"
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}
