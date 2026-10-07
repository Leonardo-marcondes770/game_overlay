// src/lupa.rs
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

// 👇 CORREÇÃO: Rect e Stroke adicionados aqui!
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
    tamanho_origem: u32,
}

pub struct GerenciadorLupa {
    pub ativo: bool,
    pub modo_mouse: bool,
    pub fator_zoom: f32,
    pub tamanho_janela: f32,
    pub pixel_art: bool,
    
    pub selecionando_centro: bool,
    pub centro_fixo: Option<Pos2>,
    
    // 👇 CORREÇÃO: Faltou essa variável aqui na estrutura principal!
    pub ponto_inicial: Option<Pos2>,

    textura: Option<TextureHandle>,
    rx_imagem: Receiver<ColorImage>,
    config_sync: Arc<Mutex<LupaConfigSync>>,
}

impl GerenciadorLupa {
    pub fn new(ctx: Context) -> Self {
        let (tx, rx) = channel::<ColorImage>();
        let config_sync = Arc::new(Mutex::new(LupaConfigSync {
            ativo: false,
            modo_mouse: true,
            centro_global_x: 0,
            centro_global_y: 0,
            tamanho_origem: 100,
        }));

        let config_thread = Arc::clone(&config_sync);
        let ctx_thread = ctx.clone();

        thread::spawn(move || {
            let mut monitores_cacheados = Monitor::all().unwrap_or_default();
            let mut loops_sem_atualizar = 0;

            loop {
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

                    if let Some(color_img) = Self::capturar_regiao(c_x, c_y, config.tamanho_origem, &monitores_cacheados) {
                        let _ = tx.send(color_img);
                        ctx_thread.request_repaint(); 
                    }
                    thread::sleep(Duration::from_millis(30)); 
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
            selecionando_centro: false,
            centro_fixo: None,
            
            // 👇 CORREÇÃO: E faltou inicializá-la aqui também!
            ponto_inicial: None,
            
            textura: None,
            rx_imagem: rx,
            config_sync,
        }
    }

    fn capturar_regiao(centro_x: i32, centro_y: i32, tamanho: u32, monitores: &[Monitor]) -> Option<ColorImage> {
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
        
        let half = tamanho / 2;
        let start_x = local_x.saturating_sub(half);
        let start_y = local_y.saturating_sub(half);
        
        let clip_w = tamanho.min(img.width().saturating_sub(start_x));
        let clip_h = tamanho.min(img.height().saturating_sub(start_y));
        
        if clip_w == 0 || clip_h == 0 { return None; }
        
        let cropped = image::imageops::crop_imm(&img, start_x, start_y, clip_w, clip_h).to_image();
        
        let size = [cropped.width() as usize, cropped.height() as usize];
        let pixels = cropped.into_raw();
        Some(ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    pub fn atualizar_tick(&mut self, ctx: &Context) {
        if self.ativo && !self.selecionando_centro {
            ctx.request_repaint(); // Força o Egui a manter os 60 FPS da lupa
        }

        {
            let mut lock = self.config_sync.lock().unwrap();
            lock.ativo = self.ativo && !self.selecionando_centro;
            lock.modo_mouse = self.modo_mouse;
            lock.tamanho_origem = (self.tamanho_janela / self.fator_zoom).max(10.0) as u32; // Evita bugar em 0
            
            if let Some(pos) = self.centro_fixo {
                let dpi = ctx.pixels_per_point();
                let offset_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let offset_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                lock.centro_global_x = (pos.x * dpi) as i32 + offset_x;
                lock.centro_global_y = (pos.y * dpi) as i32 + offset_y;
            }
        }

        while let Ok(img) = self.rx_imagem.try_recv() {
            let filtro = if self.pixel_art { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
            self.textura = Some(ctx.load_texture("textura_lupa", img, filtro));
        }
    }

    pub fn exibir_ui(&mut self, ui: &mut Ui) -> bool {
        ui.heading("🔍 Lupa de Acessibilidade");
        ui.separator();
        
        ui.checkbox(&mut self.ativo, "Ligar Lupa na Tela");

        let mut disparar_selecao = false;

        ui.add_enabled_ui(self.ativo, |ui| {
            ui.add(egui::Slider::new(&mut self.fator_zoom, 1.5..=10.0).text("Zoom (Multiplicador)"));
            ui.add(egui::Slider::new(&mut self.tamanho_janela, 150.0..=800.0).text("Tamanho da Janela"));
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
                // 👇 Mudamos Sense::click() para Sense::drag()
                let response = ui.allocate_rect(ctx.screen_rect(), Sense::drag());
                let pos_mouse = ctx.input(|i| i.pointer.interact_pos());
                
                // Grava o ponto onde o clique começou
                if response.drag_started() { self.ponto_inicial = pos_mouse; }
                
                // Desenha o retângulo em tempo real enquanto arrasta
                if response.dragged() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = Rect::from_two_pos(inicial, atual);
                        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(100));
                        ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::GREEN));
                    }
                }
                
                // Quando soltar o clique, salva o centro desse retângulo como o novo alvo
                if response.drag_stopped() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = Rect::from_two_pos(inicial, atual);
                        self.centro_fixo = Some(rect.center());
                        self.selecionando_centro = false;
                    }
                    self.ponto_inicial = None;
                }
                
                // Instruções visuais quando não estiver arrastando
                if self.ponto_inicial.is_none() {
                    ui.painter().rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(150));
                    ui.painter().text(
                        ctx.screen_rect().center(), 
                        Align2::CENTER_CENTER, 
                        "Arraste e desenhe um quadrado sobre a área que deseja focar (ex: Mini-mapa)", 
                        FontId::proportional(24.0), 
                        Color32::WHITE
                    );
                }
            });
    }

    pub fn renderizar_janela(&self, ctx: &Context) {
        if !self.ativo || self.selecionando_centro { return; }

        // 👇 GARANTE QUE A JANELA TERÁ FUNDO PRETO MESMO SEM IMAGEM
        let frame_estilo = egui::Frame::window(&ctx.style())
            .inner_margin(0.0)
            .fill(Color32::from_rgb(15, 15, 15));

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
                        ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(self.tamanho_janela, self.tamanho_janela)));
                    } else {
                        // Se não tem imagem ainda, aloca espaço vazio para a janela aparecer
                        ui.allocate_exact_size(Vec2::new(self.tamanho_janela, self.tamanho_janela), Sense::hover());
                    }
                });
        } else {
            egui::Window::new("🔍 Lupa de Foco")
                .resizable(false)
                .frame(frame_estilo)
                .show(ctx, |ui| {
                    if let Some(tex) = &self.textura {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(self.tamanho_janela, self.tamanho_janela)));
                    } else {
                        ui.allocate_exact_size(Vec2::new(self.tamanho_janela, self.tamanho_janela), Sense::hover());
                    }
                });
        }
    }
}