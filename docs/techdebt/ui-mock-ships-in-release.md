# The browser mock ships inside the app

`ui/dist` includes `mock.js` and `en.json`. If `window.__TAURI__` were ever missing in the real window, the app
would show invented projects and appear to save. Leave the mock out of release builds, or show an error instead.
