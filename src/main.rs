#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod kafka;
mod message;

use chrono::Local;
use kafka::Connection;
use message::{
    Action, FieldKind, FieldValue, Service, Template, beautify_json, build_message, load_services,
    preview_message, validate_field_entry, validate_field_input,
};
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();

struct AppState {
    services: Vec<Service>,
    selected_service: usize,
    selected_target: usize,
    action: Action,
    fields: Vec<FieldValue>,
}

impl AppState {
    fn new(services: Vec<Service>) -> Self {
        let fields = services
            .first()
            .and_then(|service| service.templates.first())
            .map(default_fields)
            .unwrap_or_default();
        Self {
            services,
            selected_service: 0,
            selected_target: 0,
            action: Action::Reload,
            fields,
        }
    }

    fn template(&self) -> Option<&Template> {
        self.service()?.templates.get(self.selected_target)
    }

    fn service(&self) -> Option<&Service> {
        self.services.get(self.selected_service)
    }

    fn select_service(&mut self, index: usize) {
        if let Some(service) = self.services.get(index) {
            self.selected_service = index;
            self.selected_target = 0;
            self.fields = service
                .templates
                .first()
                .map(default_fields)
                .unwrap_or_default();
        }
    }
}

fn default_fields(template: &Template) -> Vec<FieldValue> {
    template
        .fields
        .iter()
        .map(|field| FieldValue {
            name: field.name.clone(),
            value: String::new(),
        })
        .collect()
}

fn service_dir() -> PathBuf {
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("services")));
    if let Some(path) = beside_exe {
        if path.is_dir() {
            return path;
        }
    }
    PathBuf::from("services")
}

fn update_fields(ui: &AppWindow, state: &AppState) {
    let rows = state
        .template()
        .map(|template| {
            template
                .fields
                .iter()
                .zip(&state.fields)
                .map(|(definition, value)| {
                    let mut options = vec![SharedString::new()];
                    options.extend(definition.options.iter().cloned().map(SharedString::from));
                    UiField {
                        name: definition.name.clone().into(),
                        kind: match definition.kind {
                            FieldKind::Text => "text".into(),
                            FieldKind::Select => "select".into(),
                        },
                        value: value.value.clone().into(),
                        options: ModelRc::from(Rc::new(VecModel::from(options))),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ui.set_fields(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn update_service_ui(ui: &AppWindow, state: &AppState) {
    let tables = state
        .service()
        .map(|service| {
            service
                .templates
                .iter()
                .map(|template| SharedString::from(template.table.as_str()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ui.set_tables(ModelRc::from(Rc::new(VecModel::from(tables))));
    if let Some(service) = state.service() {
        ui.set_selected_service(service.name.as_str().into());
        ui.set_service_topic(service.topic.as_str().into());
    } else {
        ui.set_selected_service(SharedString::new());
        ui.set_service_topic(SharedString::new());
    }
    ui.set_selected_target(
        state
            .template()
            .map(|template| SharedString::from(template.target.as_str()))
            .unwrap_or_default(),
    );
    ui.set_selected_table(
        state
            .template()
            .map(|template| SharedString::from(template.table.as_str()))
            .unwrap_or_default(),
    );
}

fn update_preview(ui: &AppWindow, state: &AppState) {
    let Some(template) = state.template() else {
        ui.set_preview(SharedString::new());
        return;
    };
    let changed_at = Local::now().fixed_offset();
    match build_message(template, state.action, &state.fields, changed_at.clone()) {
        Ok(payload) => ui.set_preview(payload.into()),
        Err(error) => {
            let preview = preview_message(template, state.action, &state.fields, changed_at)
                .unwrap_or_default();
            ui.set_preview(preview.into());
            ui.set_status(error.into());
        }
    }
}

fn connection_from_ui(ui: &AppWindow, topic: String) -> Connection {
    Connection {
        broker: ui.get_broker().to_string(),
        topic,
        username: ui.get_username().to_string(),
        password: ui.get_password().to_string(),
    }
}

fn copy_to_clipboard(text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Không có nội dung để sao chép".into());
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use std::mem::size_of;
        use std::ptr::{copy_nonoverlapping, null_mut};
        use windows_sys::Win32::Foundation::GlobalFree;
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{
            GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
        };
        use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

        let text = text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        if OpenClipboard(null_mut()) == 0 {
            return Err("Không mở được clipboard".into());
        }
        if EmptyClipboard() == 0 {
            CloseClipboard();
            return Err("Không thể xóa clipboard hiện tại".into());
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * size_of::<u16>());
        if memory.is_null() {
            CloseClipboard();
            return Err("Không đủ bộ nhớ để sao chép".into());
        }
        let destination = GlobalLock(memory) as *mut u16;
        if destination.is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return Err("Không thể ghi vào clipboard".into());
        }
        copy_nonoverlapping(text.as_ptr(), destination, text.len());
        GlobalUnlock(memory);
        if SetClipboardData(CF_UNICODETEXT as u32, memory).is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return Err("Không thể sao chép vào clipboard".into());
        }
        CloseClipboard();
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        Err("Sao chép chỉ hỗ trợ trên Windows".into())
    }
}

fn show_delivery_notice(ui: &AppWindow, title: &str, message: String, success: bool) {
    ui.set_status(title.into());
    ui.set_notice_success(success);
    ui.set_notice_title(title.into());
    ui.set_notice_message(message.into());
    ui.set_notice_visible(true);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let services = match load_services(&service_dir()) {
        Ok(services) => services,
        Err(error) => {
            ui.set_status(error.into());
            Vec::new()
        }
    };
    let state = Rc::new(RefCell::new(AppState::new(services)));

    {
        let state_ref = state.borrow();
        ui.set_services(ModelRc::from(Rc::new(VecModel::from(
            state_ref
                .services
                .iter()
                .map(|service| SharedString::from(service.name.as_str()))
                .collect::<Vec<_>>(),
        ))));
        update_service_ui(&ui, &state_ref);
        if let Some(template) = state_ref.template() {
            ui.set_selected_target(template.target.as_str().into());
        }
        update_fields(&ui, &state_ref);
        update_preview(&ui, &state_ref);
    }

    {
        let state = state.clone();
        ui.on_field_input_accepted(move |index, value| {
            let state = state.borrow();
            state
                .template()
                .and_then(|template| template.fields.get(index as usize))
                .is_none_or(|definition| validate_field_input(definition, value.as_str()).is_ok())
        });
    }
    {
        let state = state.clone();
        ui.on_field_input_error_requested(move |index, value| {
            let state = state.borrow();
            state
                .template()
                .and_then(|template| template.fields.get(index as usize))
                .and_then(|definition| validate_field_entry(definition, value.as_str()).err())
                .unwrap_or_default()
                .into()
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_service_selected(move |service_name| {
            let Some(ui) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            if let Some(index) = state
                .services
                .iter()
                .position(|service| service.name == service_name.to_string())
            {
                state.select_service(index);
                update_service_ui(&ui, &state);
                update_fields(&ui, &state);
                update_preview(&ui, &state);
            }
        });
    }

    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_table_selected(move |table| {
            let Some(ui) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            if let Some(index) = state.service().and_then(|service| {
                service
                    .templates
                    .iter()
                    .position(|item| item.table == table.to_string())
            }) {
                state.selected_target = index;
                state.fields = state.template().map(default_fields).unwrap_or_default();
                ui.set_selected_target(
                    state
                        .template()
                        .map(|template| SharedString::from(template.target.as_str()))
                        .unwrap_or_default(),
                );
                update_fields(&ui, &state);
                update_preview(&ui, &state);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_copy_requested(move |text| {
            let Some(ui) = weak.upgrade() else { return };
            match copy_to_clipboard(text.to_string()) {
                Ok(()) => ui.set_status("Đã sao chép vào clipboard".into()),
                Err(error) => ui.set_status(error.into()),
            }
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_action_selected(move |action| {
            let Some(ui) = weak.upgrade() else { return };
            if let Ok(action) = Action::parse(action.as_str()) {
                let mut state = state.borrow_mut();
                state.action = action;
                update_preview(&ui, &state);
            }
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_field_value_changed(move |index, value| {
            let Some(ui) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            if let Some(field) = state.fields.get_mut(index as usize) {
                field.value = value.to_string();
                update_preview(&ui, &state);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_test_connection_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            let config = connection_from_ui(&ui, ui.get_service_topic().to_string());
            if let Err(error) = config.validate() {
                ui.set_status(error.into());
                return;
            }
            ui.set_status("Đang kiểm tra kết nối Kafka...".into());
            ui.set_busy(true);
            let weak = weak.clone();
            std::thread::spawn(move || {
                let result = kafka::test_connection(config);
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_busy(false);
                    match result {
                        Ok(status) => {
                            ui.set_connected(true);
                            ui.set_status(status.into());
                        }
                        Err(error) => ui.set_status(error.into()),
                    }
                });
            });
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_change_connection_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_connected(false);
            ui.set_status(SharedString::new());
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_json_beauty_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            match beautify_json(ui.get_manual_payload().as_str()) {
                Ok(formatted) => {
                    ui.set_manual_payload(formatted.into());
                    ui.set_status("Đã định dạng JSON.".into());
                }
                Err(error) => ui.set_status(format!("Không thể format JSON: {error}").into()),
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_close_notice_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_notice_visible(false);
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_send_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            if !ui.get_connected() {
                ui.set_status("Hãy kiểm tra kết nối Kafka trước.".into());
                return;
            }
            let (config, payload, key, key_status) = if ui.get_send_mode() == "Gửi tùy ý" {
                let topic = ui.get_manual_topic().trim().to_owned();
                if topic.is_empty() {
                    ui.set_status("Cần nhập topic để gửi message tùy ý".into());
                    return;
                }
                let payload = ui.get_manual_payload().to_string();
                let key = kafka::manual_key(ui.get_manual_key().as_str());
                let key_status = key
                    .as_deref()
                    .map(|key| format!("Kafka key: {key}"))
                    .unwrap_or_else(|| "Không nhập key; Kafka tự chọn partition.".into());
                (connection_from_ui(&ui, topic), payload, key, key_status)
            } else {
                let payload = {
                    let state = state.borrow();
                    let Some(template) = state.template() else {
                        ui.set_status("Chưa có target hợp lệ".into());
                        return;
                    };
                    build_message(
                        template,
                        state.action,
                        &state.fields,
                        Local::now().fixed_offset(),
                    )
                };
                let payload = match payload {
                    Ok(payload) => payload,
                    Err(error) => {
                        ui.set_status(error.into());
                        return;
                    }
                };
                ui.set_preview(payload.clone().into());
                let key = match kafka::message_key(&payload) {
                    Ok(key) => key,
                    Err(error) => {
                        ui.set_status(error.into());
                        return;
                    }
                };
                let key_status = format!("Kafka key: {key}");
                (
                    connection_from_ui(&ui, ui.get_service_topic().to_string()),
                    payload,
                    Some(key),
                    key_status,
                )
            };
            ui.set_status("Đang gửi message...".into());
            ui.set_busy(true);

            let weak = weak.clone();
            std::thread::spawn(move || {
                let result = kafka::send(config, payload, key.as_deref());
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_busy(false);
                    match result {
                        Ok(_) => show_delivery_notice(
                            &ui,
                            "Gửi message thành công",
                            format!("Đã gửi message thành công.\n{key_status}"),
                            true,
                        ),
                        Err(_) => show_delivery_notice(
                            &ui,
                            "Gửi message thất bại",
                            format!(
                                "Không gửi được message.\n{key_status}\nVui lòng kiểm tra kết nối Kafka và thông tin đăng nhập."
                            ),
                            false,
                        ),
                    }
                });
            });
        });
    }

    ui.run()?;
    Ok(())
}
