# Security policy

TubbyKeys listens to global keyboard events and installs files from the internet, so security reports are taken seriously.

## Reporting a vulnerability

Report privately through GitHub: **Security → Report a vulnerability** on this repository. Please do not open a public issue.

Include what you found, how to reproduce it, and the impact.

## Supported versions

Only the latest release gets security fixes.

## What is in scope

- Anything that could leak key identities or typed text out of the key listener
- Marketplace downloads: path traversal, checksum bypass, oversized or malformed files, decoder crashes
- The webview: script injection through pack names or descriptions, or calling commands a window should not have

## Design notes

- The key listener reads only scan codes and key up/down state. It forwards row and position data, never characters.
- Marketplace downloads use HTTPS only, allow no redirects, and cap sizes. Each file must match the SHA-256 in the index, and every sound must decode before the pack is moved into place.
- Audio decoding uses symphonia, a memory-safe Rust decoder.
- The webview has a strict content security policy with no remote content. Each window gets only the commands it needs (`src-tauri/capabilities/`).
