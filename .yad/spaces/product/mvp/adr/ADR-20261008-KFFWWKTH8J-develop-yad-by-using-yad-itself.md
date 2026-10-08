---
yad: 1
schema: adr
schema_version: 1
id: ADR-20261008-KFFWWKTH8J
title: Develop Yad by using Yad itself
status: accepted
space: product/mvp
created: 2026-10-08T06:37:25Z
updated: 2026-10-08T06:37:28Z
tags:
- dogfooding
- workflow
---

# Develop Yad by using Yad itself

## Context

Yad is intended to preserve project continuity for agents and teams. Its own development is the strongest realistic test of whether the memory, record, search, lifecycle, and team workflows are usable.

## Decision

Yad development will dogfood Yad. Before substantial work, the agent should search Yad for relevant current decisions and lessons. After substantial work, it should record durable decisions as ADRs and useful experience, state, failures, or handoffs as Memory. Temporary implementation noise should not be stored.

## Consequences

The project continuously tests its own product assumptions. Stored knowledge must remain curated rather than becoming a raw activity log. Failures discovered during self-hosting become product feedback.
