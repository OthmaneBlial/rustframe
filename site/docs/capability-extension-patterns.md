# Capability Extension Patterns

RustFrame does not yet have a formal plugin API for richer native capabilities.

Until that exists, the safest extension path is still explicit and app-owned.

## Default Path

Stay on the stock runtime when the built-in surface is enough:

- window management
- SQLite
- scoped filesystem
- allowlisted shell commands
- dialogs
- clipboard

Do not eject just because you might want native depth later.

## When To Eject

Eject when the product needs a capability that does not fit the current runtime contract, for example:

- OS-specific integration points
- a narrower app-owned native bridge
- a domain-specific native helper
- platform APIs that are too specific to expose globally

The command is:

```bash
cargo run -p rustframe-cli -- eject <app>
```

## Recommended Extension Shape

When you add native code after ejection:

1. keep the new capability module isolated
2. keep the frontend bridge narrow and named
3. avoid exposing generic host access when a specific action will do
4. document the capability in the app docs and manifest comments

## Good Extension Examples

- a media organizer that needs a host thumbnail generator
- a research app that needs a domain-specific parser binary
- an internal tool that needs a company-specific sync client wrapper

## Bad Extension Examples

- exposing a general shell because one import job was needed
- exposing the full filesystem because one workspace root was not modeled
- adding broad native hooks that only one app actually needs

## Future Plugin Direction

If RustFrame grows a richer plugin story later, it should preserve the same values:

- explicit capability boundaries
- app-visible contracts
- small default surface
- honest platform support promises

Until then, ejection is the intentional extension point.
