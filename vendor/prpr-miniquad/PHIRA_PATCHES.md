# Phira miniquad patch

This directory vendors `Mivik/prpr-miniquad` commit
`138679cab57d84637bbada4d7eef54da34c2871a`.

The local patch keeps Phira's Android host integration together with the exact
miniquad source revision. It:

- passes `MotionEvent.getEventTime()` to the existing five-argument Rust JNI
  implementation so input-latency compensation receives a monotonic event
  timestamp;
- wires Phira's data, cache, input-box, file-picker, export, DPI, and
  pause/resume lifecycle JNI hooks into the generated activity;
- delegates bounded `content://` imports to Phira's `ImportFiles` helper;
- guards fullscreen APIs by Android API level; and
- removes three `unused_mut` warnings required by the workspace's
  `clippy -D warnings` policy.

Keep this list and the vendored diff in sync when updating the upstream commit.
