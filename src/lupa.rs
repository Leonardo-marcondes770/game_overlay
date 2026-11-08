// src/lupa.rs
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{sync_channel, Receiver}; // 👇 Usando sync_channel
use std::thread;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, ColorImage, Context, FontId, Id, Pos2, TextureHandle, TextureOptions, Ui, Vec2, Order, CursorIcon, Sense, Rect, Stroke};
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};
use xcap::Monitor;

#[derive(Clone)]
struct LupaConfigSync {
    ativo: bool,
    modo_mouse: bool,
    centro_global_x: i32,
    centro_global_y: i32,
    largura_origem: u32,
    altura_origem: u32,
    fps_captura: u64,
}

pub struct GerenciadorLupa {
    pub ativo: bool,
    pub modo_mouse: bool,
    pub fator_zoom: f32,
    pub tamanho_janela: f32,
    pub pixel_art: bool,
    pub fps_captura: u64,
    
    pub selecionando_centro: bool,
    pub area_fixa: Option<Rect>,
    pub ponto_inicial: Option<Pos2>,

    textura: Option<TextureHandle>,
    rx_imagem: Receiver<ColorImage>,
    config_sync: Arc<Mutex<LupaConfigSync>>,
}

impl GerenciadorLupa {
    pub fn new(ctx: Context) -> Self {
        // 👇 OTIMIZAÇÃO 1: Buffer de apenas 1 frame! Se Egui não ler a tempo, descarta para poupar RAM.
        let (tx, rx) = sync_channel::<ColorImage>(1); 
        
        let config_sync = Arc::new(Mutex::new(LupaConfigSync {
            ativo: false,
            modo_mouse: true,
            centro_global_x: 0,
            centro_global_y: 0,
            largura_origem: 100,
            altura_origem: 100,
            fps_captura: 24,
        }));

        let config_thread = Arc::clone(&config_sync);
        let ctx_thread = ctx.clone();

        thread::spawn(move || {
            let mut monitores_cacheados = Monitor::all().unwrap_or_default();
            let mut loops_sem_atualizar = 0;

            loop {
                let inicio_frame = Instant::now(); // 👇 OTIMIZAÇÃO 2: Medir o tempo do processador!

                loops_sem_atualizar += 1;
                if loops_sem_atualizar > 100 {
                    if let Ok(m) = Monitor::all() {
                        monitores_cacheados = m;
                    }
                    loops_sem_atualizar = 0;
                }

                let config = {
                    let lock = config_thread.lock().unwrap();
                    lock.clone()
                };

                if config.ativo && !monitores_cacheados.is_empty() {
                    let mut c_x = config.centro_global_x;
                    let mut c_y = config.centro_global_y;

                    if config.modo_mouse {
                        let mut pt = POINT { x: 0, y: 0 };
                        unsafe { GetCursorPos(&mut pt); }
                        c_x = pt.x;
                        c_y = pt.y;
                    }

                    if let Some(color_img) = Self::capturar_regiao(c_x, c_y, config.largura_origem, config.altura_origem, &monitores_cacheados) {
                        // try_send envia sem bloquear. Se o buffer(1) estiver cheio, ele ignora e não sobrecarrega a máquina!
                        if tx.try_send(color_img).is_ok() {
                            ctx_thread.request_repaint(); 
                        }
                    }
                    
                    // OTIMIZAÇÃO 2 CONTINUAÇÃO: Desconta o tempo da captura do tempo de descanso!
                    let tempo_gasto = inicio_frame.elapsed();
                    let delay_alvo = Duration::from_millis(1000 / config.fps_captura.max(1));
                    
                    if delay_alvo > tempo_gasto {
                        thread::sleep(delay_alvo - tempo_gasto);
                    }
                } else {
                    thread::sleep(Duration::from_millis(200)); 
                }
            }
        });

        Self {
            ativo: false,
            modo_mouse: true,
            fator_zoom: 3.0,
            tamanho_janela: 300.0,
            pixel_art: false,
            fps_captura: 24,
            selecionando_centro: false,
            area_fixa: None,
            ponto_inicial: None,
            textura: None,
            rx_imagem: rx,
            config_sync,
        }
    }

    fn capturar_regiao(centro_x: i32, centro_y: i32, largura: u32, altura: u32, monitores: &[Monitor]) -> Option<ColorImage> {
        let mut monitor_alvo = None;
        for m in monitores {
            if centro_x >= m.x() && centro_x < m.x() + m.width() as i32 &&
               centro_y >= m.y() && centro_y < m.y() + m.height() as i32 {
                monitor_alvo = Some(m);
                break;
            }
        }
        
        let monitor = monitor_alvo.unwrap_or_else(|| monitores.first().unwrap());
        let local_x = (centro_x - monitor.x()).max(0) as u32;
        let local_y = (centro_y - monitor.y()).max(0) as u32;
        
        let img = monitor.capture_image().ok()?;
        
        let half_w = largura / 2;
        let half_h = altura / 2;
        let start_x = local_x.saturating_sub(half_w);
        let start_y = local_y.saturating_sub(half_h);
        
        let img_w = img.width();
        let img_h = img.height();
        
        let clip_w = largura.min(img_w.saturating_sub(start_x));
        let clip_h = altura.min(img_h.saturating_sub(start_y));
        
        if clip_w == 0 || clip_h == 0 { return None; }
        
        // 👇 OTIMIZAÇÃO 3: Corte Manual Nativo! Arranca o imageops e copia os bits diretamente para a memória.
        let src_bytes = img.as_raw();
        let mut pixels = Vec::with_capacity((clip_w * clip_h) as usize);
        
        for y in 0..clip_h {
            let src_y = start_y + y;
            let start_idx = ((src_y * img_w + start_x) * 4) as usize;
            let end_idx = start_idx + (clip_w * 4) as usize;
            
            let row = &src_bytes[start_idx..end_idx];
            for chunk in row.chunks_exact(4) {
                pixels.push(Color32::from_rgba_unmultiplied(chunk[0], chunk[1], chunk[2], chunk[3]));
            }
        }
        
        let size = [clip_w as usize, clip_h as usize];
        Some(ColorImage { size, pixels })
    }

    pub fn atualizar_tick(&mut self, ctx: &Context) {
        {
            let mut lock = self.config_sync.lock().unwrap();
            lock.ativo = self.ativo && !self.selecionando_centro;
            lock.modo_mouse = self.modo_mouse;
            lock.fps_captura = self.fps_captura;
            
            let dpi = ctx.pixels_per_point();
            
            if self.modo_mouse {
                lock.largura_origem = (self.tamanho_janela / self.fator_zoom).max(10.0) as u32;
                lock.altura_origem = (self.tamanho_janela / self.fator_zoom).max(10.0) as u32;
            } else if let Some(rect) = self.area_fixa {
                let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                
                lock.centro_global_x = (rect.center().x * dpi) as i32 + offset_x;
                lock.centro_global_y = (rect.center().y * dpi) as i32 + offset_y;
                
                lock.largura_origem = ((rect.width() * dpi) / self.fator_zoom).max(10.0) as u32;
                lock.altura_origem = ((rect.height() * dpi) / self.fator_zoom).max(10.0) as u32;
            }
        }

        while let Ok(img) = self.rx_imagem.try_recv() {
            let filtro = if self.pixel_art { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
            
            // 👇 OTIMIZAÇÃO 4: Reuso de Buffers OpenGL! Paramos de alocar Texturas novas por Frame!
            if let Some(tex) = &mut self.textura {
                tex.set(img, filtro); 
            } else {
                self.textura = Some(ctx.load_texture("textura_lupa", img, filtro));
            }
        }
    }

    // A partir daqui, as funções `exibir_ui`, `executar_selecao_centro` e `renderizar_janela` continuam
    // EXATAMENTE iguais ao código anterior (coloque-as aqui embaixo).

    pub fn exibir_ui(&mut self, ui: &mut Ui) -> bool {
        ui.heading("🔍 Lupa de Acessibilidade");
        ui.separator();
        
        ui.checkbox(&mut self.ativo, "Ligar Lupa na Tela");

        let mut disparar_selecao = false;

        ui.add_enabled_ui(self.ativo, |ui| {
            ui.add(egui::Slider::new(&mut self.fator_zoom, 1.0..=10.0).text("Zoom (Multiplicador)"));
            ui.add(egui::Slider::new(&mut self.fps_captura, 5..=60).text("FPS da Lupa (Uso de CPU)"));
            ui.add_enabled(self.modo_mouse, egui::Slider::new(&mut self.tamanho_janela, 150.0..=800.0).text("Tamanho da Janela (Apenas Modo Mouse)"));
            ui.checkbox(&mut self.pixel_art, "Modo Pixel Art (Bordas Duras)");

            ui.add_space(8.0);
            ui.group(|ui| {
                ui.radio_value(&mut self.modo_mouse, true, "📌 Seguir o Cursor do Mouse");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.modo_mouse, false, "🎯 Área Fixa");
                    if !self.modo_mouse {
                        if ui.button("Definir Alvo").clicked() {
                            self.selecionando_centro = true;
                            disparar_selecao = true;
                        }
                    }
                });
            });
        });

        disparar_selecao
    }

    pub fn executar_selecao_centro(&mut self, ctx: &Context) {
        if !self.selecionando_centro { return; }
        
        egui::Area::new(Id::new("area_selecao_centro_lupa"))
            .order(Order::Foreground)
            .fixed_pos(Pos2::new(0.0, 0.0))
            .show(ctx, |ui| {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
                let response = ui.allocate_rect(ctx.screen_rect(), Sense::click_and_drag());
                let pos_mouse = ctx.input(|i| i.pointer.interact_pos());
                
                if response.drag_started() { self.ponto_inicial = pos_mouse; }
                
                if response.dragged() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = Rect::from_two_pos(inicial, atual);
                        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(100));
                        ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::GREEN));
                    }
                }
                
                if response.drag_stopped() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let mut rect = Rect::from_two_pos(inicial, atual);
                        if rect.width() < 10.0 || rect.height() < 10.0 {
                            rect = Rect::from_center_size(rect.center(), Vec2::new(100.0, 100.0));
                        }
                        self.area_fixa = Some(rect);
                    }
                    self.selecionando_centro = false;
                    self.ponto_inicial = None;
                }

                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.area_fixa = Some(Rect::from_center_size(pos, Vec2::new(100.0, 100.0)));
                    }
                    self.selecionando_centro = false;
                    self.ponto_inicial = None;
                }
                
                if self.ponto_inicial.is_none() {
                    ui.painter().rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(150));
                    ui.painter().text(
                        ctx.screen_rect().center(), 
                        Align2::CENTER_CENTER, 
                        "Arraste para desenhar o tamanho da lupa, ou clique para foco padrão.", 
                        FontId::proportional(24.0), 
                        Color32::WHITE
                    );
                }
            });
    }

    pub fn renderizar_janela(&self, ctx: &Context) {
        if !self.ativo || self.selecionando_centro { return; }

        let frame_estilo = egui::Frame::window(&ctx.style())
            .inner_margin(0.0)
            .fill(Color32::from_rgb(15, 15, 15));

        let (exibir_w, exibir_h) = if self.modo_mouse {
            (self.tamanho_janela, self.tamanho_janela)
        } else if let Some(rect) = self.area_fixa {
            let mut w = rect.width();
            let mut h = rect.height();
            let maior_lado = w.max(h);
            if maior_lado < 250.0 {
                let escala = 250.0 / maior_lado;
                w *= escala;
                h *= escala;
            }
            (w, h)
        } else {
            (self.tamanho_janela, self.tamanho_janela)
        };

        if self.modo_mouse {
            let mut pt = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut pt); }
            let dpi = ctx.pixels_per_point();
            let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            
            let mouse_x = (pt.x - offset_x) as f32 / dpi;
            let mouse_y = (pt.y - offset_y) as f32 / dpi;

            egui::Window::new("Lupa_Flutuante")
                .fixed_pos(Pos2::new(mouse_x + 25.0, mouse_y + 25.0))
                .title_bar(false)
                .resizable(false)
                .frame(frame_estilo)
                .show(ctx, |ui| {
                    if let Some(tex) = &self.textura {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(exibir_w, exibir_h)));
                    } else {
                        ui.allocate_exact_size(Vec2::new(exibir_w, exibir_h), Sense::hover());
                    }
                });
        } else {
            egui::Window::new("🔍 Lupa de Foco")
                .resizable(true)
                .min_size(Vec2::new(50.0, 50.0))
                .default_size(Vec2::new(exibir_w, exibir_h))
                .frame(frame_estilo)
                .show(ctx, |ui| {
                    let mut size = ui.available_size();
                    if size.x < 10.0 || size.y < 10.0 {
                        size = Vec2::new(exibir_w, exibir_h);
                    }
                    if let Some(tex) = &self.textura {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(size));
                    } else {
                        ui.allocate_exact_size(size, Sense::hover());
                    }
                });
        }
    }
}