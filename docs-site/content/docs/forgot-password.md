---
title: Forgot your Password?
description: Where the Punktfunk web console login password lives, and how to reset it, on each host platform.
---

The Punktfunk **web console** (status, paired devices, PIN pairing) is protected by a login
password. That password is generated — or, on Windows, chosen — when the console is first set up, and
it lives on the **host**. So if you can't get past the login screen, you reset it on the host
machine itself, not from the browser.

New to the console? See [The Web Console](/docs/web-console) to enable it and arm pairing.

> This is **only** the web console login. It is **not** your client/device pairing — if a client
> won't connect, that's [Pairing](/docs/pairing), not this password.

## Find your host

Find your host platform for exactly where the password lives, then read or reset it below:

| Host | Where the password lives | Section |
|------|--------------------------|---------|
| **Linux packages (apt / RPM / Arch / Bazzite / NixOS)** | `~/.config/punktfunk/web-password` | [Login password](/docs/web-console#login-password) |
| **SteamOS (host)** | `~/.config/punktfunk/web.env` | [Login password](/docs/web-console#login-password) |
| **Windows host** | `%ProgramData%\punktfunk\web-password` | [Login password](/docs/web-console#login-password) · [Windows Host](/docs/windows-host) |

## A forgotten password is reset, not recovered

The console stores a **salted argon2id hash**, not your password. A generated or typed password is
readable out of the file exactly once — until you first sign in, which is when the console replaces
that clear line with the hash. So there are two cases.

**You have not signed in yet.** Read it. On the **Linux packages** and the **SteamOS host**:

```sh
sed -n 's/^PUNKTFUNK_UI_PASSWORD=//p' ~/.config/punktfunk/web-password   # Linux packages
sed -n 's/^PUNKTFUNK_UI_PASSWORD=//p' ~/.config/punktfunk/web.env        # SteamOS host
```

On a **Windows host**, from an **elevated** PowerShell (the file is readable only by Administrators
and SYSTEM). This is also how you get the password after a winget or silent install, which never
displays one:

```powershell
punktfunk-host web password
```

**Nothing comes back.** Then it is hashed, and you set a new one instead: write a
`PUNKTFUNK_UI_PASSWORD=<your-password>` line back into the same file and restart the console. Your
next sign-in hashes it and signs every other session out — so a reset also boots anyone still
holding an old cookie. [Login password](/docs/web-console#login-password) has the exact steps for
each of the three platforms, and it is the one place that procedure is kept up to date.

## The password is right and it still won't let you in

The login screen says **"Wrong password."** for every failure, including two that have nothing to do
with the password you typed.

- **Too many attempts.** Five wrong guesses from the same device are free; every one after that
  arms a lockout that doubles — a second, two, four — up to **five minutes**. While it holds, even
  the correct password is refused. Wait it out, or clear it at once by restarting the console (the
  lockout is only kept in the console's memory):

  ```sh
  systemctl --user restart punktfunk-web
  ```

  ```powershell
  punktfunk-host service restart
  ```

  (The PowerShell one is Windows, from an **elevated** prompt — the console runs under the
  Punktfunk Host service there.)
- **No password is configured at all.** If the file is missing or empty, or a line lost its
  `PUNKTFUNK_UI_PASSWORD_HASH=` prefix, the console fails closed and admits nobody — a page you open
  answers `auth not configured: set PUNKTFUNK_UI_PASSWORD_HASH`. Put a clear line back —
  `PUNKTFUNK_UI_PASSWORD=<your-password>`, on its own line, nothing else on it — and restart the
  console as above; the next sign-in hashes it. On the Linux packages you can instead **delete**
  `~/.config/punktfunk/web-password` and run

  ```sh
  systemctl --user restart punktfunk-web-init punktfunk-web
  ```

  which generates a fresh password and starts the console with it — read it back with the command
  above, before you sign in.

Still stuck? See [Troubleshooting](/docs/troubleshooting).
