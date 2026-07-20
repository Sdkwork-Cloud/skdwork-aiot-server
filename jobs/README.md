# Jobs

Background workers and scheduled jobs for the SDKWork AIoT server.

No standalone worker binaries are defined in this repository. Protocol ingest,
outbox, and dead-letter handling are explicit responsibilities of
`crates/sdkwork-aiot-device-edge-runtime`; they are not separate job processes.

Future worker crates should follow `sdkwork-<domain>-<capability>-worker` naming from `../sdkwork-specs/NAMING_SPEC.md`.
