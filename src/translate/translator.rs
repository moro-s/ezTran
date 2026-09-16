use super::engine::EngineConfig;
use serde::{Deserialize, Serialize};

/// 翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    /// 译文
    pub text: String,
    /// 源语言
    pub from: String,
    /// 目标语言
    pub to: String,
    /// 使用的引擎名
    pub engine: String,
}

/// 翻译器：根据引擎配置执行翻译
pub struct Translator;

impl Translator {
    /// 执行翻译（同步阻塞）
    pub fn translate(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        match engine.kind {
            super::engine::EngineKind::Youdao => Self::translate_youdao(engine, text, from, to),
            super::engine::EngineKind::Baidu => Self::translate_baidu(engine, text, from, to),
            super::engine::EngineKind::DeepL => Self::translate_deepl(engine, text, from, to),
            super::engine::EngineKind::Custom => Self::translate_custom(engine, text, from, to),
        }
    }

    fn translate_youdao(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() || engine.api_secret.is_empty() {
            anyhow::bail!("有道翻译需要配置 API Key 和 API Secret");
        }

        // 有道智云 API: https://openapi.youdao.com/api
        // 需要 appKey, appSecret, q, from, to, salt, sign
        // TODO: 实现完整签名逻辑（sign = sha256(appKey + q + salt + appSecret)）
        let url = format!(
            "https://openapi.youdao.com/api?q={}&from={}&to={}&appKey={}",
            url_encode(text),
            from,
            to,
            engine.api_key
        );

        let resp = http_get_json(&url)?;
        let translated = resp
            .get("translation")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_baidu(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() || engine.api_secret.is_empty() {
            anyhow::bail!("百度翻译需要配置 App ID 和 App Secret");
        }

        // 百度翻译 API: https://fanyi-api.baidu.com/api/trans/vip/translate
        // 需要 q, from, to, appid, salt, sign
        // TODO: 实现完整签名逻辑（sign = md5(appid + q + salt + key)）
        let url = format!(
            "https://fanyi-api.baidu.com/api/trans/vip/translate?q={}&from={}&to={}",
            url_encode(text),
            from,
            to
        );

        let resp = http_get_json(&url)?;
        let translated = resp
            .get("trans_result")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("dst"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_deepl(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.api_key.is_empty() {
            anyhow::bail!("DeepL 需要配置 API Key");
        }

        // DeepL API (free): https://api-free.deepl.com/v2/translate
        // DeepL API (pro): https://api.deepl.com/v2/translate
        let endpoint = if engine.endpoint.is_empty() {
            "https://api-free.deepl.com/v2/translate".to_string()
        } else {
            engine.endpoint.clone()
        };

        let url = format!(
            "{}?text={}&target_lang={}",
            endpoint,
            url_encode(text),
            to.to_uppercase()
        );

        let resp = http_get_json_with_auth(&url, &engine.api_key, "DeepL-Auth-Key")?;
        let translated = resp
            .get("translations")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }

    fn translate_custom(
        engine: &EngineConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<TranslateResult> {
        if engine.endpoint.is_empty() {
            anyhow::bail!("自定义引擎需要配置 Endpoint URL");
        }

        let url = format!(
            "{}/translate?q={}&from={}&to={}",
            engine.endpoint.trim_end_matches('/'),
            url_encode(text),
            from,
            to
        );

        let resp = http_get_json_with_auth_opt(&url, &engine.api_key)?;
        let translated = resp
            .get("text")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TranslateResult {
            text: translated,
            from: from.into(),
            to: to.into(),
            engine: engine.name.clone(),
        })
    }
}

// ── HTTP 工具函数（基于 minreq）──

fn http_get_json(url: &str) -> anyhow::Result<serde_json::Value> {
    let response = minreq::get(url).send()?;
    if response.status_code >= 400 {
        anyhow::bail!("HTTP {}: {}", response.status_code, url);
    }
    let value: serde_json::Value = serde_json::from_str(response.as_str()?)?;
    Ok(value)
}

fn http_get_json_with_auth(
    url: &str,
    api_key: &str,
    auth_prefix: &str,
) -> anyhow::Result<serde_json::Value> {
    let response = minreq::get(url)
        .with_header("Authorization", format!("{} {}", auth_prefix, api_key))
        .send()?;
    if response.status_code >= 400 {
        anyhow::bail!("HTTP {}: {}", response.status_code, url);
    }
    let value: serde_json::Value = serde_json::from_str(response.as_str()?)?;
    Ok(value)
}

fn http_get_json_with_auth_opt(url: &str, api_key: &str) -> anyhow::Result<serde_json::Value> {
    let mut req = minreq::get(url);
    if !api_key.is_empty() {
        req = req.with_header("Authorization", format!("Bearer {}", api_key));
    }
    let response = req.send()?;
    if response.status_code >= 400 {
        anyhow::bail!("HTTP {}: {}", response.status_code, url);
    }
    let value: serde_json::Value = serde_json::from_str(response.as_str()?)?;
    Ok(value)
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u8)
            }
        })
        .collect()
}
