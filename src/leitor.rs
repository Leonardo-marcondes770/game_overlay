// src/leitor.rs
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use egui::{Align2, Area, Color32, Context, CursorIcon, FontId, Id, Order, Pos2, Rect, Sense, Stroke, Ui};
use image::DynamicImage;
use tts::Tts;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};
use xcap::Monitor;

use windows::core::h;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

const TEMPO_MOUSE_PARADO_MS: u64 = 500;
const TOLERANCIA_MOVIMENTO_MOUSE: i32 = 5;

pub enum MensagemThread {
    ProcessarRect(Rect),
    ForcarFala(String),
    AtualizarTtsConfig { volume: f32, velocidade: f32, pitch: f32 },
}

// 👇 NOVA FUNÇÃO DE INTELIGÊNCIA: Mede a similaridade entre textos ignorando pontuação e ruídos (Levenshtein)
fn sao_similares(a: &str, b: &str) -> bool {
    if a == b { return true; }
    
    // Filtra apenas letras e números, ignorando espaços e cursores piscando
    let clean_a: String = a.chars().filter(|c| c.is_alphanumeric()).collect();
    let clean_b: String = b.chars().filter(|c| c.is_alphanumeric()).collect();
    
    if clean_a == clean_b { return true; }
    
    let a_chars: Vec<char> = clean_a.chars().collect();
    let b_chars: Vec<char> = clean_b.chars().collect();
    let len_a = a_chars.len();
    let len_b = b_chars.len();
    
    if len_a == 0 || len_b == 0 { return false; }
    
    // Se a diferença de tamanho for bizarra (> 25%), com certeza mudou o diálogo
    let diff = (len_a as i32 - len_b as i32).abs();
    if diff > (len_a.max(len_b) as i32 / 4).max(3) {
        return false;
    }

    let mut d = vec![vec![0; len_b + 1]; len_a + 1];
    for i in 0..=len_a { d[i][0] = i; }
    for j in 0..=len_b { d[0][j] = j; }

    for i in 1..=len_a {
        for j in 1..=len_b {
            let cost = if a_chars[i-1] == b_chars[j-1] { 0 } else { 1 };
            d[i][j] = (d[i-1][j] + 1).min(d[i][j-1] + 1).min(d[i-1][j-1] + cost);
        }
    }
    
    let max_len = len_a.max(len_b) as f32;
    let dist = d[len_a][len_b] as f32;
    let similaridade = 1.0 - (dist / max_len);
    
    similaridade > 0.85 // 85% de igualdade é considerado o mesmo texto
}

pub struct GerenciadorLeitor {
    pub ativo: bool,
    pub texto_extraido: String,
    pub modo_debug: bool,
    
    pub modo_selecao: bool,
    pub regiao_selecionada: Option<Rect>,
    pub processar_pendente: bool,
    ponto_inicial: Option<Pos2>,

    pub leitura_periodica: bool,
    pub intervalo_leitura: f32,
    ultimo_disparo_periodico: Instant,

    pub leitura_mouse: bool,
    pub largura_area_mouse: f32, 
    pub altura_area_mouse: f32,  
    posicao_anterior_mouse: (i32, i32),
    tempo_mouse_parado: Instant,
    ja_leu_mouse_nesta_posicao: bool, // 👇 NOVO: Anti-spam pro CPU

    pub tts_volume: f32,
    pub tts_velocidade: f32,
    pub tts_pitch: f32,

    ocr_ocupado: Arc<AtomicBool>,
    tx: Sender<MensagemThread>,
}

impl GerenciadorLeitor {
    pub fn new(ctx: Context) -> Self {
        let (tx, rx) = channel::<MensagemThread>();
        let ocr_ocupado = Arc::new(AtomicBool::new(false));
        
        let ocr_ocupado_thread = Arc::clone(&ocr_ocupado);
        let ctx_thread = ctx.clone();

        thread::spawn(move || {
            let mut tts = Tts::default().ok();
            let mut ultimo_texto_falado = String::new();

            while let Ok(msg) = rx.recv() {
                match msg {
                    MensagemThread::AtualizarTtsConfig { volume, velocidade, pitch } => {
                        if let Some(ref mut motor) = tts {
                            let _ = motor.set_volume(volume);
                            let _ = motor.set_rate(velocidade);
                            let _ = motor.set_pitch(pitch);
                        }
                    }
                    MensagemThread::ForcarFala(texto) => {
                        if let Some(ref mut motor) = tts {
                            let _ = motor.speak(&texto, true);
                        }
                    }
                    MensagemThread::ProcessarRect(rect) => {
                        if let Some(texto) = Self::capturar_e_ocr(rect, &ctx_thread) {
                            let texto_limpo = texto.trim().to_string();
                            
                            if !texto_limpo.is_empty() {
                                // 👇 A MÁGICA ACONTECE AQUI: Só manda falar se o texto for relamente diferente
                                if !sao_similares(&texto_limpo, &ultimo_texto_falado) {
                                    if let Some(ref mut motor) = tts {
                                        let _ = motor.speak(&texto_limpo, true);
                                    }
                                    ultimo_texto_falado = texto_limpo.clone();
                                    ctx_thread.data_mut(|d| d.insert_temp(Id::new("ocr_texto_key"), texto_limpo.clone()));
                                }
                            }
                        }
                        ocr_ocupado_thread.store(false, Ordering::Relaxed);
                        ctx_thread.request_repaint();
                    }
                }
            }
        });

        Self {
            ativo: false,
            texto_extraido: String::from("Nenhum texto processado ainda."),
            modo_debug: false,
            
            modo_selecao: false,
            regiao_selecionada: None,
            processar_pendente: false,
            ponto_inicial: None,

            leitura_periodica: false,
            intervalo_leitura: 3.0,
            ultimo_disparo_periodico: Instant::now(),

            leitura_mouse: false,
            largura_area_mouse: 350.0,
            altura_area_mouse: 80.0,
            posicao_anterior_mouse: (0, 0),
            tempo_mouse_parado: Instant::now(),
            ja_leu_mouse_nesta_posicao: false, 

            tts_volume: 1.0,
            tts_velocidade: 1.0,
            tts_pitch: 1.0,

            ocr_ocupado,
            tx,
        }
    }

    pub fn atualizar_tick(&mut self, ctx: &Context) {
        if self.modo_debug || (self.ativo && (self.leitura_periodica || self.leitura_mouse)) {
            ctx.request_repaint();
        }

        if !self.ativo { return; }

        let ocr_livre = !self.ocr_ocupado.load(Ordering::Relaxed);

        if self.processar_pendente && self.regiao_selecionada.is_some() && ocr_livre {
            self.disparar_ocr(self.regiao_selecionada.unwrap());
            self.processar_pendente = false;
        }

        if self.leitura_periodica && self.regiao_selecionada.is_some() && ocr_livre {
            if self.ultimo_disparo_periodico.elapsed().as_secs_f32() >= self.intervalo_leitura {
                self.ultimo_disparo_periodico = Instant::now();
                self.disparar_ocr(self.regiao_selecionada.unwrap());
            }
        }

        if self.leitura_mouse && !self.modo_selecao {
            let mut pt = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut pt); } 

            let mov_x = (pt.x - self.posicao_anterior_mouse.0).abs();
            let mov_y = (pt.y - self.posicao_anterior_mouse.1).abs();

            if mov_x > TOLERANCIA_MOVIMENTO_MOUSE || mov_y > TOLERANCIA_MOVIMENTO_MOUSE {
                self.posicao_anterior_mouse = (pt.x, pt.y);
                self.tempo_mouse_parado = Instant::now();
                self.ja_leu_mouse_nesta_posicao = false; // Mouse moveu, reseta flag
            } else if !self.ja_leu_mouse_nesta_posicao && self.tempo_mouse_parado.elapsed() >= Duration::from_millis(TEMPO_MOUSE_PARADO_MS) && ocr_livre {
                let dpi = ctx.pixels_per_point();

                let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };

                let mouse_x = (pt.x - offset_x) as f32 / dpi;
                let mouse_y = (pt.y - offset_y) as f32 / dpi;

                let meio_x = self.largura_area_mouse / 2.0;
                let meio_y = self.altura_area_mouse / 2.0;
                
                let rect_mouse = Rect::from_min_max(
                    Pos2::new(mouse_x - meio_x, mouse_y - meio_y),
                    Pos2::new(mouse_x + meio_x, mouse_y + meio_y),
                );
                
                self.disparar_ocr(rect_mouse);
                self.ja_leu_mouse_nesta_posicao = true; // Evita loop no mouse parado!
            }
        }

        ctx.data(|d| {
            if let Some(texto) = d.get_temp::<String>(Id::new("ocr_texto_key")) {
                self.texto_extraido = texto;
            }
        });
    }

    fn disparar_ocr(&mut self, rect: Rect) {
        self.ocr_ocupado.store(true, Ordering::Relaxed);
        let _ = self.tx.send(MensagemThread::ProcessarRect(rect));
    }

    pub fn atualizar_config_tts(&self) {
        let _ = self.tx.send(MensagemThread::AtualizarTtsConfig {
            volume: self.tts_volume,
            velocidade: self.tts_velocidade,
            pitch: self.tts_pitch,
        });
    }

    pub fn exibir_ui(&mut self, ui: &mut Ui, _ctx: &Context) -> bool {
        ui.heading("On-Screen Reader (OCR)");
        ui.separator();
        
        ui.checkbox(&mut self.ativo, "Ligar Sistema de Leitura de Tela");
        let mut disparar_selecao = false;

        ui.add_enabled_ui(self.ativo, |ui| {
            ui.group(|ui| {
                ui.label("📌 Leitura de Área Fixa");
                ui.horizontal(|ui| {
                    if ui.button("Selecionar Nova Área").clicked() {
                        self.modo_selecao = true;
                        self.ponto_inicial = None;
                        disparar_selecao = true;
                    }
                    if ui.button("Limpar Área").clicked() {
                        self.regiao_selecionada = None;
                        self.texto_extraido = String::from("Nenhum texto.");
                    }
                });

                if let Some(rect) = self.regiao_selecionada {
                    ui.colored_label(Color32::LIGHT_GREEN, format!("Salvo: {}x{}", rect.width() as i32, rect.height() as i32));
                    ui.checkbox(&mut self.leitura_periodica, "Ler essa área automaticamente");
                    if self.leitura_periodica {
                        ui.add(egui::Slider::new(&mut self.intervalo_leitura, 1.0..=15.0)
                            .text("Segundos entre leituras"));
                    }
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label("🖱️ Leitura no Cursor (Hover)");
                ui.checkbox(&mut self.leitura_mouse, "Ler texto ao parar o mouse (0.5s)");
                if self.leitura_mouse {
                    ui.add(egui::Slider::new(&mut self.largura_area_mouse, 50.0..=1000.0).text("Largura da Faixa"));
                    ui.add(egui::Slider::new(&mut self.altura_area_mouse, 30.0..=500.0).text("Altura da Faixa"));
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label("🔊 Configurações de Voz");
                let mut mudou_voz = false;

                mudou_voz |= ui.add(egui::Slider::new(&mut self.tts_volume, 0.0..=1.0)
                    .text("Volume")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed();

                mudou_voz |= ui.add(egui::Slider::new(&mut self.tts_velocidade, 0.5..=2.0)
                    .text("Velocidade da Fala")
                    .custom_formatter(|v, _| format!("{:.1}x", v))).changed();

                mudou_voz |= ui.add(egui::Slider::new(&mut self.tts_pitch, 0.5..=2.0)
                    .text("Tom / Frequência")
                    .custom_formatter(|v, _| format!("{:.1}x", v))).changed();

                if mudou_voz {
                    self.atualizar_config_tts();
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(format!("Último texto lido:\n{}", self.texto_extraido));
                ui.add_space(6.0);
                if ui.button("🗣️ Ler Texto Atual Novamente").clicked() {
                    let texto = self.texto_extraido.clone();
                    if !texto.is_empty() && texto != "Nenhum texto processado ainda." {
                        let _ = self.tx.send(MensagemThread::ForcarFala(texto));
                    }
                }
            });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.checkbox(&mut self.modo_debug, "🛠️ Ativar Visualizador de Debug");

        disparar_selecao
    }

    pub fn renderizar_overlay_debug(&self, ctx: &Context) {
        if !self.modo_debug { return; }

        let cor_amarela = Color32::YELLOW;
        let cor_ciano = Color32::from_rgb(0, 255, 255);

        egui::Window::new("🛠️ Status Interno")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let ocupado = self.ocr_ocupado.load(Ordering::Relaxed);
                
                ui.horizontal(|ui| {
                    ui.label("Thread OCR:");
                    if ocupado {
                        ui.colored_label(Color32::RED, "🔴 PROCESSANDO");
                    } else {
                        ui.colored_label(Color32::GREEN, "🟢 LIVRE");
                    }
                });

                ui.label(format!("Mouse Parado: {:.2}s", self.tempo_mouse_parado.elapsed().as_secs_f32()));
                
                if self.leitura_periodica {
                    let tempo = self.ultimo_disparo_periodico.elapsed().as_secs_f32();
                    ui.add(egui::ProgressBar::new(tempo / self.intervalo_leitura)
                        .text(format!("{:.1}s / {:.1}s", tempo, self.intervalo_leitura)));
                }
            });

        let painter = ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("debug_painter")));

        if self.leitura_mouse && !self.modo_selecao {
            let mut pt = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut pt); }
            let dpi = ctx.pixels_per_point();

            let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };

            let mouse_x = (pt.x - offset_x) as f32 / dpi;
            let mouse_y = (pt.y - offset_y) as f32 / dpi;
            
            let meio_x = self.largura_area_mouse / 2.0;
            let meio_y = self.altura_area_mouse / 2.0;
            
            let rect_mouse = Rect::from_min_max(
                Pos2::new(mouse_x - meio_x, mouse_y - meio_y),
                Pos2::new(mouse_x + meio_x, mouse_y + meio_y),
            );

            painter.rect_stroke(rect_mouse, 0.0, Stroke::new(2.0, cor_amarela));
            painter.text(rect_mouse.min, Align2::LEFT_BOTTOM, "Faixa do Mouse", FontId::proportional(12.0), cor_amarela);
        }

        if let Some(rect) = self.regiao_selecionada {
            painter.rect_stroke(rect, 0.0, Stroke::new(2.0, cor_ciano));
            painter.text(rect.min, Align2::LEFT_BOTTOM, "Fixo (OCR)", FontId::proportional(12.0), cor_ciano);
        }
    }

    pub fn executar_selecao_invisivel(&mut self, ctx: &Context) {
        if !self.modo_selecao { return; }
        Area::new(Id::new("area_invisivel"))
            .order(Order::Foreground)
            .fixed_pos(Pos2::new(0.0, 0.0))
            .show(ctx, |ui| {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
                let response = ui.allocate_rect(ctx.screen_rect(), Sense::drag());
                let pos_mouse = ctx.input(|i| i.pointer.interact_pos());

                if response.drag_started() { self.ponto_inicial = pos_mouse; }
                if response.dragged() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = Rect::from_two_pos(inicial, atual);
                        ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(255, 0, 0, 30));
                        ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::RED));
                    }
                }
                if response.drag_stopped() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = Rect::from_two_pos(inicial, atual);
                        if rect.width() > 10.0 && rect.height() > 10.0 { 
                            self.regiao_selecionada = Some(rect);
                            self.processar_pendente = true;
                        }
                    }
                    self.modo_selecao = false;
                }
            });
    }

    fn capturar_e_ocr(rect: Rect, ctx: &Context) -> Option<String> {
        let dpi = ctx.pixels_per_point();
        let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };

        // 1. Converte a coordenada interna do Egui de volta para a coordenada Global do Windows
        let global_x = (rect.min.x * dpi) as i32 + offset_x;
        let global_y = (rect.min.y * dpi) as i32 + offset_y;
        let w = (rect.width() * dpi) as u32;
        let h = (rect.height() * dpi) as u32;

        let monitors = Monitor::all().ok()?;
        let mut monitor_alvo = None;
        
        // 2. Verifica em qual monitor esse ponto global caiu
        for m in monitors {
            if global_x >= m.x() && global_x < m.x() + m.width() as i32 &&
               global_y >= m.y() && global_y < m.y() + m.height() as i32 {
                monitor_alvo = Some(m);
                break;
            }
        }
        
        let monitor = monitor_alvo.unwrap_or_else(|| Monitor::all().unwrap().into_iter().next().unwrap());

        // 3. Converte a coordenada Global para uma coordenada Local do Monitor específico
        let local_x = (global_x - monitor.x()).max(0) as u32;
        let local_y = (global_y - monitor.y()).max(0) as u32;

        let img = DynamicImage::ImageRgba8(monitor.capture_image().ok()?);

        // Previne crash caso a área selecionada passe da borda desse monitor
        let clip_w = w.min(img.width().saturating_sub(local_x));
        let clip_h = h.min(img.height().saturating_sub(local_y));
        if clip_w == 0 || clip_h == 0 { return None; }

        let img_cortada = img.crop_imm(local_x, local_y, clip_w, clip_h);
        
        let mut bytes = Vec::new();
        img_cortada.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).ok()?;

        let stream = InMemoryRandomAccessStream::new().ok()?;
        let output_stream = stream.GetOutputStreamAt(0).ok()?;
        let writer = DataWriter::CreateDataWriter(&output_stream).ok()?;
        writer.WriteBytes(&bytes);
        writer.StoreAsync().ok()?.get().ok()?;
        writer.FlushAsync().ok()?.get().ok()?;

        let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.get().ok()?;
        let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.get().ok()?;

        let idioma = Language::CreateLanguage(h!("pt-BR")).ok()?;
        let engine = OcrEngine::TryCreateFromLanguage(&idioma)
            .unwrap_or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages().unwrap());

        let resultado = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;
        let texto_lido = resultado.Text().ok()?.to_string_lossy();

        if texto_lido.trim().is_empty() {
            None
        } else {
            Some(texto_lido)
        }
    }
}