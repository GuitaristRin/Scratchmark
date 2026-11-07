# 引擎通过 JNI 调用，不需要额外 keep 规则；保留类名以免混淆后 JNI 找不到符号。
-keep class com.scratchmark.Native { *; }
