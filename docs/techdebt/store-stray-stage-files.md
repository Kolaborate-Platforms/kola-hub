# Stage files from a killed save stay in the project folder

The vendored atomic writer stages a save as `.name.<pid>-<n>.craft_tmp` beside the target. If the app is
killed in the middle of a save, that hidden file stays. The kill test saw a few. They hold no user data and
never replace a real file, but they pile up. Fix: on open, remove `*.craft_tmp` files that are older than a
day in folders the project has written to. Not done because it deletes files outside `.kola-hub/`.
