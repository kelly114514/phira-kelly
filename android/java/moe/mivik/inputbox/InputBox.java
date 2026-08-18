package moe.mivik.inputbox;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.DialogInterface;
import android.text.InputType;
import android.view.Gravity;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

import java.util.concurrent.atomic.AtomicBoolean;

/** Minimal Java host for inputbox's stable JNI contract. */
public final class InputBox {
    private static volatile Activity activity;

    private InputBox() {}

    private static native void inputCallback(long callback, String text);

    public static void initialize(Activity value) {
        activity = value;
    }

    public static void clear(Activity value) {
        if (activity == value) {
            activity = null;
        }
    }

    public static String showInput(
        long callback,
        String title,
        String prompt,
        String defaultValue,
        String okLabel,
        String cancelLabel,
        String mode,
        boolean autoWrap,
        boolean scrollToEnd
    ) {
        Activity current = activity;
        if (current == null || current.isFinishing()) {
            return "Android activity is unavailable";
        }
        current.runOnUiThread(() -> showDialog(
            current,
            callback,
            title,
            prompt,
            defaultValue,
            okLabel,
            cancelLabel,
            mode,
            autoWrap,
            scrollToEnd
        ));
        return null;
    }

    private static void showDialog(
        Context context,
        long callback,
        String title,
        String prompt,
        String defaultValue,
        String okLabel,
        String cancelLabel,
        String mode,
        boolean autoWrap,
        boolean scrollToEnd
    ) {
        int padding = (int)(20 * context.getResources().getDisplayMetrics().density);
        LinearLayout content = new LinearLayout(context);
        content.setOrientation(LinearLayout.VERTICAL);
        content.setPadding(padding, 0, padding, 0);

        if (prompt != null && !prompt.isEmpty()) {
            TextView promptView = new TextView(context);
            promptView.setText(prompt);
            promptView.setPadding(0, 0, 0, padding / 2);
            content.addView(promptView);
        }

        EditText input = new EditText(context);
        input.setText(defaultValue == null ? "" : defaultValue);
        if ("password".equals(mode)) {
            input.setSingleLine(true);
            input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        } else if ("multiline".equals(mode)) {
            input.setSingleLine(false);
            input.setMinLines(4);
            input.setGravity(Gravity.TOP | Gravity.START);
            input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
            input.setHorizontallyScrolling(!autoWrap);
        } else {
            input.setSingleLine(true);
            input.setInputType(InputType.TYPE_CLASS_TEXT);
        }
        int selection = scrollToEnd ? input.length() : 0;
        input.setSelection(selection);
        content.addView(input);

        AtomicBoolean completed = new AtomicBoolean(false);
        DialogInterface.OnClickListener finish = (dialog, which) -> {
            if (completed.compareAndSet(false, true)) {
                inputCallback(callback, which == DialogInterface.BUTTON_POSITIVE ? input.getText().toString() : null);
            }
        };
        AlertDialog dialog = new AlertDialog.Builder(context)
            .setTitle(title)
            .setView(content)
            .setPositiveButton(okLabel, finish)
            .setNegativeButton(cancelLabel, finish)
            .create();
        dialog.setOnCancelListener(ignored -> {
            if (completed.compareAndSet(false, true)) {
                inputCallback(callback, null);
            }
        });
        dialog.setOnShowListener(ignored -> input.requestFocus());
        dialog.show();
    }
}
