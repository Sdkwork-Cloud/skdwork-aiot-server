# Jobs

Background workers and scheduled jobs for the SDKWork AIoT server.

No standalone worker binaries are defined in this repository yet. Protocol ingest, outbox, and dead-letter handling currently run inside `services/sdkwork-aiot-cloud-gateway`.

Future worker crates should follow `sdkwork-<domain>-<capability>-worker` naming from `../sdkwork-specs/NAMING_SPEC.md`.
