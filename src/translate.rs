//! «Перевести на русский» for project descriptions.
//!
//! Google Translate's public web endpoint (`client=gtx`, the one browser
//! extensions use) — no key, no account. Text goes in paragraph-aligned
//! chunks of ≤ 4 500 characters (the endpoint's GET limit), in parallel,
//! and is joined back in order. Only public description text is sent.

use anyhow::{anyhow, Context, Result};

const CHUNK: usize = 4_500;

/// Splits `text` into chunks of at most `max` characters, cutting at
/// paragraph breaks, then line breaks, then spaces — never inside a word
/// when it can be helped. Joining the chunks gives back the text.
pub fn chunks(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.chars().count() > max {
        let limit = rest.char_indices().nth(max).map(|(i, _)| i).unwrap_or(rest.len());
        let window = &rest[..limit];
        let cut = window
            .rfind("\n\n")
            .map(|i| i + 2)
            .or_else(|| window.rfind('\n').map(|i| i + 1))
            .or_else(|| window.rfind(' ').map(|i| i + 1))
            .filter(|&i| i > 0)
            .unwrap_or(limit);
        out.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

/// The translated text out of the endpoint's answer:
/// `[[["перевод","original",…],…],…]` → the first fields, concatenated.
pub fn parse_answer(json: &serde_json::Value) -> Option<String> {
    let parts = json.get(0)?.as_array()?;
    let mut out = String::new();
    for part in parts {
        if let Some(s) = part.get(0).and_then(|v| v.as_str()) {
            out.push_str(s);
        }
    }
    (!out.is_empty()).then_some(out)
}

async fn translate_chunk(client: &reqwest::Client, text: &str) -> Result<String> {
    let resp = client
        .post("https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=ru&dt=t")
        .form(&[("q", text)])
        .send()
        .await
        .context("Переводчик не отвечает")?
        .error_for_status()
        .context("Переводчик отказал — попробуй позже")?;
    let json: serde_json::Value = resp.json().await.context("Ответ переводчика не читается")?;
    parse_answer(&json).ok_or_else(|| anyhow!("Переводчик вернул пустой ответ"))
}

/// Russian translation of `text` (any source language).
pub async fn to_russian(client: &reqwest::Client, text: &str) -> Result<String> {
    let parts = chunks(text, CHUNK);
    let futures = parts.iter().map(|p| translate_chunk(client, p));
    let done = futures_util::future::join_all(futures).await;
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    for (part, res) in parts.iter().zip(done) {
        // A blank chunk (only line breaks) comes back empty: keep it as is.
        match res {
            Ok(t) => out.push_str(&t),
            Err(_) if part.trim().is_empty() => out.push_str(part),
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_cut_at_paragraphs_and_rejoin() {
        let text = "Первый абзац текста.\n\nВторой абзац, длиннее первого.\n\nТретий.";
        let parts = chunks(text, 30);
        assert!(parts.iter().all(|p| p.chars().count() <= 30));
        assert_eq!(parts.concat(), text);
        assert!(parts[0].ends_with("\n\n"));
        assert_eq!(chunks("коротко", 30), vec!["коротко"]);
        let long_word = "a".repeat(70);
        assert_eq!(chunks(&long_word, 30).concat(), long_word);
    }

    #[test]
    fn answer_is_joined_in_order() {
        let json: serde_json::Value =
            serde_json::from_str(r#"[[["Привет. ","Hello. ",null],["Мир","World",null]],null,"en"]"#).unwrap();
        assert_eq!(parse_answer(&json).as_deref(), Some("Привет. Мир"));
        assert_eq!(parse_answer(&serde_json::json!([])), None);
    }

    /// `cargo test live_translate -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_translate() {
        crate::net::rt().block_on(async {
            let ru = to_russian(&crate::net::client(), "Better Minecraft adds hundreds of new dungeons.\n\nFully optimized.")
                .await
                .unwrap();
            println!("{ru}");
            assert!(ru.chars().any(|c| ('а'..='я').contains(&c)));
        });
    }
}
