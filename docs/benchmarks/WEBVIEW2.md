# WebView2 installer choices

Research checked 25 November 2026 against the [Tauri v2 Windows installer documentation](https://v2.tauri.app/distribute/windows-installer/) and [Tauri's `webviewInstallMode` reference](https://v2.tauri.app/reference/config/), plus Microsoft's [WebView2 distribution guide](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution) and [supported Windows versions](https://learn.microsoft.com/microsoft-edge/webview2/).

The “additional size” figures below are Tauri's documented approximate costs, not measurements from this project. They describe added installer size, not installed size.

| `webviewInstallMode` | Clean Windows 10 with no internet | Added installer size (from docs, not measured) | Notes |
|---|---|---:|---|
| `skip` | Only works if WebView2 Runtime is already present. It cannot install a missing runtime. | 0 MB | Does not install WebView2. |
| `downloadBootstrapper` | No, if the runtime is missing. | 0 MB | Downloads and runs Microsoft's bootstrapper; requires internet. Tauri default. |
| `embedBootstrapper` | No, if the runtime is missing. | About 1.8 MB | Bundles the small bootstrapper, which still needs internet to fetch the runtime. |
| `offlineInstaller` | Yes, if the runtime is missing. | About 127 MB | Bundles and runs the Evergreen standalone installer. Microsoft's installer supports per-user installation when run without elevation. |
| `fixedRuntime` | Yes. | About 180 MB | Bundles a fixed WebView2 runtime and uses it for this app. The app must ship updates to keep that runtime current. |

Tauri's standard Windows installer defaults to a per-user app installation and does not require administrator rights. Selecting `perMachine`, or offering `both`, requires administrator rights. Microsoft's WebView2 bootstrapper and standalone runtime installer support per-user installs when not elevated, so the offline Evergreen option itself does not inherently require admin rights. `fixedRuntime` uses the bundled runtime rather than installing a shared runtime.

## Runtime availability and support

Microsoft says the Evergreen WebView2 Runtime is included in Windows 11, and that the vast majority of Windows 10 devices already have it, but a small number do not. It advises checking on the machine. Tauri's installer page says it ships with Windows 10 from the April 2018 release. Do not rely on `skip` until the actual machine has been checked.

Not checked: how long WebView2 will keep receiving updates on Windows 10, whose standard support ended on 14 October 2025. The three pages above do not say. Check Microsoft's lifecycle page before signing the shell decision.

For Kola Hub's clean, offline Windows 10 requirement, the two documented ways to handle a missing runtime are `offlineInstaller` and `fixedRuntime`. Both exceed the proposed 25 MB installer budget by a wide margin before counting the app. The test below should record actual installer size and installation behavior on the maintainer's clean test machine.

## Clean Windows 10 test plan

1. Use a clean 64-bit Windows 10 machine or VM with no network connection, no administrator account, and no prior WebView2 Runtime. Record Windows edition, version, build and whether the image is LTSC.
2. Confirm WebView2 Runtime is absent using Microsoft's documented detection method. Record the result.
3. Build the same Kola Hub app four times with `skip`, `downloadBootstrapper`, `embedBootstrapper` and `offlineInstaller`. Record each installer file size. Also build `fixedRuntime` if the project has a selected fixed-runtime folder available.
4. Copy each installer to the offline machine and run it as the standard user. Record whether installation succeeds, whether the app launches, any runtime prompt or error, and whether elevation is requested. For online-dependent modes, record the expected failure if the runtime is absent.
5. Repeat on a clean Windows 11 machine, then on a Windows 10 image with WebView2 already present, to check the `skip` path and the behavior when the runtime is present.
6. For each successful install, launch the app after a reboot and verify the local interface works while still offline. Record installer and installed sizes.

| Mode | Windows / image | Runtime present before? | Install works offline? | Admin prompt? | App launches offline? | Installer size | Installed size | Notes |
|---|---|---|---|---|---|---:|---:|---|
| `skip` | | | | | | | | |
| `downloadBootstrapper` | | | | | | | | |
| `embedBootstrapper` | | | | | | | | |
| `offlineInstaller` | | | | | | | | |
| `fixedRuntime` | | | | | | | | |
