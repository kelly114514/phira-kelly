package org.flos.phira;

import android.content.Context;
import android.net.Uri;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;

/** Safe, bounded copying for documents handed to the native game. */
public final class ImportFiles {
    public static final long MAX_IMPORT_BYTES = 512L * 1024L * 1024L;

    private ImportFiles() {}

    public static File copyUriToCache(Context context, Uri uri) throws IOException {
        if (uri == null || !"content".equals(uri.getScheme())) {
            throw new IllegalArgumentException("Only content:// imports are supported");
        }

        File output = File.createTempFile("phira-import-", ".bin", context.getCacheDir());
        boolean complete = false;
        try (InputStream input = context.getContentResolver().openInputStream(uri);
             FileOutputStream stream = new FileOutputStream(output)) {
            if (input == null) {
                throw new IOException("Cannot open imported file");
            }
            byte[] buffer = new byte[8192];
            long total = 0;
            int count;
            while ((count = input.read(buffer)) != -1) {
                total += count;
                if (total > MAX_IMPORT_BYTES) {
                    throw new IOException("Imported file exceeds the 512 MiB limit");
                }
                stream.write(buffer, 0, count);
            }
            complete = true;
            return output;
        } finally {
            if (!complete && output.exists() && !output.delete()) {
                output.deleteOnExit();
            }
        }
    }
}
