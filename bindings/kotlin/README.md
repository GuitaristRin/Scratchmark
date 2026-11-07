# Android 封装（Kotlin + JNI）

验证等级：**可编译、符号与依赖可核对**。本机没有设备/模拟器连着，运行时行为未验证（见 `docs/PLATFORMS.md`）。

## 组成

| 层 | 位置 | 说明 |
| --- | --- | --- |
| JNI 层 | `crates/scratchmark-jni` | 24 个 `Java_com_scratchmark_Native_*` 导出；只做编组，语义来自 `scratchmark-core` |
| Kotlin 封装 | `bindings/kotlin/src/main/kotlin/com/scratchmark` | `Engine` / `Sample` / `BrushPoint` / `SampleFlags` / `BrushKind` |
| AAR 骨架 | `bindings/kotlin/build.gradle.kts` | 把 `src/main/jniLibs` 下的 `.so` 打进 AAR |

## 构建

```powershell
pwsh tools/build-android.ps1                      # 三个 ABI 的 .so → dist/android/jniLibs
Copy-Item dist/android/jniLibs -Destination bindings/kotlin/src/main/jniLibs -Recurse -Force
# 然后在 Android Studio / gradle 里 assembleRelease 出 AAR
```

`build-android.ps1` 会核对每个 ABI 的导出：C 层 52 个 `sm_*`，JNI 层 24 个 `Java_com_scratchmark_Native_*`，
并用 `llvm-strip` 去掉符号（约 0.33–0.51 MB/ABI）。

## 用法

```kotlin
Engine().use { engine ->
    engine.begin(width = 3.0, color = 0x11223344)
    engine.append(listOf(Sample(0.0, 0.0, 0L), Sample(40.0, 10.0, 8_000L)))
    engine.finish()

    // 渲染：拿逐点笔宽自己画多边形
    val shape = engine.brushPoints(0)

    // 局部擦除：沿折线切分，切口端点会带 SampleFlags.CUT_END
    engine.erasePath(doubleArrayOf(20.0, 0.0), radius = 8.0)
    val flags = engine.points(0).last().flags

    // 保存 / 打开
    val bytes = engine.save()
}
```

## 已知限制

- 未在设备或模拟器上运行过；JNI 参数与返回值类型只经过编译与符号核对。
- `Engine` 不是线程安全的：同一实例不要并发调用，并发场景各自开一个实例。
- 只打包了 `libscratchmark_jni.so`（Kotlin 用）与 `libscratchmark.so`（C 用），两者都进 AAR。
- 没有发布到 Maven；目前是源码 + 预编译原生库的骨架。
