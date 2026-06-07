use std::time::{Duration, Instant};
use xcap::Monitor;
use image::DynamicImage;
use std::process::Command;
use std::fs;
use std::os::windows::process::CommandExt;
use std::thread; // 🧵 Importamos para criar threads em segundo plano
use std::sync::{Arc, Mutex}; // 🔒 Para compartilhar o texto entre as threads com segurança

#[derive(Clone)]
pub struct GerenciadorLeitor {
    pub ativo: bool,
    pub modo_selecao: bool,
    pub regiao_selecionada: Option<egui::Rect>,
    ponto_inicial: Option<egui::Pos2>,
    ultima_captura: Instant,
    pub texto_extraido: String,
    // Usamos um contador ou flag simples para saber se já existe uma thread de OCR rodando
    is_processing: Arc<Mutex<bool>>,
}

impl GerenciadorLeitor {
    pub fn new() -> Self {
        Self {
            ativo: false,
            modo_selecao: false,
            regiao_selecionada: None,
            ponto_inicial: None,
            ultima_captura: Instant::now(),
            texto_extraido: String::from("Nenhum texto processado ainda."),
            is_processing: Arc::new(Mutex::new(false)),
        }
    }

    /// Retorna `true` se o usuário clicar no botão de selecionar área para o main.rs ocultar o menu
    pub fn exibir_ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) -> bool {
        ui.heading("🔍 On-Screen Reader");
        ui.separator();

        ui.checkbox(&mut self.ativo, "👁️ Ativar Monitoramento de Tela");

        let mut disparar_selecao = false;

        ui.horizontal(|ui| {
            if ui.button("🎯 Selecionar Área da Tela").clicked() {
                self.modo_selecao = true;
                self.ponto_inicial = None;
                disparar_selecao = true;
            }

            if ui.button("🗑️ Limpar").clicked() {
                self.regiao_selecionada = None;
                self.texto_extraido = String::from("Nenhum texto processado ainda.");
            }
        });

        if let Some(rect) = self.regiao_selecionada {
            ui.colored_label(
                egui::Color32::LIGHT_GREEN,
                format!(
                    "Área salva: Pos({}, {}) - Tam({}x{})",
                    rect.min.x as i32, rect.min.y as i32,
                    rect.width() as i32, rect.height() as i32
                ),
            );

            if rect.width() * rect.height() > 250_000.0 {
                ui.colored_label(
                    egui::Color32::GOLD,
                    "⚠️ Área muito grande! Otimização de nitidez desativada para poupar RAM."
                );
            }
            
            ui.add_space(5.0);
            ui.label("📝 Último texto lido:");
            ui.vertical(|ui| {
                ui.colored_label(egui::Color32::WHITE, &self.texto_extraido);
            });
        } else {
            ui.colored_label(egui::Color32::LIGHT_RED, "Nenhuma área selecionada.");
        }

        disparar_selecao
    }

    /// Executa a captura do mouse usando uma Área transparente fixa sobre toda a tela
    pub fn executar_selecao_invisivel(&mut self, ctx: &egui::Context) {
        if !self.modo_selecao { return; }

        egui::Area::new(egui::Id::new("area_de_selecao_invisivel"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                
                let tela_toda = ctx.screen_rect();
                let response = ui.allocate_rect(tela_toda, egui::Sense::drag());
                let pos_mouse = ctx.input(|i| i.pointer.interact_pos());

                if response.drag_started() {
                    self.ponto_inicial = pos_mouse;
                }

                if response.dragged() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = egui::Rect::from_two_pos(inicial, atual);
                        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(255, 0, 0, 30));
                        ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::RED));
                    }
                }

                if response.drag_stopped() {
                    if let (Some(inicial), Some(atual)) = (self.ponto_inicial, pos_mouse) {
                        let rect = egui::Rect::from_two_pos(inicial, atual);
                        if rect.width() > 10.0 && rect.height() > 10.0 {
                            self.regiao_selecionada = Some(rect);
                        }
                    }
                    self.modo_selecao = false;
                    self.ponto_inicial = None;
                }
            });
    }

    /// Executa o fluxo de captura e OCR sem travar a interface (Assíncrono via Thread)
    pub fn atualizar_tick(&mut self, ctx: &egui::Context) {
        if !self.ativo || self.regiao_selecionada.is_none() {
            return;
        }

        if self.ultima_captura.elapsed() >= Duration::from_millis(1000) {
            // Se já tiver uma thread rodando o Tesseract, não iniciamos outra para não acumular processos
            if let Ok(processing) = self.is_processing.try_lock() {
                if *processing { return; }
            }

            self.ultima_captura = Instant::now();

            if let Some(rect) = self.regiao_selecionada {
                // Clonamos as referências seguras para enviar para dentro da nova thread
                let is_processing_clone = Arc::clone(&self.is_processing);
                let ctx_clone = ctx.clone();
                
                // Criamos um canal ou um ponteiro temporário para coletar o texto modificado depois.
                // Mas para manter sua struct simples, passamos os dados diretamente por cópia de valores.
                if let Ok(mut processing) = self.is_processing.lock() {
                    *processing = true;
                }

                // 🚀 Criamos a Thread em segundo plano
                let gerenciador_clone = self.clone();
                
                thread::spawn(move || {
                    if let Some(texto) = gerenciador_clone.capturar_e_ocr(rect, &ctx_clone) {
                        let texto_limpo = texto.trim().to_string();
                        if !texto_limpo.is_empty() {
                            // Como precisamos atualizar a UI principal, o truque é usar o egui Context
                            // para injetar a string atualizada de volta com segurança na próxima renderização.
                            ctx_clone.data_mut(|d| d.insert_temp(egui::Id::new("ocr_texto_key"), texto_limpo));
                        }
                    }
                    
                    // Libera a flag indicando que terminou
                    if let Ok(mut processing) = is_processing_clone.lock() {
                        *processing = false;
                    }
                    
                    // Força a UI a atualizar imediatamente quando o texto chegar
                    ctx_clone.request_repaint();
                });
            }
        }

        // 📥 Recuperamos o texto que a thread paralela salvou no cache de dados do egui
        ctx.data(|d| {
            if let Some(texto) = d.get_temp::<String>(egui::Id::new("ocr_texto_key")) {
                if self.texto_extraido != texto {
                    self.texto_extraido = texto;
                }
            }
        });
    }

    fn capturar_e_ocr(&self, rect: egui::Rect, ctx: &egui::Context) -> Option<String> {
        let monitors = Monitor::all().ok()?;
        let monitor_principal = monitors.first()?;
        let imagem_completa = monitor_principal.capture_image().ok()?;
        
        let img = DynamicImage::ImageRgba8(imagem_completa);
        let dpi = ctx.pixels_per_point();

        let x = (rect.min.x * dpi).max(0.0) as u32;
        let y = (rect.min.y * dpi).max(0.0) as u32;
        let largura = (rect.width() * dpi) as u32;
        let altura = (rect.height() * dpi) as u32;

        if x + largura > img.width() || y + altura > img.height() {
            return None;
        }

        let imagem_cortada = img.crop_imm(x, y, largura, altura);

        let img_cinza = imagem_cortada.into_luma8();

        let area_fisica = largura * altura;
        let fator_escala = if area_fisica > 400_000 { 1 } else { 2 };

        let img_redimensionada = image::imageops::resize(
            &img_cinza, 
            img_cinza.width() * fator_escala, 
            img_cinza.height() * fator_escala, 
            image::imageops::FilterType::Triangle
        );

        let pasta_temp = std::env::temp_dir();
        let caminho_txt_base = pasta_temp.join("overlay_ocr_output");
        let caminho_txt_final = pasta_temp.join("overlay_ocr_output.txt");

        let mut bytes_imagem = Vec::new();
        let _ = DynamicImage::ImageLuma8(img_redimensionada)
            .write_to(&mut std::io::Cursor::new(&mut bytes_imagem), image::ImageFormat::Png);

        // Configura o executável portátil local
        let mut comando = Command::new(".\\tesseract_bin\\tesseract.exe");
        
        comando.arg("-") 
               .arg(&caminho_txt_base)
               .arg("-l")
               .arg("por")
               .arg("--tessdata-dir")
               .arg(".\\tessdata")
               .stdin(std::process::Stdio::piped());

        #[cfg(windows)]
        comando.creation_flags(0x08000000); 

        if let Ok(mut processo) = comando.spawn() {
            if let Some(mut stdin) = processo.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(&bytes_imagem);
            }
            
            if let Ok(status) = processo.wait() {
                if status.success() {
                    if let Ok(texto) = fs::read_to_string(&caminho_txt_final) {
                        let _ = fs::remove_file(&caminho_txt_final);
                        return Some(texto);
                    }
                }
            }
        }

        None
    }
}