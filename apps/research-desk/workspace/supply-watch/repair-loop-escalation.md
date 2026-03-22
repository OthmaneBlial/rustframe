---
title: Repair loop escalation memo
collection: Supply Watch
kind: Audit
tags: repair, cycle-time, backlog
reviewer: Mina Calder
status: queued
priority: critical
summary: Devices entering a second repair pass are not getting a separate queue, which hides the real cycle-time problem behind aggregate averages.
---
# Repair loop escalation memo

Second-pass repairs are currently mixed back into the standard queue. That makes the average cycle time look cleaner than the customer experience.

## What to separate

- First-pass repairs.
- Second-pass repairs.
- Units pending parts.

## Risk

Without separate queues, the team will continue to optimize the wrong average.
