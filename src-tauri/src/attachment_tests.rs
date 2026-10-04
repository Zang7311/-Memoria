use crate::attachments::tests::{image, text};
use crate::config::defaults::default_config;
use crate::engine::model_router::*;
use crate::types::{AppConfig, ModelSlot};

fn config() -> AppConfig {
    let mut cfg = default_config();
    cfg.api_model = "legacy-main-text".into();
    cfg.cheap_model = Some("legacy-cheap-text".into());
    cfg.vision_model = Some("legacy-vision".into());
    cfg.models = [
        ("main", "main-text"),
        ("cheap", "cheap-text"),
        ("vision", "vision-model"),
    ]
    .into_iter()
    .map(|(role, name)| ModelSlot {
        id: role.into(),
        name: name.into(),
        roles: vec![role.into()],
        enabled: true,
        ..ModelSlot::default()
    })
    .collect();
    cfg
}

#[test]
fn 图片附件始终走视觉槽位且判断失败也不降级() {
    let cfg = config();
    let cheap = next_cheap_slot(&cfg);
    for ai_enabled in [true, false] {
        for verdict in [
            None,
            Some(Verdict {
                easy: true,
                needs_vision: false,
            }),
            Some(Verdict {
                easy: false,
                needs_vision: true,
            }),
        ] {
            assert_eq!(
                pick_slot_with_attachments(&cfg, "你好", &[image()], cheap, ai_enabled, verdict)
                    .unwrap()
                    .id,
                "vision"
            );
            assert!(
                attachment_verdict("你好", &[image()], verdict)
                    .unwrap()
                    .needs_vision
            );
        }
    }
}

#[test]
fn 没有视觉槽位时图片回退主力而不是便宜槽位() {
    let mut cfg = config();
    cfg.models[2].enabled = false;
    assert_eq!(
        pick_slot_with_attachments(&cfg, "你好", &[image()], next_cheap_slot(&cfg), false, None)
            .unwrap()
            .id,
        "main"
    );
    cfg.models.retain(|slot| slot.id != "main");
    assert!(pick_slot_with_attachments(
        &cfg,
        "你好",
        &[image()],
        next_cheap_slot(&cfg),
        false,
        None
    )
    .is_none());
}

#[test]
fn 文本附件闲聊走便宜复杂任务强制主力() {
    let cfg = config();
    let cheap = next_cheap_slot(&cfg);
    for ai_enabled in [true, false] {
        assert_eq!(
            pick_slot_with_attachments(
                &cfg,
                "你好",
                &[text("note.txt", "今天好开心")],
                cheap,
                ai_enabled,
                None
            )
            .unwrap()
            .id,
            "cheap"
        );
        for (input, attachment) in [
            ("帮我总结", text("note.txt", "今天好开心")),
            ("你好", text("note.txt", &"内容".repeat(251))),
            ("你好", text("note.txt", "帮我翻译这份资料")),
            (&"消息".repeat(30), text("note.txt", "短文本")),
        ] {
            let verdict = Some(Verdict {
                easy: true,
                needs_vision: false,
            });
            assert_eq!(
                pick_slot_with_attachments(&cfg, input, &[attachment], cheap, ai_enabled, verdict)
                    .unwrap()
                    .id,
                "main"
            );
        }
    }
    let attachments = [
        text("a.txt", &"字".repeat(300)),
        text("b.txt", &"字".repeat(300)),
    ];
    assert_eq!(
        pick_slot_with_attachments(&cfg, "你好", &attachments, cheap, false, None)
            .unwrap()
            .id,
        "main"
    );
}

#[test]
fn 图文混合附件视觉优先于文本复杂度() {
    let cfg = config();
    let attachments = [text("a.rs", &"执行".repeat(1000)), image()];
    assert_eq!(
        pick_slot_with_attachments(
            &cfg,
            "帮我分析",
            &attachments,
            next_cheap_slot(&cfg),
            true,
            None
        )
        .unwrap()
        .id,
        "vision"
    );
}

#[test]
fn 无附件新旧路由逐条一致() {
    let mut cfg = config();
    let long = "啊".repeat(70);
    for ai_enabled in [false, true] {
        cfg.ai_router = ai_enabled;
        for input in [
            "",
            " ",
            "你好",
            "晚安",
            "帮我打开QQ",
            "总结资料",
            long.as_str(),
        ] {
            for verdict in [
                None,
                Some(Verdict {
                    easy: true,
                    needs_vision: false,
                }),
                Some(Verdict {
                    easy: false,
                    needs_vision: false,
                }),
                Some(Verdict {
                    easy: true,
                    needs_vision: true,
                }),
            ] {
                let cheap = next_cheap_slot(&cfg);
                assert_eq!(
                    pick_slot_with_attachments(&cfg, input, &[], cheap, ai_enabled, verdict)
                        .map(|slot| &slot.id),
                    pick_slot_with_verdict(&cfg, input, false, false, cheap, ai_enabled, verdict)
                        .map(|slot| &slot.id)
                );
                assert_eq!(
                    pick_model_with_attachments(&cfg, input, &[], verdict),
                    pick_model_with_verdict(
                        input,
                        false,
                        false,
                        cfg.cheap_model.as_deref(),
                        &cfg.api_model,
                        cfg.vision_model.as_deref(),
                        ai_enabled,
                        verdict
                    )
                );
            }
        }
    }
}

#[test]
fn 旧配置附件路由保持视觉优先与任务保底() {
    let mut cfg = config();
    cfg.models.clear();
    assert_eq!(
        pick_model_with_attachments(&cfg, "你好", &[image()], None),
        "legacy-vision"
    );
    cfg.cheap_model = None;
    assert_eq!(
        pick_model_with_attachments(&cfg, "你好", &[image()], None),
        "legacy-vision"
    );
    cfg.vision_model = None;
    assert_eq!(
        pick_model_with_attachments(&cfg, "你好", &[image()], None),
        "legacy-main-text"
    );
    cfg.cheap_model = Some("legacy-cheap-text".into());
    assert_eq!(
        pick_model_with_attachments(&cfg, "你好", &[text("a.txt", "开心")], None),
        "legacy-cheap-text"
    );
    assert_eq!(
        pick_model_with_attachments(
            &cfg,
            "帮我总结",
            &[text("a.txt", "开心")],
            Some(Verdict {
                easy: true,
                needs_vision: false
            })
        ),
        "legacy-main-text"
    );
}

#[tokio::test]
async fn 在线判断收到附件元信息且失败或误判也不阻断视觉路由() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (index, (status, answer)) in [(503, ""), (200, "无法判断"), (200, "easy text")]
        .into_iter()
        .enumerate()
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            let body = loop {
                let length = socket.read(&mut buffer).await.unwrap();
                assert!(length > 0);
                request.extend_from_slice(&buffer[..length]);
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse().ok())
                        })
                        .unwrap();
                    if request.len() >= header_end + 4 + content_length {
                        break serde_json::from_slice::<serde_json::Value>(
                            &request[header_end + 4..header_end + 4 + content_length],
                        )
                        .unwrap();
                    }
                }
            };
            let response =
                serde_json::json!({"choices":[{"message":{"content":answer}}]}).to_string();
            let headers = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len());
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
            body
        });
        let attachments = [
            image(),
            text(&format!("classifier-{index}.txt"), "secret附件内容"),
        ];
        let context = crate::attachments::classification_input("你好", &attachments);
        let verdict = classify_with_ai(
            &reqwest::Client::new(),
            &format!("http://{address}"),
            "test-key",
            "cheap-text",
            &context,
        )
        .await;
        if index < 2 {
            assert!(verdict.is_none());
        } else {
            assert_eq!(
                verdict,
                Some(Verdict {
                    easy: true,
                    needs_vision: false
                })
            );
        }
        let request = server.await.unwrap();
        let sent = request["messages"][1]["content"].as_str().unwrap();
        assert!(sent.contains("photo.JPG"));
        assert!(sent.contains(&format!("classifier-{index}.txt")));
        assert!(!sent.contains("secret"));
        assert!(!sent.contains("AQID"));
        let cfg = config();
        assert_eq!(
            pick_slot_with_attachments(
                &cfg,
                "你好",
                &attachments,
                next_cheap_slot(&cfg),
                true,
                verdict
            )
            .unwrap()
            .id,
            "vision"
        );
    }
}
