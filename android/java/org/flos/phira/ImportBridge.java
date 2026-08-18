package org.flos.phira;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

import java.io.File;

import quad_native.QuadNative;

abstract class ImportBridge extends Activity {
    protected abstract boolean isResourcePack();

    @Override
    public void onCreate(Bundle state) {
        super.onCreate(state);
        Uri uri = getIntent().getData();
        if (uri == null) {
            finish();
            return;
        }
        new Thread(() -> {
            try {
                File output = ImportFiles.copyUriToCache(getApplicationContext(), uri);
                QuadNative.setExternalImport(output.getAbsolutePath(), isResourcePack());
                runOnUiThread(() -> {
                    startActivity(new Intent(this, MainActivity.class));
                    finish();
                });
            } catch (Exception error) {
                Log.e("SAPP", "Failed to import", error);
                runOnUiThread(this::finish);
            }
        }, "phira-external-import").start();
    }
}
