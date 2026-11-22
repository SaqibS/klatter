# The engine calls these from native code by name (src/plat/android/jni).
-keep class com.dgomeze42.klatter.KlatterNative { native <methods>; }
-keep interface com.dgomeze42.klatter.KlatterNative$AudioSink { *; }
-keepclassmembers class * implements com.dgomeze42.klatter.KlatterNative$AudioSink {
    public int onAudio(byte[], int);
}
