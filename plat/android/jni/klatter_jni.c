/*
 * The JNI bridge between the Klatter app (KlatterNative.java) and the engine's
 * C interface (ffi/klatter.h), linked with rust/ into libklatter.so.
 *
 * Text crosses as UTF-8 bytes. Speaking pulls the engine's audio a block at a
 * time and hands each block to Java as 16-bit little-endian PCM; Java returns
 * nonzero to stop, which cancels the utterance at once.
 */
#include <jni.h>
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "../../../ffi/klatter.h"

#define JNI_FN(name) Java_com_dgomeze42_klatter_KlatterNative_##name

/* ---- Android 4.x on 32-bit ARM ------------------------------------------------
 * Rust's standard library and the unwinder refer to dl_iterate_phdr, which 32-bit
 * ARM Android only has from 5.0 (API 21): without it, the library would not load on
 * 4.x at all. This stand-in hands over to the real one when the system has it.
 * Without it, it reports this library alone, from its own program headers in
 * memory, found with dladdr (which every Android has). That is all the unwinder
 * needs, since Rust's code only ever unwinds through its own frames, so an internal
 * error is still caught and returned as an error code rather than ending the app.
 * Hidden, so it serves this library alone. */
#if defined(__arm__)
#include <dlfcn.h>
#include <elf.h>
#include <link.h>

typedef int (*dl_iterate_phdr_fn)(int (*)(struct dl_phdr_info *, size_t, void *), void *);

__attribute__((visibility("hidden")))
int dl_iterate_phdr(int (*callback)(struct dl_phdr_info *, size_t, void *), void *data) {
    static dl_iterate_phdr_fn real;
    static int looked;
    if (!looked) {
        void *libdl = dlopen("libdl.so", RTLD_NOW);
        if (libdl) real = (dl_iterate_phdr_fn)dlsym(libdl, "dl_iterate_phdr");
        looked = 1;
    }
    if (real && real != dl_iterate_phdr) return real(callback, data);

    Dl_info self;
    if (!dladdr((void *)&dl_iterate_phdr, &self) || !self.dli_fbase) return 0;
    const Elf32_Ehdr *header = (const Elf32_Ehdr *)self.dli_fbase;
    if (memcmp(header->e_ident, ELFMAG, SELFMAG) != 0) return 0;
    const Elf32_Phdr *phdr = (const Elf32_Phdr *)((const char *)header + header->e_phoff);
    /* The load bias: where the library is, less where its first segment asked to be. */
    Elf32_Addr lowest = (Elf32_Addr)-1;
    for (int i = 0; i < header->e_phnum; i++)
        if (phdr[i].p_type == PT_LOAD && phdr[i].p_vaddr < lowest) lowest = phdr[i].p_vaddr;
    if (lowest == (Elf32_Addr)-1) return 0;
    struct dl_phdr_info info;
    memset(&info, 0, sizeof info);
    info.dlpi_addr = (Elf32_Addr)(uintptr_t)self.dli_fbase - lowest;
    info.dlpi_name = self.dli_fname;
    info.dlpi_phdr = phdr;
    info.dlpi_phnum = header->e_phnum;
    return callback(&info, sizeof info, data);
}
#endif

/* A NUL-terminated copy of a Java byte array, or NULL. Free with free(). */
static char *utf8_of(JNIEnv *env, jbyteArray bytes) {
    if (!bytes) return NULL;
    jsize n = (*env)->GetArrayLength(env, bytes);
    char *s = malloc((size_t)n + 1);
    if (!s) return NULL;
    (*env)->GetByteArrayRegion(env, bytes, 0, n, (jbyte *)s);
    s[n] = 0;
    return s;
}

JNIEXPORT jint JNICALL JNI_FN(parameterCount)(JNIEnv *env, jclass cls) {
    (void)env; (void)cls;
    return (jint)klatter_parameter_count();
}

/* "code\tname\tunit\thelp" for setting index, or NULL. */
JNIEXPORT jstring JNICALL JNI_FN(parameterText)(JNIEnv *env, jclass cls, jint index) {
    (void)cls;
    KlatterParameterInfo info;
    if (index < 0 || klatter_parameter_info((size_t)index, &info) != 1) return NULL;
    size_t n = strlen(info.code) + strlen(info.name) + strlen(info.unit) + strlen(info.help) + 4;
    char *s = malloc(n);
    if (!s) return NULL;
    strcpy(s, info.code); strcat(s, "\t"); strcat(s, info.name); strcat(s, "\t");
    strcat(s, info.unit); strcat(s, "\t"); strcat(s, info.help);
    jstring out = (*env)->NewStringUTF(env, s);
    free(s);
    return out;
}

/* {min, max, default, step} for setting index, or NULL. */
JNIEXPORT jdoubleArray JNICALL JNI_FN(parameterNumbers)(JNIEnv *env, jclass cls, jint index) {
    (void)cls;
    KlatterParameterInfo info;
    if (index < 0 || klatter_parameter_info((size_t)index, &info) != 1) return NULL;
    jdouble v[4] = {info.min, info.max, info.default_value, info.step};
    jdoubleArray out = (*env)->NewDoubleArray(env, 4);
    if (out) (*env)->SetDoubleArrayRegion(env, out, 0, 4, v);
    return out;
}

JNIEXPORT jlong JNICALL JNI_FN(dictionaryNew)(JNIEnv *env, jclass cls) {
    (void)env; (void)cls;
    return (jlong)(intptr_t)klatter_pronunciations_create();
}

/* 1 when the word and its ARPAbet sounds were taken, -1 when not. */
JNIEXPORT jint JNICALL JNI_FN(dictionarySet)(JNIEnv *env, jclass cls, jlong dictionary, jbyteArray word,
                                            jbyteArray phones) {
    (void)cls;
    if (!dictionary) return -1;
    char *w = utf8_of(env, word), *p = utf8_of(env, phones);
    int r = (w && p) ? klatter_pronunciations_set((KlatterPronunciations *)(intptr_t)dictionary, w, p) : -1;
    free(w);
    free(p);
    return r;
}

JNIEXPORT void JNICALL JNI_FN(dictionaryFree)(JNIEnv *env, jclass cls, jlong dictionary) {
    (void)env; (void)cls;
    if (dictionary) klatter_pronunciations_free((KlatterPronunciations *)(intptr_t)dictionary);
}

/*
 * Speaks UTF-8 text with these settings, handing audio to sink.onAudio(byte[],
 * int) in blocks of at most chunk_bytes. Returns 0 when it was all spoken, 1 when
 * the sink stopped it, -1 on an error.
 */
JNIEXPORT jint JNICALL JNI_FN(speak)(JNIEnv *env, jclass cls, jbyteArray utf8, jdouble wpm, jdouble pitch,
                                    jdouble inflection, jdouble volume, jint sample_rate, jdoubleArray quality,
                                    jlong dictionary, jint chunk_bytes, jobject sink) {
    (void)cls;
    jclass sink_class = (*env)->GetObjectClass(env, sink);
    jmethodID on_audio = (*env)->GetMethodID(env, sink_class, "onAudio", "([BI)I");
    if (!on_audio) return -1;
    char *text = utf8_of(env, utf8);
    if (!text) return -1;
    double q[64];
    size_t qn = 0;
    if (quality) {
        jsize n = (*env)->GetArrayLength(env, quality);
        qn = n > 64 ? 64 : (size_t)n;
        (*env)->GetDoubleArrayRegion(env, quality, 0, (jsize)qn, q);
    }
    KlatterVoice voice = {wpm, pitch, inflection, volume};
    const char *texts[1] = {text};
    KlatterUtterance *u = klatter_utterance_create_quality(texts, NULL, 1, (uint32_t)sample_rate, &voice, q, qn,
                                                          (const KlatterPronunciations *)(intptr_t)dictionary);
    free(text);
    if (!u) return -1;
    size_t frames = (size_t)(chunk_bytes < 256 ? 256 : chunk_bytes) / 2;
    int16_t *pcm = malloc(frames * sizeof(int16_t));
    jbyteArray block = (*env)->NewByteArray(env, (jsize)(frames * 2));
    int result = -1;
    if (pcm && block) {
        for (;;) {
            KlatterPullBlock info;
            int status = klatter_utterance_read(u, pcm, frames, &info);
            if (status < 0) { result = -1; break; }
            if (info.sample_count > 0) {
                /* Android is little-endian, as the PCM Java expects. */
                (*env)->SetByteArrayRegion(env, block, 0, (jsize)(info.sample_count * 2), (const jbyte *)pcm);
                jint stop = (*env)->CallIntMethod(env, sink, on_audio, block, (jint)(info.sample_count * 2));
                if ((*env)->ExceptionCheck(env) || stop != 0) { result = 1; break; }
            }
            if (status == 0) { result = 0; break; }
        }
    }
    if (block) (*env)->DeleteLocalRef(env, block);
    free(pcm);
    klatter_utterance_free(u);
    return result;
}
