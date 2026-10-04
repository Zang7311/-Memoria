use crate::config::defaults::default_config;
use crate::engine::{api::slot_api_credentials, model_router::*};
use crate::types::{AppConfig, GetConfigResponse, ModelSlot};

fn slot(id: &str, name: &str, roles: &[&str]) -> ModelSlot {
    ModelSlot {
        id: id.into(), name: name.into(), roles: roles.iter().map(|role| role.to_string()).collect(),
        enabled: true, ..ModelSlot::default()
    }
}

fn config() -> AppConfig {
    let mut cfg = default_config();
    cfg.api_model = "old-main".into();
    cfg.cheap_model = Some("old-cheap".into());
    cfg.vision_model = Some("old-vision".into());
    cfg.api_base_url = Some("https://global.example/v1".into());
    cfg.api_key_plain = Some("global-secret".into());
    cfg.models = vec![slot("main-id", "main-text", &["main"]), slot("cheap-id", "cheap-text", &["cheap"])];
    cfg
}

#[test]
fn legacy_migration_carries_roles_urls_and_both_key_formats() {
    let mut cfg = config();
    cfg.models.clear();
    cfg.api_key_encrypted = Some("global-ciphertext".into());
    cfg.cheap_api_base_url = Some("https://cheap.example".into());
    cfg.cheap_api_key_plain = Some("cheap-secret".into());
    cfg.cheap_api_key_encrypted = Some("cheap-ciphertext".into());
    cfg.migrate_model_slots();
    assert_eq!(cfg.models.len(), 3);
    assert_eq!(cfg.models[0].name, "old-main");
    assert_eq!(cfg.models[0].roles, ["main"]);
    assert_eq!(cfg.models[0].base_url, cfg.api_base_url);
    assert_eq!(cfg.models[0].api_key_plain, cfg.api_key_plain);
    assert_eq!(cfg.models[0].api_key_encrypted, cfg.api_key_encrypted);
    assert_eq!(cfg.models[1].roles, ["cheap"]);
    assert_eq!(cfg.models[1].base_url, cfg.cheap_api_base_url);
    assert_eq!(cfg.models[1].api_key_plain, cfg.cheap_api_key_plain);
    assert_eq!(cfg.models[1].api_key_encrypted, cfg.cheap_api_key_encrypted);
    assert_eq!(cfg.models[2].roles, ["vision"]);
    assert_eq!(cfg.models[2].base_url, None);
    assert_eq!(cfg.models[2].api_key_plain, None);
    assert_eq!(cfg.models[2].api_key_encrypted, None);
    assert!(cfg.models.iter().all(|slot| slot.enabled && !slot.id.is_empty()));
    let once = serde_json::to_value(&cfg).unwrap();
    cfg.migrate_model_slots();
    assert_eq!(serde_json::to_value(&cfg).unwrap(), once);
}

#[test]
fn populated_slots_are_not_migrated_and_ignore_old_role_names() {
    let mut cfg = config();
    let before = serde_json::to_value(&cfg.models).unwrap();
    cfg.migrate_model_slots();
    assert_eq!(serde_json::to_value(&cfg.models).unwrap(), before);
    assert_eq!(main_slot(&cfg).unwrap().name, "main-text");
    assert_eq!(next_cheap_slot(&cfg).unwrap().name, "cheap-text");
    assert_eq!(vision_slot(&cfg).unwrap().name, "main-text");
    cfg.models.retain(|slot| slot.roles.contains(&"main".into()));
    assert!(next_cheap_slot(&cfg).is_none());
    assert_eq!(pick_slot_with_verdict(&cfg, "你好", false, false, None, false, None).unwrap().name, "main-text");
}

#[test]
fn legacy_empty_models_keep_existing_picker_and_ai_gate() {
    let mut cfg = config();
    cfg.models.clear();
    assert_eq!(pick_model("你好", false, false, cfg.cheap_model.as_deref(), &cfg.api_model), "old-cheap");
    assert_eq!(pick_model("帮我写代码", false, false, cfg.cheap_model.as_deref(), &cfg.api_model), "old-main");
    assert_eq!(config_ai_router_allowed(&cfg), ai_router_allowed(cfg.cheap_model.as_deref(), &cfg.api_model));
}

#[test]
fn cheap_slots_round_robin_and_wrap_without_disabled_slots() {
    let mut cfg = config();
    cfg.models = (0..3).map(|index| slot(&format!("round-robin-{index}"), &format!("cheap-{index}"), &["cheap"])).collect();
    let mut disabled = slot("disabled", "disabled", &["cheap"]);
    disabled.enabled = false;
    cfg.models.insert(1, disabled);
    let chosen: Vec<_> = (0..4).map(|_| next_cheap_slot(&cfg).unwrap().id.clone()).collect();
    assert_eq!(chosen, ["round-robin-0", "round-robin-1", "round-robin-2", "round-robin-0"]);
}

#[test]
fn role_selection_uses_first_enabled_and_respects_multiple_roles() {
    let mut cfg = config();
    cfg.models[0].enabled = false;
    cfg.models.push(slot("both", "both-text", &["main", "vision"]));
    cfg.models.push(slot("later", "later-text", &["main", "vision"]));
    assert_eq!(main_slot(&cfg).unwrap().id, "both");
    assert_eq!(vision_slot(&cfg).unwrap().id, "both");
    cfg.models.retain(|slot| !slot.roles.contains(&"main".into()));
    assert!(main_slot(&cfg).is_none());
    assert!(vision_slot(&cfg).is_none());
    assert_eq!(pick_slot_with_verdict(&cfg, "请写代码", false, false, None, false, None).map(|slot| slot.name.as_str()).unwrap_or(&cfg.api_model), "old-main");
}

#[test]
fn slot_credentials_override_independently_and_fall_back_to_global() {
    let mut cfg = config();
    cfg.cheap_api_base_url = Some("https://old-cheap.example".into());
    cfg.cheap_api_key_plain = Some("old-cheap-secret".into());
    let mut chosen = slot("own", "cheap-text", &["cheap"]);
    assert_eq!(slot_api_credentials(&cfg, Some(&chosen)).unwrap(), ("https://global.example/v1".into(), Some("global-secret".into())));
    chosen.base_url = Some(" https://own.example/v1 ".into());
    assert_eq!(slot_api_credentials(&cfg, Some(&chosen)).unwrap(), ("https://own.example/v1".into(), Some("global-secret".into())));
    chosen.api_key_plain = Some("own-secret".into());
    assert_eq!(slot_api_credentials(&cfg, Some(&chosen)).unwrap(), ("https://own.example/v1".into(), Some("own-secret".into())));
    chosen.base_url = Some("  ".into());
    assert_eq!(slot_api_credentials(&cfg, Some(&chosen)).unwrap(), ("https://global.example/v1".into(), Some("own-secret".into())));
    chosen.api_key_plain = Some(String::new());
    chosen.api_key_encrypted = Some(String::new());
    assert_eq!(slot_api_credentials(&cfg, Some(&chosen)).unwrap(), slot_api_credentials(&cfg, None).unwrap());
}

#[test]
fn own_plain_key_does_not_require_decrypting_unused_global_ciphertext() {
    let mut cfg = config();
    cfg.api_key_encrypted = Some("unused-invalid-ciphertext".into());
    cfg.models[0].api_key_plain = Some("own-secret".into());
    assert_eq!(slot_api_credentials(&cfg, main_slot(&cfg)).unwrap().1.as_deref(), Some("own-secret"));
}

#[test]
fn same_model_name_or_same_slot_id_cannot_unlock_ai_router() {
    let mut cfg = config();
    cfg.models[1].name = " main-text ".into();
    assert!(!config_ai_router_allowed(&cfg));
    cfg.models[1].name = "cheap-text".into();
    cfg.models[1].id = cfg.models[0].id.clone();
    assert!(!config_ai_router_allowed(&cfg));
    cfg.models[1].id = "separate".into();
    assert!(config_ai_router_allowed(&cfg));
}

#[test]
fn slot_ai_gate_reuses_kind_semantics_for_each_cheap_model() {
    let mut cfg = config();
    cfg.models.push(slot("vision-cheap", "gpt-4o", &["cheap"]));
    for cheap in cfg.models.iter().filter(|slot| slot.roles.contains(&"cheap".into())) {
        assert_eq!(slot_ai_router_allowed(&cfg, Some(cheap)), ai_router_allowed(Some(&cheap.name), "main-text"));
    }
    cfg.models[1].enabled = false;
    assert!(!config_ai_router_allowed(&cfg));
    cfg.models[2].enabled = false;
    assert!(!config_ai_router_allowed(&cfg));
}

#[test]
fn verdict_selects_exact_slot_even_when_models_have_the_same_name() {
    let mut cfg = config();
    cfg.models.push(slot("vision", "main-text", &["vision"]));
    let cheap = next_cheap_slot(&cfg);
    assert_eq!(pick_slot_with_verdict(&cfg, "你好", false, false, cheap, true, Some(Verdict { easy: true, needs_vision: false })).unwrap().id, "cheap-id");
    assert_eq!(pick_slot_with_verdict(&cfg, "看图", false, false, cheap, true, Some(Verdict { easy: false, needs_vision: true })).unwrap().id, "vision");
    assert_eq!(pick_slot_with_verdict(&cfg, "帮我写代码", false, false, cheap, true, None).unwrap().id, "main-id");
    assert_eq!(pick_slot_with_verdict(&cfg, "你好", false, true, cheap, true, Some(Verdict { easy: true, needs_vision: false })).unwrap().id, "main-id");
    assert_eq!(pick_slot_with_verdict(&cfg, "图片", true, false, cheap, false, None).unwrap().id, "vision");
}

#[test]
fn classifier_and_reply_reuse_one_round_robin_choice() {
    let mut cfg = config();
    cfg.models[1].base_url = Some("https://classifier.example".into());
    cfg.models[1].api_key_plain = Some("classifier-secret".into());
    cfg.models.push(slot("next", "next-text", &["cheap"]));
    let cheap = next_cheap_slot(&cfg);
    let classifier_credentials = slot_api_credentials(&cfg, cheap).unwrap();
    let reply_slot = pick_slot_with_verdict(&cfg, "你好", false, false, cheap, true, Some(Verdict { easy: true, needs_vision: false }));
    assert_eq!(reply_slot.unwrap().id, cheap.unwrap().id);
    assert_eq!(slot_api_credentials(&cfg, reply_slot).unwrap(), classifier_credentials);
    assert_eq!(next_cheap_slot(&cfg).unwrap().id, "next");
}

#[test]
fn public_projection_recursively_removes_all_api_key_fields_and_values() {
    let mut cfg = config();
    cfg.api_key_encrypted = Some("global-ciphertext".into());
    cfg.cheap_api_key_plain = Some("cheap-secret".into());
    cfg.cheap_api_key_encrypted = Some("cheap-ciphertext".into());
    cfg.models[0].api_key_plain = Some("slot-secret".into());
    cfg.models[0].api_key_encrypted = Some("slot-ciphertext".into());
    fn assert_no_key_fields(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => for (name, value) in object {
                assert!(!name.contains("api_key_"), "secret field {name}");
                assert_no_key_fields(value);
            },
            serde_json::Value::Array(array) => for value in array { assert_no_key_fields(value); },
            _ => {},
        }
    }
    let public = cfg.to_public().unwrap();
    assert_no_key_fields(&public);
    assert_eq!(public["models"][0]["has_api_key"], true);
    assert_eq!(public["models"][1]["has_api_key"], false);
    let text = serde_json::to_string(&GetConfigResponse::from_config(&cfg).unwrap()).unwrap();
    for secret in ["global-secret", "global-ciphertext", "cheap-secret", "cheap-ciphertext", "slot-secret", "slot-ciphertext"] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn old_config_and_slot_defaults_deserialize_without_new_fields() {
    let mut value = serde_json::to_value(default_config()).unwrap();
    value.as_object_mut().unwrap().remove("models");
    let cfg: AppConfig = serde_json::from_value(value).unwrap();
    assert!(cfg.models.is_empty());
    let slot: ModelSlot = serde_json::from_value(serde_json::json!({"id":"id", "name":"model"})).unwrap();
    assert!(slot.enabled);
    assert!(slot.roles.is_empty());
    assert_eq!(slot.api_key_plain, None);
}
