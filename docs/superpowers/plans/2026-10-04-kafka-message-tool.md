# Kafka Message Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows desktop tool that creates and sends the specified cache-update JSON messages to Kafka.

**Architecture:** Deserialize target form definitions from JSON files next to the executable. Keep message construction separate from the Slint window and Kafka producer. The UI displays one selected field at a time and lets the user include any subset in `data`.

**Tech Stack:** Rust, Slint, serde, serde_json, chrono, rdkafka.

**Spec:** `docs/superpowers/specs/2026-10-04-kafka-message-tool-design.md`

## Global Constraints

- Windows x86-64 is the target platform.
- Default broker: `localhost:9093`; default topic: `my-awesome-topic`.
- Kafka uses SASL_PLAINTEXT and SCRAM-SHA-512; credentials are entered at runtime and never persisted.
- Action choices: INSERT, UPDATE, DELETE, RELOAD; default RELOAD.
- `changedAt` uses local time and an explicit UTC offset.
- Kafka records have no key.
- No Git operations, per user request.

---

### Task 1: Template loading and message construction

**Files:** Create `Cargo.toml`, `src/message.rs`, `templates/channel-config.json`.

**Interfaces:** `load_templates(path: &Path) -> Result<Vec<Template>, String>` loads and validates JSON definitions. `build_message(template: &Template, action: Action, data_enabled: bool, fields: &[FieldValue], changed_at: DateTime<FixedOffset>) -> Result<String, String>` serializes exactly the four top-level fields.

- [ ] Write tests for invalid templates, null data, selected text field, selected choice field, multiple fields, invalid choice, and explicit timestamp offset. Expected JSON values must be literal fixtures.
- [ ] Run `cargo test message` and confirm each new behavior fails because it is missing. If crates cannot be fetched, record the network blocker and keep the tests as unverified.
- [ ] Implement the smallest parser and message builder that satisfy the tests. Use serde_json for serialization; never interpolate unescaped user text into JSON.
- [ ] Run `cargo test message` and confirm all tests pass when dependencies are available.

### Task 2: Kafka delivery

**Files:** Create `src/kafka.rs`.

**Interfaces:** `send(config: Connection, payload: String) -> Result<String, String>` configures rdkafka with SASL_PLAINTEXT, SCRAM-SHA-512, and runtime credentials; it waits for delivery confirmation and returns a status or error.

- [ ] Add a focused test for connection-field validation before contacting Kafka.
- [ ] Run the test and confirm it fails for the missing behavior.
- [ ] Implement the producer with `FutureProducer` and a record without a key. Do not log credentials.
- [ ] Run `cargo test` and, when the local Docker broker is reachable, run a smoke send using the GUI.

### Task 3: Slint UI and Windows package

**Files:** Create `ui/app.slint`, `build.rs`, `src/main.rs`, `README.md`, `package.ps1`.

**Interfaces:** The window exposes target/action selection, a data toggle, field inclusion and editor controls, JSON preview, connection inputs, Send callback, and status. `main.rs` maps UI state to `build_message` and `kafka::send` without blocking the UI thread.

- [ ] Build the window with the agreed controls and a `ChannelConfig` example template.
- [ ] Connect callbacks so the preview reflects the selected target/action/field values; create a fresh timestamp immediately before each send.
- [ ] On Send, validate inputs, disable repeated sending while pending, send on a worker thread, and return delivery status through the Slint event loop.
- [ ] Run `cargo fmt --check`, `cargo test`, and `cargo build --release`. Where dependency download blocks these commands, report the exact blocker.
- [ ] Document Rust/MSVC prerequisites and package the release executable with `templates/`; verify the package contents.
