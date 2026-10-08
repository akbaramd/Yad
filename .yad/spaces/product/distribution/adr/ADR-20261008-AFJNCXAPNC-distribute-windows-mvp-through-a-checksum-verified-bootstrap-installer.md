---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-AFJNCXAPNC
title: Distribute Windows MVP through a checksum-verified bootstrap installer
status: accepted
space: product/distribution
created: 2026-10-08T09:55:08Z
updated: 2026-10-08T09:55:09Z
tags:
- distribution
- windows
- installer
- github
---

# Distribute Windows MVP through a checksum-verified bootstrap installer

## Context

Yad should be installable without cloning the repository or installing the Rust toolchain. The Windows MVP also depends on a local embedding model and Docker-backed Qdrant.

## Decision

Publish versioned GitHub Release assets for the Windows x64 binary and optional pinned embedding model, each with SHA-256 sidecars. The root install.ps1 resolves the latest release, verifies checksums, installs yad.exe to %LOCALAPPDATA%\Yad\bin by default, adds that directory to persistent User PATH, optionally installs the model asset, detects Docker and can install Docker Desktop through winget. tools/install.ps1 remains the local/source installer and supports optional Machine scope.

## Consequences

End users get a one-command irm URL | iex installation after GitHub publication. Large binaries, models, dist output and runtime state remain outside Git. The GitHub release asset names become a stable distribution contract. Before publication the <OWNER>/<REPO> placeholder must be replaced with the real repository.
