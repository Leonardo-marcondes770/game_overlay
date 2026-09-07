// src/filtros.rs
use windows_sys::Win32::UI::Magnification::{MagSetFullscreenColorEffect, MAGCOLOREFFECT};
use serde::{Serialize, Deserialize};

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum ModoDaltonismo {
    Desativado,
    Protanopia,
    Deuteranopia,
    Tritanopia,
    Acromatopsia,
}

// Função auxiliar para multiplicar duas matrizes 5x5
fn multiplicar_matrizes(a: [f32; 25], b: [f32; 25]) -> [f32; 25] {
    let mut resultado = [0.0; 25];
    for r in 0..5 {
        for c in 0..5 {
            let mut soma = 0.0;
            for k in 0..5 {
                soma += a[r * 5 + k] * b[k * 5 + c];
            }
            resultado[r * 5 + c] = soma;
        }
    }
    resultado
}

// NOVA FUNÇÃO: Interpolação linear entre duas matrizes (Garante a intensidade do filtro)
fn interpolar_matrizes(base: [f32; 25], alvo: [f32; 25], intensidade: f32) -> [f32; 25] {
    let mut resultado = base;
    for i in 0..25 {
        resultado[i] = base[i] * (1.0 - intensidade) + alvo[i] * intensidade;
    }
    resultado
}

// Adicionado o parâmetro 'intensidade'
pub fn atualizar_filtros(modo: ModoDaltonismo, brilho: f32, contraste: f32, saturacao: f32, intensidade: f32) {
    let m_identidade = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    // Matriz alvo baseada no modo selecionado
    let m_alvo = match modo {
        ModoDaltonismo::Desativado => m_identidade,
        ModoDaltonismo::Protanopia => [
            0.567, 0.558, 0.000, 0.000, 0.000,
            0.433, 0.442, 0.242, 0.000, 0.000,
            0.000, 0.000, 0.758, 0.000, 0.000,
            0.000, 0.000, 0.000, 1.000, 0.000,
            0.000, 0.000, 0.000, 0.000, 1.000,
        ],
        ModoDaltonismo::Deuteranopia => [
            0.625, 0.700, 0.000, 0.000, 0.000,
            0.375, 0.300, 0.300, 0.000, 0.000,
            0.000, 0.000, 0.700, 0.000, 0.000,
            0.000, 0.000, 0.000, 1.000, 0.000,
            0.000, 0.000, 0.000, 0.000, 1.000,
        ],
        ModoDaltonismo::Tritanopia => [
            0.950, 0.000, 0.000, 0.000, 0.000,
            0.050, 0.433, 0.475, 0.000, 0.000,
            0.000, 0.567, 0.525, 0.000, 0.000,
            0.000, 0.000, 0.000, 1.000, 0.000,
            0.000, 0.000, 0.000, 0.000, 1.000,
        ],
        ModoDaltonismo::Acromatopsia => [
            0.299, 0.299, 0.299, 0.000, 0.000,
            0.587, 0.587, 0.587, 0.000, 0.000,
            0.114, 0.114, 0.114, 0.000, 0.000,
            0.000, 0.000, 0.000, 1.000, 0.000,
            0.000, 0.000, 0.000, 0.000, 1.000,
        ],
    };

    // Aplica a intensidade: mistura a identidade com a matriz do daltonismo escolhido
    let m_daltonismo = interpolar_matrizes(m_identidade, m_alvo, intensidade);

    // 2. MATRIZ DE SATURAÇÃO
    let lr = 0.299;
    let lg = 0.587;
    let lb = 0.114;
    let m_saturacao = [
        saturacao + (1.0 - saturacao) * lr, (1.0 - saturacao) * lr,     (1.0 - saturacao) * lr,     0.0, 0.0,
        (1.0 - saturacao) * lg,     saturacao + (1.0 - saturacao) * lg, (1.0 - saturacao) * lg,     0.0, 0.0,
        (1.0 - saturacao) * lb,     (1.0 - saturacao) * lb,     saturacao + (1.0 - saturacao) * lb, 0.0, 0.0,
        0.0,                        0.0,                        0.0,                        1.0, 0.0,
        0.0,                        0.0,                        0.0,                        0.0, 1.0,
    ];

    // 3. MATRIZ DE CONTRASTE
    let t = 0.5 * (1.0 - contraste);
    let m_contraste = [
        contraste, 0.0,       0.0,       0.0, 0.0,
        0.0,       contraste, 0.0,       0.0, 0.0,
        0.0,       0.0,       contraste, 0.0, 0.0,
        0.0,       0.0,       0.0,       1.0, 0.0,
        t,         t,         t,         0.0, 1.0,
    ];

    // 4. MATRIZ DE BRILHO
    let m_brilho = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 0.0,
        brilho, brilho, brilho, 0.0, 1.0,
    ];

    // Combinando todas as operações matemáticas
    let mut combinada = multiplicar_matrizes(m_daltonismo, m_saturacao);
    combinada = multiplicar_matrizes(combinada, m_contraste);
    combinada = multiplicar_matrizes(combinada, m_brilho);

    unsafe {
        let efeito = MAGCOLOREFFECT { transform: combinada };
        MagSetFullscreenColorEffect(&efeito);
    }
}