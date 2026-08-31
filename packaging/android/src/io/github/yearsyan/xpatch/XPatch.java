package io.github.yearsyan.xpatch;

/**
 * Deterministic binary delta patches for app update bundles (XPDL
 * format). The native library is bundled by the AAR; every method is
 * thread safe and never returns partially patched data.
 *
 * Replay only: patches are produced by the build toolchain / server side.
 * The AAR ships no patch production code at all — apps can only replay
 * patches they download.
 */
public final class XPatch {

    static {
        System.loadLibrary("xpatchlib_jni");
    }

    /** Algorithm names compiled into the native library. */
    public static native String[] nativeAlgorithms();

    /**
     * Replays {@code patch} against {@code base}. Verifies both the base
     * hash and the result hash; throws {@link XPatchException} on any
     * mismatch.
     */
    public static native byte[] nativeApply(byte[] patch, byte[] base);

    /**
     * Result size recorded in the patch envelope, or -1 when the patch
     * cannot be parsed. Lets callers pre-flight disk space before
     * downloading.
     */
    public static native long nativeResultSize(byte[] patch);

    public static String[] algorithms() {
        return nativeAlgorithms();
    }

    public static byte[] applyPatch(byte[] patch, byte[] base) {
        return nativeApply(patch, base);
    }

    public static long resultSize(byte[] patch) {
        return nativeResultSize(patch);
    }

    private XPatch() {}
}
