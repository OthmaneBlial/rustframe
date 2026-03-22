# Release Checklist

Use this before you hand a RustFrame app to real users.

## Host And Build

- Run `cargo run -p rustframe-cli -- doctor` on the native host you will package from.
- Run `cargo run -p rustframe-cli -- platform-check <app>` for the host row you are shipping.
- Confirm `packaging.version`, description, publisher, homepage, and icons are correct in `rustframe.json`.

## App Contract

- Run `cargo run -p rustframe-cli -- inspect <app>`.
- Confirm the security model is correct for the frontend trust level.
- Confirm filesystem roots are narrow and intentional.
- Confirm shell commands are named, bounded, and still needed.
- Confirm the schema version, seeds, and migrations match the release you are shipping.

## Packaging

- Run `cargo run -p rustframe-cli -- package <app> --verify`.
- Open the produced bundle on the target host and smoke-test the main workflow.
- Confirm packaged filesystem roots actually appear in the bundle.
- Confirm install and uninstall scripts match your delivery method.

## Distribution

- Sign or notarize the bundle with host tooling if your users or environment require it.
- Generate checksums for the published artifacts.
- Publish host-specific install notes with the release.
- Keep the release notes tied to `packaging.version`.

## Updates

- State the current update path clearly: manual, host-assisted, or app-managed.
- Confirm users know which bundle replaces which old install.
- Do not promise auto-updates if the product does not implement them.

## Support Boundaries

- State which host OS rows you actually validated.
- State which rows are not promised.
- State any known constraints around signing, enterprise deployment, or unsupported native integrations.
