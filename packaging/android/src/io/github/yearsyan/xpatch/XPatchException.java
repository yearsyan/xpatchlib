package io.github.yearsyan.xpatch;

/** Thrown when a patch cannot be parsed, the base does not match, or the
 *  patched result fails its checksum. */
public class XPatchException extends RuntimeException {

    public XPatchException(String message) {
        super(message);
    }
}
