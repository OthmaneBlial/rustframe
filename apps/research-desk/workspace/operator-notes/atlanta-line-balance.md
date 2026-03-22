---
title: Atlanta line balance walkthrough
collection: Operator Notes
kind: Note
tags: balancing, queue, labor
reviewer: Jules Varga
status: ready
priority: watch
summary: The issue is not an undersized station, but the lack of a clear handoff rule when one cell finishes early and another is still backlogged.
---
# Atlanta line balance walkthrough

The station map looks balanced on paper. On the floor, the problem is that early-finishing cells wait for instruction instead of moving into the backlog automatically.

## Suggested rule change

- Add one visible trigger for cross-cell help.
- Let the line lead reassign two operators without another approval step.
- Review the trigger every two hours during the pilot.
