# Project agent guide

## Purpose

This is a Windows 64-bit desktop app for composing JSON cache-update messages and sending them to Kafka. It uses Rust, Slint, and `rdkafka`. Keep changes small and focused on the user's request. The user prefers to build the app themselves unless they explicitly ask you to build it. Do not use Git unless requested.

## Where to work

- `ui/app.slint`: window layout and UI callbacks. Keep the Send button accessible when forms grow.
- `src/main.rs`: UI state, preview updates, and the background send thread. Do not block the UI thread with Kafka calls.
- `src/message.rs`: template loading, validation, JSON preview, strict message creation, and unit tests.
- `src/kafka.rs`: Kafka producer settings and delivery result handling.
- `templates/*.json`: one form per target. Add new targets or fields here when code changes are unnecessary.
- `docker-compose.local.yml`: local Kafka, SCRAM users, topic, and Kafka UI. It advertises `localhost:9093`, which works for clients on the same machine.
- `build-windows.bat` and `package.ps1`: Windows Release build and ZIP packaging.

## Message and form rules

- Message fields are `target` (string), `action` (`INSERT`, `UPDATE`, `DELETE`, `RELOAD`), `data` (object or `null`), and `changedAt` (local timestamp with UTC offset).
- Unchecked data fields are omitted. If data is disabled or no fields are selected, `data` is `null`.
- A template field has `name` and `kind`: `text` for manual entry or `select` with a nonempty `options` array. Values sent in `data` are strings.
- The preview may show an empty selected text value while the user types. Sending must still reject empty required values. Preserve that distinction when changing validation.
- Templates are read from the `templates` directory beside the executable at startup. Package the directory with the `.exe`.

## Kafka and local environment

- The app uses `SASL_PLAINTEXT` with `SCRAM-SHA-512`. The local defaults are `localhost:9093` and `my-awesome-topic`.
- Credentials are entered in the UI and kept in memory. Do not add production passwords to source, templates, logs, or packages.
- The local Compose file provisions test users during KRaft formatting. Its volume is shared by `kafka-init` and `kafka`; preserve compatible write permissions and routable `advertised.listeners` in the init configuration.
- `localhost` in the broker's advertised address only supports clients on the Docker host. Remote clients need a reachable advertised host and corresponding network access.

## Verification and build

- For source-only changes, run `cargo fmt --check` when applicable and state clearly if compilation was not run.
- When the user requests a full build, run `build-windows.bat` on Windows. It uses the installed VS 2026 Build Tools, CMake, Strawberry Perl, the `NMake Makefiles` generator, and `target-nmake`.
- The batch runs Release tests, builds the `.exe`, and calls `package.ps1` to create a ZIP in `dist`. A first build may spend several minutes in `openssl-sys` because OpenSSL is compiled from source.
- The optional Kafka integration test in `src/kafka.rs` is ignored by ordinary tests and sends a real message when explicitly enabled.
- Do not claim a build or Kafka send succeeded without its actual output. An existing ZIP may predate the latest source edits.
