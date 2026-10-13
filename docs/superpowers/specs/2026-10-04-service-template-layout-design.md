# Service template layout

## Goal

Organize message templates and Kafka topics by service while retaining one shared Kafka connection configuration. A target name may appear in more than one service.

## Filesystem layout

```text
services/
  cache/
    service.json
    templates/
      channel-config.json
  notification/
    service.json
    templates/
      channel-config.json
```

Each `service.json` contains:

```json
{
  "name": "Cache Service",
  "topic": "cache-update-topic"
}
```

The directory name is the stable service identifier. `name` is the label shown in the UI. `topic` is required and is used for every send from that service.

Template files retain the current format. Template target names must be unique only within one service.

## Loading and validation

At startup the app scans immediate subdirectories of `services`. It accepts a service only when both `service.json` and a `templates` directory exist.

`service.json` must have a non-empty `name` and `topic`. Each service must have at least one valid template. Service directory names and display names must be unique. Validation errors identify the affected service path.

Services and templates are sorted by filesystem path to keep UI ordering deterministic. If no valid services are available, the UI shows the load error and disables sending.

## User interface

A `Service` selector appears before `Mục tiêu`. Choosing a service updates the read-only displayed topic, available targets, editable fields, and message preview.

The shared Kafka section continues to collect broker, username, and password. Topic is no longer entered there; it comes from the selected service. Connection testing uses the selected service topic.

The selected service and target determine the Kafka key and the JSON `target`. Existing field omission, previews, message dialogs, and send behavior remain unchanged.

## Sending

The send operation builds its `Connection` from the shared broker credentials and the selected service topic. The Kafka key remains the message `target`. The app never sets a Kafka partition explicitly.

## Packaging and migration

`package.ps1` copies the complete `services` directory beside the executable. The app no longer reads the top-level `templates` directory.

The current `templates/channel-config.json` and `templates/user-profile.json` move to `services/cache/templates`. A new `services/cache/service.json` uses the present default topic `my-awesome-topic`.

## Verification

Unit tests cover service loading, missing metadata, duplicate target names within a service, and the same target name in different services. Existing message and Kafka key tests continue to run.

Manual verification checks that selecting a service changes its topic and targets, and that the packaged ZIP contains the service directory.
