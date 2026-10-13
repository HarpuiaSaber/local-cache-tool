# Service Template Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Load message templates and Kafka topics from independently configured service directories.

**Architecture:** `message.rs` owns service metadata, scanning, and validation. `main.rs` owns selected service and selected target state, while Slint exposes the service selector and its read-only topic. Kafka connection credentials remain shared; `Connection.topic` always comes from the selected service.

**Tech Stack:** Rust 2024, Serde JSON, Slint, rdkafka, Cargo tests.

**Spec:** `docs/superpowers/specs/2026-10-04-service-template-layout-design.md`

## Global Constraints

- Scan immediate subdirectories of `services`, each containing `service.json` and `templates`.
- `service.json` requires non-empty `name` and `topic`.
- Target names are unique within a service and may repeat in different services.
- Broker, username, and password remain shared Kafka configuration; service topic is read-only in the UI.
- Kafka key remains the message `target`; no Kafka partition is set explicitly.
- Package the entire `services` directory beside the executable.
- Do not use Git commands or create commits.

---

### Task 1: Service model and scanner

**Files:**
- Modify: `src/message.rs`
- Test: `src/message.rs`

**Interfaces:**
- Produces `pub struct Service { pub id: String, pub name: String, pub topic: String, pub templates: Vec<Template> }`.
- Produces `pub fn load_services(dir: &Path) -> Result<Vec<Service>, String>`.

- [ ] **Step 1: Write failing tests for valid services and duplicate target scope**

Add temporary-directory fixtures to the existing test module. Assert the scanner accepts the same target in two services but rejects it twice in one service.

```rust
#[test]
fn allows_a_target_name_in_different_services() {
    let root = service_fixture(&[
        ("cache", "Cache", "cache-topic", &["ChannelConfig"]),
        ("notification", "Notification", "notification-topic", &["ChannelConfig"]),
    ]);
    assert_eq!(load_services(root.path()).unwrap().len(), 2);
}

#[test]
fn rejects_duplicate_targets_in_one_service() {
    let root = service_fixture_with_template_files("cache", &["a", "b"], "ChannelConfig");
    assert!(load_services(root.path()).is_err());
}
```

- [ ] **Step 2: Run the new tests and confirm they fail**

Run:

```powershell
cargo test allows_a_target_name_in_different_services
```

Expected: compilation failure because `load_services` does not exist.

- [ ] **Step 3: Add the service data types and scanner**

Add a deserializable private metadata type and the public runtime service type:

```rust
#[derive(Deserialize)]
struct ServiceFile { name: String, topic: String }

pub struct Service {
    pub id: String,
    pub name: String,
    pub topic: String,
    pub templates: Vec<Template>,
}
```

Implement `load_services` by reading `dir`, sorting immediate child paths, reading each child `service.json`, validating non-empty metadata, and reusing a helper that loads templates from `child/templates`. Keep target uniqueness local to that helper call.

- [ ] **Step 4: Add missing-metadata validation tests**

```rust
#[test]
fn rejects_a_service_without_a_topic() {
    let root = service_fixture(&[("cache", "Cache", "", &["ChannelConfig"])]);
    assert!(load_services(root.path()).is_err());
}
```

- [ ] **Step 5: Run the message test module**

Run:

```powershell
cargo test message::tests
```

Expected: all message tests pass.

### Task 2: Service selection state and sending topic

**Files:**
- Modify: `src/main.rs`
- Modify: `src/message.rs`
- Test: `src/message.rs`

**Interfaces:**
- Consumes `message::Service` and `message::load_services` from Task 1.
- `AppState` stores `services: Vec<Service>`, `selected_service: usize`, `selected_target: usize`, and fields for the selected template.

- [ ] **Step 1: Replace template startup loading with service startup loading**

Replace `template_dir()` with `service_dir()`, preserving the executable-adjacent fallback pattern:

```rust
fn service_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("services")))
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("services"))
}
```

Initialize `AppState` from `load_services(&service_dir())`. Set `Connection.topic` from `state.service().topic` instead of an editable UI topic field.

- [ ] **Step 2: Add service selection helpers**

Implement methods whose names are used by UI callbacks:

```rust
fn service(&self) -> Option<&Service>;
fn template(&self) -> Option<&Template>;
fn select_service(&mut self, index: usize);
```

`select_service` resets `selected_target` to zero, initializes fields from the service's first template, and causes target list and preview refreshes.

- [ ] **Step 3: Verify all message construction still uses target as key**

Keep the existing `kafka::message_key(&payload)` call in the send callback. The selected service only supplies `Connection.topic`; the JSON target and Kafka key stay unchanged.

- [ ] **Step 4: Run targeted unit tests**

Run:

```powershell
cargo test message::tests kafka::tests
```

Expected: service scanner tests, existing message tests, and Kafka key tests pass.

### Task 3: Service selector UI

**Files:**
- Modify: `ui/app.slint`
- Modify: `src/main.rs`

**Interfaces:**
- Adds Slint properties `services`, `selected-service`, and `service-topic`.
- Adds callback `service-selected(string)`.

- [ ] **Step 1: Add the service selector and topic display**

Place this above the existing target selector in the message card:

```slint
HorizontalBox {
    spacing: 8px;
    Text { text: "Dịch vụ"; width: 88px; color: #355b7d; }
    ComboBox {
        model: root.services;
        current-value <=> root.selected-service;
        selected(value) => { root.service-selected(value); }
    }
    Text { text: "Topic"; width: 45px; color: #355b7d; }
    Text { text: root.service-topic; color: #123a63; horizontal-stretch: 1; }
}
```

- [ ] **Step 2: Remove editable Kafka topic input**

Delete the Topic `LineEdit` from the shared connection form. Keep Broker, Tài khoản, and Mật khẩu. Connection testing and sending construct `Connection` using `ui.get_service_topic()`.

- [ ] **Step 3: Wire the Rust callback**

Populate service labels from `Service.name`, set `service-topic` from the selected service, and in `on_service_selected` find the service by name, reset its target, then call `update_fields` and `update_preview`.

- [ ] **Step 4: Compile the Slint UI and tests**

Run:

```powershell
cargo test --release --target-dir target-nmake -q
```

Expected: Slint compilation succeeds and all ordinary tests pass.

### Task 4: Service files, second service example, and packaging

**Files:**
- Create: `services/cache/service.json`
- Create: `services/cache/templates/channel-config.json`
- Create: `services/cache/templates/user-profile.json`
- Create: `services/notification/service.json`
- Create: `services/notification/templates/channel-config.json`
- Modify: `package.ps1`
- Delete: `templates/channel-config.json`
- Delete: `templates/user-profile.json`

**Interfaces:**
- Service example metadata matches `load_services` from Task 1.
- Packaging copies `services` beside `kafka-message-tool.exe`.

- [ ] **Step 1: Create cache service metadata**

```json
{
  "name": "Cache Service",
  "topic": "my-awesome-topic"
}
```

Move the two existing templates unchanged into `services/cache/templates`.

- [ ] **Step 2: Create a notification service example**

Create `services/notification/service.json`:

```json
{
  "name": "Notification Service",
  "topic": "notification-events"
}
```

Create `services/notification/templates/channel-config.json` to demonstrate a duplicate target name in another service:

```json
{
  "target": "ChannelConfig",
  "fields": [
    { "name": "channelId", "kind": "text" },
    { "name": "eventType", "kind": "select", "options": ["CREATED", "DISABLED"] }
  ]
}
```

- [ ] **Step 3: Package the new directory**

Replace the template copy operation in `package.ps1`:

```powershell
Copy-Item -LiteralPath (Join-Path $root 'services') -Destination $bundle -Recurse
```

- [ ] **Step 4: Run formatting and all release tests**

Run:

```powershell
cargo fmt --check
cargo test --release --target-dir target-nmake -q
```

Expected: formatting check succeeds; ordinary tests pass, with the existing Kafka integration test ignored.

- [ ] **Step 5: Build only when requested**

Do not run `build-windows.bat` unless the user explicitly requests a new executable or ZIP. When requested, confirm that the resulting ZIP includes `services/cache` and `services/notification`.

## Plan Self-Review

- Spec coverage: Tasks 1–2 cover service scanning, local target uniqueness, shared credentials, service topic sending, and no fixed partition. Task 3 covers service/topic UI. Task 4 covers migration, a second service example, and packaging.
- Placeholder scan: no TBD, TODO, or implicit validation steps remain.
- Type consistency: Task 1 defines `Service` and `load_services`; Task 2 consumes them; Task 3 uses service selector properties; Task 4 creates the directory shape consumed by Task 1.
