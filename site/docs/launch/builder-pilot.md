# RustFrame builder pilot

Preparation only. No invitations have been sent and no participant results are claimed.

## Entry gate

Recruit after the exact public release passes registry-only installation and native launch. Use one version throughout the pilot. Link the verified release, quickstart, limitations and issue template in every invitation. The current missing npm package is a blocker to this public pilot.

## Invitation draft

I'm building RustFrame, a desktop toolkit for frontend developers working with local documents and SQLite. I'm looking for a few people who have a small local tool they actually want to build: a document queue, research desk or offline catalog.

Would you be interested in trying the quickstart and telling me where you get stuck? A small working tool and candid feedback are more useful than a star. Participation is optional, and there's no runtime telemetry or requirement to share private documents.

## Session protocol

1. Ask permission to retain anonymous notes. Do not record the screen or publish quotes without separate consent.
2. Give the participant the README and ask what they think the toolkit does, who it serves and where they would start.
3. Observe the documented install on their normal development machine. Record prerequisites separately from CLI download, npm install, native compilation and first interactive window.
4. Ask them to change one schema field, regenerate types, save a row and reopen the application.
5. Ask them to select a folder containing non-sensitive test documents and explain the permission boundary.
6. Ask them to package the app and locate their data/export. Record the first point where private maintainer help was necessary.
7. Ask what they would use it for next week and what would prevent them continuing.
8. Follow up after seven days only if they consented. Record continued use, abandonment or no response distinctly.

## Anonymous record

```text
Participant ID:
Consent to anonymous notes: yes/no
Consent to follow-up: yes/no
Release and OS:
Existing Rust/Node/native dependencies:
Understood purpose without help: yes/no + explanation
CLI installation time:
Dependency installation time:
First native compilation time:
Time to interactive window:
First unaided success: yes/no
First blocking command/error:
Maintainer intervention required:
Schema/edit/persistence result:
Folder/permissions result:
Packaging/export result:
Intended real project:
Seven-day outcome: continued / stopped / no response / pending
Accepted fix or explicit deferral:
```

Keep private participant notes outside Git. Publish only aggregate findings with consent; do not fabricate an interview, external app or adoption metric.

## Decision rules

With five participants, aim for four who understand the README and four who reach an unaided first window. These are small-pilot acceptance targets, not population statistics. Fix repeated blockers before recruiting another wave. Require actual continued use before prioritizing conditional roadmap features.

Use existing issues #7 (developer-loop measurements), #11 (installer upgrades), #12 (accessibility) and #6 (job cancellation design). Link evidence, reproduction, affected version, expected outcome and test command; do not duplicate an existing issue or label unapproved designs as ready.

## Technical story outline

Title: From a folder of Markdown to a desktop review queue

- Show the real Research Desk workflow and link the source/build version.
- Explain schema-driven local data, folder grants and database events with the public APIs used by the app.
- Include the actual installation prerequisites and measured build times.
- State where the approach fits, where Tauri/Electron may fit better and the current distribution limitations.
- Link the verified download, full demo and one concrete feedback question.

A complete evidence-based draft is in [technical-story-draft.md](technical-story-draft.md). Replace its publication gates with verified release links before posting. Recheck community posting rules at publication time and disclose being the creator. No outreach or posting is authorized by this preparation document.
