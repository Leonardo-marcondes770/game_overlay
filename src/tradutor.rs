// src/tradutor.rs
use reqwest::blocking::Client;
use serde_json::Value;
use urlencoding::encode;

pub fn traduzir_texto(texto: &str, idioma_origem: &str, idioma_destino: &str) -> Option<String> {
    if texto.trim().is_empty() {
        return None;
    }

    // Monta a URL da API gratuita do Google Translate
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
        idioma_origem,
        idioma_destino,
        encode(texto)
    );

    // Envia a requisição bloqueante (seguro fazer isso pois estará na thread de background do leitor)
    let client = Client::new();
    let resposta = client.get(&url).send().ok()?.json::<Value>().ok()?;

    // O Google retorna um JSON complexo cheio de arrays aninhados.
    // O formato é algo como: [ [ ["Olá mundo", "Hello world", null, null, 1] ], null, "en", ... ]
    let mut traducao_completa = String::new();

    if let Some(linhas) = resposta.get(0).and_then(|v| v.as_array()) {
        for linha in linhas {
            if let Some(pedaco) = linha.get(0).and_then(|v| v.as_str()) {
                traducao_completa.push_str(pedaco);
                traducao_completa.push(' ');
            }
        }
    }

    let traducao_limpa = traducao_completa.trim().to_string();
    
    if traducao_limpa.is_empty() {
        None
    } else {
        Some(traducao_limpa)
    }
}