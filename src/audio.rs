// src/audio.rs
// Estrutura temporária vazia para não quebrar o restante do app
#[derive(Clone)]
pub struct GerenciadorAudio;

impl GerenciadorAudio {
    pub fn new() -> Self {
        Self
    }

    pub fn set_ativo(&self, _valor: bool) {}
    //pub fn is_ativo(&self) -> bool { false }
    pub fn set_limite(&self, _valor: f32) {}
    //pub fn get_limite(&self) -> f32 { 0.5 }
}