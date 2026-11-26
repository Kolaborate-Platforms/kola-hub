This is an early test build. Please use copies of your files, and tell us what goes wrong by opening an issue.

## Windows 10 and 11 (64-bit)

1. Download the file ending in `setup.exe`.
2. Open it. Windows may say "Windows protected your PC" because the installer is not signed yet. Choose
   **More info**, then **Run anyway**.
3. It installs for your user only and does not need an administrator.

## Mac (Apple silicon: M1 or newer)

1. Download the file ending in `.dmg`, open it and drag Kola Hub into Applications.
2. Open Kola Hub. macOS will refuse the first time because the app is not signed with an Apple certificate.
   Open **System Settings**, then **Privacy & Security**, and choose **Open Anyway**.

## Check your download (optional)

Compare the file's SHA-256 with its line in `SHA256SUMS`. On Windows: `certutil -hashfile <file> SHA256`.
On a Mac: `shasum -a 256 <file>`.
