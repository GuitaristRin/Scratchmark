// Android 库（AAR）骨架：只负责把预编译的 libscratchmark*.so 打进 AAR。
// 原生库用 tools/build-android.ps1 生成到 dist/android/jniLibs，构建前先拷到 src/main/jniLibs。
plugins {
    id("com.android.library") version "8.7.0"
    kotlin("android") version "2.0.21"
}

android {
    namespace = "com.scratchmark"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // 只用 JDK 自带的 ByteBuffer 与 JNI，不需要额外依赖。
}
