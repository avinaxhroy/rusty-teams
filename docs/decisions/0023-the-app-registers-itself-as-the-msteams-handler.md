---
status: proposed
date: 2026-11-23
---

# ADR 0023: The app registers itself as the msteams handler

## Context

Clicking a `msteams://` link anywhere in the OS must open Rusty Teams and join that
meeting, like the Teams desktop client. The app never registered the scheme with the
OS, so an outside helper existed as a workaround. Teams on each OS uses that OS's
native registration mechanism; there is no cross-platform registry call.

Options: stay helper-only (rejected: extra component, no OS integration), or
register on each OS with the mechanism the OS actually allows (ADR 0004's rule).

## Decision

The app registers itself, per OS, at startup (non-fatal, skipped in demo):

- macOS: `CFBundleURLTypes`/`CFBundleURLSchemes` with `msteams` in the bundle's
  Info.plist (build-macos.sh), plus the runtime `NSWorkspace` scheme registration
  gpui exposes as `App::register_url_scheme` for unbundled runs.
- Linux: `MimeType=x-scheme-handler/msteams;` in every `.desktop` entry the project
  writes (self-install entry in `linux_desktop.rs`, the packaged entry, AppImage and
  package trees that copy it). The desktop environment then offers the app for the
  scheme; no xdg-mime call at startup.
- Windows: `HKEY_CURRENT_USER\Software\Classes\msteams` values pointing at the
  running executable (`URL Protocol`, `DefaultIcon`, `shell\open\command` with
  `"…teams.exe" "%1"`), the same place the Teams installer writes.

A launched URL reaches the app as a command-line argument or an open-URLs event;
both are normalized to the single-instance `join <url>` command, so a running
instance gets it through the existing single-instance pipe and a cold start
self-queues it. The join then goes through the pre-join screen like every other
join (no direct-join command exists anymore).

## Consequences

- The helper era ends: the app catches the links itself. (The "Teams on the web"
  call fallback — the helper `--join` used to go through — was already removed in
  `chore(calls): remove the Teams on the web call path`; this closes the last gap,
  the OS never launching us at all.)
- Registration is best-effort: a failure logs and the app starts anyway.
- On Linux, flatpak's sandbox needs no extra manifest key: the desktop file's
  `MimeType` inside the flatpak already exposes the handler to the host portal.
- The shadow call webview keeps cancelling its own `msteams:` launches
  (`block_app_launch`): a hidden WebView2 must not hand a running call back to the OS.
