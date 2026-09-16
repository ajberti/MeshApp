# Next Commit: Nearby transport adapter

1. Keep the in-process Mesh Session tests green.
2. Add a native/Nearby adapter that maps radio events onto `CoreEvent` / `CoreAction`.
3. Do not start Android or iOS UI work until a device-to-device session transfers a Bundle.
