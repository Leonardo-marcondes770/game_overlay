// src/config.rs
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use crate::filtros::ModoDaltonismo;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // Tela e Filtros
    pub filtro_atual: ModoDaltonismo,
    pub intensidade: f32,
    pub brilho: f32,
    pub contraste: f32,
    pub saturacao: f32,

    // Áudio
    pub limitador_ligado: bool,
    pub limite_volume: f32,

    // Leitor de Tela (OCR/TTS)
    pub leitor_ativo: bool,
    pub tts_volume: f32,
    pub tts_velocidade: f32,
    pub tts_pitch: f32,
    pub leitura_periodica: bool,
    pub intervalo_leitura: f32,
    pub leitura_mouse: bool,
    pub largura_area_mouse: f32,
    pub altura_area_mouse: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            filtro_atual: ModoDaltonismo::Desativado,
            intensidade: 1.0,
            brilho: 0.0,
            contraste: 1.0,
            saturacao: 1.0,
            limitador_ligado: false,
            limite_volume: 0.5,
            
            leitor_ativo: false,
            tts_volume: 1.0,
            tts_velocidade: 1.0,
            tts_pitch: 1.0,
            leitura_periodica: false,
            intervalo_leitura: 3.0,
            leitura_mouse: false,
            largura_area_mouse: 350.0,
            altura_area_mouse: 80.0,
        }
    }
}

impl AppConfig {
    // Descobre onde é o AppData do usuário e cria a pasta do nosso programa
    fn obter_caminho_arquivo() -> PathBuf {
        let mut caminho = PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()));
        caminho.push("GhostOverlay");
        let _ = fs::create_dir_all(&caminho); // Cria a pasta se não existir
        caminho.push("config.json");
        caminho
    }

    pub fn carregar() -> Self {
        if let Ok(json) = fs::read_to_string(Self::obter_caminho_arquivo()) {
            if let Ok(config) = serde_json::from_str(&json) {
                return config;
            }
        }
        Self::default()
    }

    pub fn salvar(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::obter_caminho_arquivo(), json);
        }
    }
}